# Platform verification

Verified locally on 18 October 2026 on macOS Apple Silicon (Darwin arm64). Cross compilation is distinguished from runtime validation below. No remote CI was triggered, and no system-wide tools or Rosetta were installed.

## Verified compilation

The workspace uses a local Rust toolchain under `work/toolchain`; cross artifacts are isolated under `work/cross-target` and `work/cross-core-target` outside the deliverable source tree.

| Target | Verification | Result |
| --- | --- | --- |
| Intel macOS, `x86_64-apple-darwin` | `cargo check --workspace --target x86_64-apple-darwin` | Passed |
| Intel macOS, `x86_64-apple-darwin` | `cargo build --workspace --target x86_64-apple-darwin` | Passed; `file` confirms a Mach-O 64-bit x86_64 executable |
| Linux, `x86_64-unknown-linux-gnu` | `cargo check --workspace --target x86_64-unknown-linux-gnu` with Zig C compiler | Passed |
| Linux, `x86_64-unknown-linux-gnu` | `cargo build --workspace --target x86_64-unknown-linux-gnu` with Zig C linker | Passed; `file` confirms ELF 64-bit x86-64 executable |
| Windows GNU, `x86_64-pc-windows-gnu` | `cargo check --workspace --target x86_64-pc-windows-gnu` with Zig C compiler | Passed |
| Windows GNU, `x86_64-pc-windows-gnu` | `scripts/platform-checks.sh --build x86_64-pc-windows-gnu` with official MSVCRT target libraries | Passed; `file` confirms PE32+ x86-64 console executable |

The macOS build uses the installed Xcode SDK returned by `xcrun --show-sdk-path`. Standard-library targets were installed only into the local Rust toolchain with `rustup target add x86_64-apple-darwin x86_64-unknown-linux-gnu x86_64-pc-windows-gnu`.

Initial full Linux and Windows workspace checks reached native C dependencies and reported the absent `x86_64-linux-gnu-gcc` and `x86_64-w64-mingw32-gcc` compilers. These failures do not imply a source compatibility defect. A local Zig cross-compiler was downloaded from the official distribution to extend compilation checks without system installations. Zig 0.17.0 archive SHA-256: `b607e9b9234790a008116ae5bdb71c6243b84b9fb42a53a9e70fde41c06c536a`, matching the [official download index](https://ziglang.org/download/index.json). Full workspace checks then passed on both targets. The compiler wrappers remove the Rust `--target=` argument and pass Zig's corresponding `-target x86_64-linux-gnu` or `-target x86_64-windows-gnu`, with `zig ar` for archive creation. `scripts/platform-checks.sh` reproduces these checks with `ZIG=/absolute/path/to/zig`; its optional `--build` flag also links binaries. It requires Python 3 for argument translation and uses only temporary wrappers. The environment variables are `CC_x86_64_unknown_linux_gnu`, `AR_x86_64_unknown_linux_gnu`, `CC_x86_64_pc_windows_gnu`, and `AR_x86_64_pc_windows_gnu`.

## Windows cross-link verification

A full Windows binary link initially lacked the MSVCRT and pthread libraries required by the Rust GNU standard library. `rustup component add rust-mingw --target x86_64-pc-windows-gnu` confirmed that this component is unavailable for the macOS host toolchain.

This was resolved locally using target libraries from the [official LLVM-MinGW 20261006 release](https://github.com/mstorsjo/llvm-mingw/releases/tag/20261006). The MSVCRT Ubuntu cross-toolchain archive SHA-256 was `9dd3bbff7f746937c99b38322c8b1fcab8054b7d8d31ec840b3457234dff7e52`, exactly matching the release asset digest. Only x86_64 Windows target `.a` libraries were extracted into `work/windows-msvcrt-libs`; no foreign host executable was run.

With `WINDOWS_GNU_LIB_DIR` pointing to those libraries, the helper uses `zig dlltool` and explicitly resolves MSVCRT and MinGW extension archives, keeping Zig's startup library to avoid duplicate TLS definitions. The full build succeeded. `objdump -p` confirms imports from Windows system DLLs only: MSVCRT, KERNEL32, Windows synchronization API, Winsock, NTDLL, USERENV, bcrypt primitives, ADVAPI32, and bcrypt. No extra pthread DLL is imported. Zig emits one linker warning that its auto-image-base option is ignored; compilation and linking complete successfully.

The successful cross-build commands were:

```sh
ZIG=/absolute/path/to/zig scripts/platform-checks.sh x86_64-unknown-linux-gnu x86_64-pc-windows-gnu
ZIG=/absolute/path/to/zig scripts/platform-checks.sh --build x86_64-unknown-linux-gnu
ZIG=/absolute/path/to/zig WINDOWS_GNU_LIB_DIR=/absolute/path/to/windows-msvcrt-libs scripts/platform-checks.sh --build x86_64-pc-windows-gnu
```

These are developer verification binaries. They have not been runtime-tested or packaged as supported platform releases.

## macOS Apple Silicon credential-store runtime check

The explicitly ignored opt-in `isolated_native_vault_roundtrip` integration test passed against the native macOS Apple Silicon backend. It created, read and deleted a unique nonsecret probe entry, then confirmed `NoEntry`. Existing provider credential entries and user API keys were never read. This verifies basic local native-vault roundtrip/cleanup, not live provider authentication or other operating systems.

## Runtime boundary

`arch -x86_64 /usr/bin/true` fails with `Bad CPU type in executable`: Rosetta execution is unavailable. Intel macOS compilation passed, but execution, keychain behavior, and installer behavior on Intel macOS are not validated here.

No Docker, Podman, Wine, QEMU, or other Linux/Windows runtime was available in the tool inventory. Target standard libraries and a C cross-compiler enable compilation checks, not OS execution. Linux Secret Service and Windows Credential Manager runtime behavior still require those operating systems. The repository's CI matrix can perform native validation when run by its owner.

## Dashboard dependency verification

The previous five development-only findings were one transitive `braces` advisory chain through `eslint-config-next`. The latest registry versions at verification were `braces` 3.0.3, `fast-glob` 3.3.3, `micromatch` 4.0.8 and `eslint-config-next` 16.4.2. A direct patched `braces` release was unavailable.

The unused Next ESLint wrapper was replaced with direct ESLint recommended JavaScript and TypeScript rules, React Hooks recommended rules, and JSX accessibility recommended rules. No vulnerability suppression or blanket rule disabling was introduced. Next.js runtime remains 16.4.2.

After installation, both full `npm audit --json` and production-only `npm audit --omit=dev --json` report zero vulnerabilities. Dashboard TypeScript checking and static production build passed. The stricter lint rules exposed an anchor/button accessibility error and an empty test catch block; both were corrected by the source owners. `npm run lint` now passes with all recommended rules retained.
