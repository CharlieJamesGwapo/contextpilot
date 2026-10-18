#!/usr/bin/env python3
"""Collect declared licenses and available bundled notices, without executing packages.
Usage: cargo metadata --locked --format-version 1 > metadata.json
       python3 scripts/third-party-notices.py metadata.json apps/dashboard/node_modules OUTPUT
Includes build and alternate-target dependencies; does not claim legal clearance.
"""
import json, pathlib, sys
metadata, npm_root, output = map(pathlib.Path, sys.argv[1:])
sections = ['ContextPilot third-party dependency notices\n\nGenerated from Cargo.lock metadata and installed npm package metadata. Includes build/dev and alternate-platform packages. Declared license expressions are reported verbatim; bundled license/copyright/notice texts follow where available. This inventory is not a trademark or legal clearance.\n']
count = 0

def add(name, version, license_expression, directory, authors=None):
    global count
    count += 1
    sections.append(f'\n{"="*72}\n{name} {version}\nDeclared license: {license_expression or "not declared"}\n')
    if authors:
        sections.append('Package authors: ' + ', '.join(authors) + '\n')
    paths = sorted(p for p in directory.iterdir() if p.is_file() and p.name.lower().startswith(('license', 'licence', 'copying', 'notice', 'copyright')))
    for path in paths:
        if path.stat().st_size > 1_048_576:
            raise ValueError('Unexpectedly large notice file')
        try:
            content = path.read_text()
        except UnicodeError:
            continue
        sections.append(f'\n--- {path.name} ---\n{content}\n')

for package in sorted(json.loads(metadata.read_text())['packages'], key=lambda p:(p['name'],p['version'])):
    if package.get('source'):
        add(package['name'], package['version'], package.get('license'), pathlib.Path(package['manifest_path']).parent, package.get('authors'))
for path in sorted(npm_root.rglob('package.json')):
    if len(path.relative_to(npm_root).parts) > 5:
        continue
    try:
        package = json.loads(path.read_text())
    except (OSError, ValueError):
        continue
    if not package.get('name') or not package.get('version'):
        continue
    add(package['name'],package['version'],package.get('license'),path.parent)
output.write_text(''.join(sections))
print(f'Collected notices for {count} dependency packages.')
