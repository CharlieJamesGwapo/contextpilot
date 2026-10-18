#!/usr/bin/env bash
# Optional local cross-compilation. Set ZIG=/absolute/path/to/zig for Linux/Windows.
set -euo pipefail
cd "$(dirname "$0")/.."
mode=check
if [[ "${1:-}" == --build ]]; then mode=build; shift; fi
if [[ $# == 0 ]]; then set -- x86_64-apple-darwin x86_64-unknown-linux-gnu x86_64-pc-windows-gnu; fi
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
cat > "$scratch/cc" <<'WRAPPER'
#!/usr/bin/env python3
import os, sys
args = [a for a in sys.argv[1:] if not a.startswith('--target=')]
if os.environ.get('WINDOWS_GNU_LIB_DIR'):
    args = [os.path.join(os.environ['WINDOWS_GNU_LIB_DIR'], 'lib' + a[2:] + '.a') if a in ('-lmsvcrt', '-lmingwex') else a for a in args]
os.execv(os.environ['ZIG'], [os.environ['ZIG'], 'cc', '-target', os.environ['CONTEXTPILOT_ZIG_TARGET']] + args)
WRAPPER
cat > "$scratch/ar" <<'WRAPPER'
#!/bin/sh
exec "$ZIG" ar "$@"
WRAPPER
cat > "$scratch/dlltool" <<'WRAPPER'
#!/bin/sh
exec "$ZIG" dlltool "$@"
WRAPPER
chmod +x "$scratch/cc" "$scratch/ar" "$scratch/dlltool"
cargo_command=cargo
if [[ -n "${CARGO_HOME:-}" && -x "$CARGO_HOME/bin/cargo" ]]; then cargo_command="$CARGO_HOME/bin/cargo"; fi
for target in "$@"; do
  case "$target" in
    x86_64-unknown-linux-gnu) export CONTEXTPILOT_ZIG_TARGET=x86_64-linux-gnu ;;
    x86_64-pc-windows-gnu) export CONTEXTPILOT_ZIG_TARGET=x86_64-windows-gnu ;;
    *-apple-darwin) "$cargo_command" "$mode" --workspace --target "$target"; continue ;;
    *) printf 'Unsupported script target: %s\n' "$target" >&2; exit 2 ;;
  esac
  if [[ -z "${ZIG:-}" || ! -x "$ZIG" ]]; then printf 'Set ZIG to an absolute executable path for %s\n' "$target" >&2; exit 2; fi
  target_key=${target//-/_}
  target_upper=$(printf '%s' "$target_key" | tr '[:lower:]' '[:upper:]')
  export "CC_${target_key}=$scratch/cc" "AR_${target_key}=$scratch/ar" "CARGO_TARGET_${target_upper}_LINKER=$scratch/cc"
  if [[ "$target" == x86_64-pc-windows-gnu && "$mode" == build ]]; then
    flags="-C dlltool=$scratch/dlltool"
    if [[ -n "${WINDOWS_GNU_LIB_DIR:-}" ]]; then flags="$flags -L native=$WINDOWS_GNU_LIB_DIR"; fi
    export "CARGO_TARGET_${target_upper}_RUSTFLAGS=$flags"
  fi
  "$cargo_command" "$mode" --workspace --target "$target"
done
