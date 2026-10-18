#!/usr/bin/env python3
"""Best-effort publication scan. Prints locations, never credential values."""
import argparse
import pathlib
import re
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--staged', action='store_true', help='Scan actual staged Git blobs')
args = parser.parse_args()
command = ['git', 'diff', '--cached', '--name-only', '--diff-filter=ACMR', '-z'] if args.staged else ['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z']
paths = subprocess.check_output(command, cwd=root).decode().split('\0')
patterns = [
    re.compile(r'\bsk-(?:proj-)?[A-Za-z0-9_-]{32,}\b'),
    re.compile(r'\bAKIA[A-Z0-9]{16}\b'),
    re.compile(r'\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{36,}\b'),
    re.compile(r'\bgithub_pat_[A-Za-z0-9_]{60,}\b'),
    re.compile(r'-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----'),
]
findings = []
for name in sorted(set(filter(None, paths))):
    path = pathlib.PurePosixPath(name)
    if path.name.startswith('.env') and path.name != '.env.example' or path.name.endswith(('.token', '.sqlite', '.sqlite3', '.sqlite-wal', '.sqlite-shm', '.log')) or any(part in {'.contextpilot', '.local-ux', '.vercel', '.wrangler', 'node_modules', '.next', 'out', 'hosted-out'} for part in path.parts):
        findings.append(f'{name}: forbidden publication path')
        continue
    try:
        raw = subprocess.check_output(['git', 'show', ':' + name], cwd=root) if args.staged else (root / name).read_bytes()
        text = raw.decode('utf-8')
    except (UnicodeError, OSError):
        continue
    for number, line in enumerate(text.splitlines(), 1):
        if any(pattern.search(line) for pattern in patterns):
            findings.append(f'{name}:{number}: potential credential')
if findings:
    print('\n'.join(findings))
    sys.exit(1)
print(f'PASS: {len(set(filter(None,paths)))} publication files scanned; no known credential patterns or forbidden data paths (best effort).')
