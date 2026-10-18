# ContextPilot

**Pick up your next AI coding session without starting from scratch.**

ContextPilot helps you prepare useful project context, remove safe repetition from prompts, and save decisions, constraints, test results and next steps as checkpoints. Export a checkpoint for your next conversation with an AI coding assistant, then continue from where you left off.

**[Try the live app](https://contextpilot-3pl.pages.dev/) · [Browse the source](https://github.com/CharlieJamesGwapo/contextpilot) · [Report an issue](https://github.com/CharlieJamesGwapo/contextpilot/issues)**

No account or AI API key is needed for the browser workspace or the core local tools. ContextPilot prepares context for your assistant; it does not generate AI answers or modify your code in the browser edition.

![ContextPilot browser workspace with no private project data](docs/hosted-preview.jpg)

## What can I do with it?

- **Prepare project context:** choose files, search locally, review their contents and attach only what you need to a session.
- **Tidy up prompts:** compare your original prompt with conservative local changes. Requirements and constraints are protected; review the result before using it.
- **Keep a session:** record your objective, decisions, useful context, verification results and next steps.
- **Save your place:** preview, inspect and export integrity-checked checkpoints for your next AI conversation.
- **Recover your work:** restore saved browser checkpoints and export/import private workspace backups.

Token estimates are labeled. A smaller prompt is not proof of lower provider bills or better AI answers.

## Try it now — no installation or API keys

Open **[contextpilot-3pl.pages.dev](https://contextpilot-3pl.pages.dev/)** in a modern browser.

1. In **Sessions**, create a session with your task and constraints.
2. In **Projects**, name a project and choose a folder or individual text files. Search, review and attach selected files to your session.
3. In **Prompts**, paste instructions and choose **Optimize** to compare local suggestions.
4. Back in **Sessions**, add progress notes, choose **Preview checkpoint**, review it, then **Save checkpoint**. Export text for your assistant or JSON for a later import.
5. In **Settings**, choose **Export browser backup** regularly. To recover elsewhere, open ContextPilot and choose **Import checkpoint or browser backup**.

File selection is explicit: the website cannot silently read arbitrary folders or edit your originals. Browser projects accept up to 250 text files, 256 KiB per file and 4 MiB per selection. Secret filenames, detected private keys, binary files and generated folders are filtered; review your selections because filtering cannot recognize every secret.

## Where does my work stay?

| | Live browser app | Full local application |
| --- | --- | --- |
| Processing | Rust engine compiled to WebAssembly, running on your device | Rust CLI and authenticated loopback API |
| Project access | Only files you explicitly select; read-only copies | Local filesystem indexing with configured exclusions |
| Sessions and recovery snapshots | This browser profile's IndexedDB | Local SQLite database |
| AI API keys | None; no provider connections | None for core tools; optional providers require separate setup and consent |
| Move to another device | Explicit checkpoint or backup export/import | Explicit exports or local database backups |

**Your project files, sessions and checkpoints are not uploaded to Cloudflare or automatically synced to GitHub.** Cloudflare serves the public application assets. The browser workspace makes no AI-provider or localhost API requests and includes no third-party analytics. The local API remains restricted to its own loopback origin and requires a private connection code.

Browser data is specific to the current browser profile and site origin. A different browser, device, or local preview starts with a separate workspace; it does not mean your original data disappeared. The hosted app does not automatically import your native workspace.

**Storage is not encrypted at rest.** Someone using the same browser profile can read browser data. Clearing site data or browser storage eviction can remove it. Native SQLite storage is also unencrypted. Export backups, keep them private, and never commit backups or checkpoints containing private work. Integrity checks detect damaged or altered checkpoint contents; they do not encrypt the contents.

Backup imports are validated locally before saving. Invalid or conflicting browser backups are rejected without replacing existing records. Importing a native checkpoint creates a browser continuation session; it cannot restore original project files or the native application's SQLite recovery snapshot. Restoring a checkpoint changes ContextPilot session state, not your source files or Git commits.

See [privacy details](docs/privacy.md), [security](SECURITY.md), and [deployment architecture](docs/deployment.md).

## Run it locally without API keys

Clone the public repository first:

```sh
git clone https://github.com/CharlieJamesGwapo/contextpilot.git
cd contextpilot
```

### Option A: browser workspace on your computer

Requires Node.js 22 or 24 with npm. The prebuilt Rust WebAssembly assets are included; you do not need a Rust server or an AI provider.

```sh
cd apps/dashboard
npm ci
npm run build:hosted
node scripts/serve-hosted.mjs
```

Open **http://127.0.0.1:4388/**. Keep the terminal running; stop it with Ctrl+C. This uses browser-local storage, separate from the live site's storage. Use explicit backup export/import to move work between those origins.

### Option B: full Rust CLI and local dashboard

Requires stable Rust, a C compiler for bundled SQLite, and Node.js 22 or 24 with npm. From the repository root:

```sh
cargo build --release --locked
cd apps/dashboard
npm ci
npm run build
cd ../..
./target/release/contextpilot doctor
./target/release/contextpilot init
./target/release/contextpilot dashboard
```

Open the loopback address printed in the terminal. Read the private connection-code file at the location announced by ContextPilot, then paste its contents into the local dashboard's connection form. **This code protects local workspace access; it is not an AI API key.** The default port is 4317; use the actual address and file path printed by your process. Never share the connection code or put it in a URL.

On Windows, use `target\release\contextpilot.exe` in place of `./target/release/contextpilot`. Stop the server with Ctrl+C. The Rust process serves the prebuilt dashboard, so no Node server is needed at runtime. When moving the binary, pass `dashboard --assets /absolute/path/to/apps/dashboard/out`. To install the CLI locally, run `cargo install --path crates/contextpilot-app --locked`.

The native runtime has been verified on macOS Apple Silicon. Intel macOS, Linux x86_64 and Windows x86_64 have been cross-compiled and linked; runtime and native-vault behavior on those platforms remain untested. See [platform verification](docs/platform-verification.md).

## Frontend development and static hosting

```sh
cd apps/dashboard
npm ci
npm run dev             # development UI; full workflows need the Rust local API
npm run build           # full local UI in out/
npm run build:hosted    # private browser workspace in hosted-out/
```

For complete local workflows, use the Rust-served production dashboard as described above. The Next.js development server alone does not proxy the Rust API; same-origin authorization is intentionally enforced. Hosted builds use a separate export directory, preserving the local `out/` build. No public backend URL or secret hosting environment variables are required.

Cloudflare Pages settings: root `apps/dashboard`, **Next.js (Static HTML Export)**, build `npm run build:hosted`, output `hosted-out`, Node 24. Direct Upload can deploy the reviewed static directory without granting access to other GitHub repositories. Use an assigned free `*.pages.dev` address, not a purchased domain. See [deployment instructions](docs/deployment.md) for exact procedures, limitations and Vercel compatibility settings.

## Local workflow

```sh
contextpilot tokens fixtures/task.md
contextpilot tokens task.md --provider openai --model gpt-4o
contextpilot optimize task.md --output optimized-task.md
contextpilot project add . --name 'My project'
contextpilot project index --root .
contextpilot retrieve 'login error' --root . --max-files 6
contextpilot session create 'Fix login' --requirement 'Test auth' --constraint 'Never deploy' --limit 32000
contextpilot project link SESSION_ID PROJECT_ID
contextpilot session retrieve SESSION_ID 'login error'
contextpilot context status --session SESSION_ID
contextpilot compact --session SESSION_ID --preview
contextpilot compact --session SESSION_ID --commit --revision PREVIEW_REVISION
contextpilot checkpoint restore CHECKPOINT_ID --yes
contextpilot handoff --session SESSION_ID NEW_DIRECTORY
contextpilot tool-output build.log --exit-status 1 --output complete-output.txt
```

File exports refuse existing destinations. Counts distinguish exact supported text-encoding counts, heuristic estimates and provider-reported usage. `tokens DIRECTORY` totals all eligible complete UTF-8 files (at most 512 KiB each), reports files analyzed and explicit coverage, and fails instead of returning a partial total if repository scan limits are exceeded. Directory tokenization, retrieval and indexing apply effective project overrides when the selected canonical root exactly matches a registered project, otherwise global settings. Exact text counts exclude message framing, tools, images and provider-internal tokens. Unknown models fall back explicitly rather than guessing. Smaller prompts do not establish lower end-to-end cost or better quality.

Repository selection respects Git ignore rules, `.contextpilotignore`, configured exclusions and secret/generated-source filters. Metadata caches validate current hashes. Symbols/imports inform deterministic ranking. Parsable Rust selects the complete containing function; other languages or unparseable Rust return complete files with an explicit fallback note. CLI/project excerpt budgets visibly mark omissions. Retrieval scans are capped at 20,000 files / 128 MiB. Reattaching identical source reuses it; changed/deleted source is removed with recovery snapshots.

Compaction preserves requirements, failures, decisions and verification evidence while reducing safe repetition and recognized routine compiler progress. `tool-output` applies the configured output target conservatively: all other redacted lines and exit status survive even when the result exceeds that target. The complete redacted tool output can be saved privately. No command is executed by that feature.

## Settings, evidence and reports

```sh
contextpilot config --file settings.json
contextpilot project config PROJECT_ID --file project-overrides.json
contextpilot models --file model-metadata.json
contextpilot price pricing.json
contextpilot import-usage normalized-usage.json
contextpilot evaluate evaluation-evidence.json
contextpilot report --output report.json
contextpilot instructions --output AGENT-INSTRUCTIONS.md
contextpilot schema
contextpilot backup backup.sqlite
contextpilot restore-backup backup.sqlite NEW_DIRECTORY
```

Global/project settings cover budgets, overhead, text tokenizer, exclusions, storage/privacy and efficiency profiles. File counts, CLI/project excerpt-token budgets and inline report limits are enforced. Tool-output targets drive conservative owned-output processing; protected content can exceed the target. Verbosity/subagent/verification preferences become exported guidance, not control over external agents. Global storage/external disables cannot be bypassed by project overrides.

Usage imports require provenance and stable identity and reject conflicting duplicates. Analytics group usage by project/session/provider/model/day, keep cache/reasoning subsets separate, separate optimization overhead and show unknown pricing explicitly. Quality/retry comparisons require supplied controlled baseline/optimized evidence; they are not self-generated experiments or causal proof.

## Optional providers

Set `OPENAI_API_KEY` or `ANTHROPIC_API_KEY` in the backend environment, or store a key from a secret manager through `providers credentials PROVIDER --stdin`. Interactive terminal input is refused to avoid echoed secrets. Keys are never command arguments or dashboard fields. Native vault storage has no plaintext fallback; environment values take precedence.

Enable `external_enabled:true` through configuration, then explicitly opt in:

```sh
contextpilot session run SESSION_ID --provider openai --model YOUR_MODEL --allow-external
contextpilot optimize task.md --mode deep --provider openai --model YOUR_MODEL --allow-external
contextpilot providers count anthropic --model YOUR_MODEL --file task.md --allow-external
```

Generation/deep/count send selected redacted content externally. Anthropic supports remote counting; OpenAI counting is explicitly unsupported. Deep candidates must pass independent conservative local validation. Provider discovery reports environment presence and an unqueried vault, not a successful connection. No live paid provider request has been verified. An isolated nonsecret macOS Apple Silicon vault roundtrip passed and confirmed deletion; existing provider credentials were never read. Linux/Windows and Intel macOS native-vault runtime checks remain pending. See [provider integrations](docs/provider-integrations.md).

## Data and privacy

Default storage is `.contextpilot/state.sqlite`; override with global `--data-dir` or `CONTEXTPILOT_DATA_DIR`. Managed sessions intentionally retain redacted context/recovery snapshots. Prompt-body logging defaults off; enabling `prompt_logging` explicitly stores redacted optimizer inputs/outputs. Event retention defaults to 30 days; sessions/checkpoints persist until explicit deletion. Set `managed_session_storage:false` to disable new managed writes. Backups/exports can contain private context, and SQLite is not encrypted at rest. No telemetry.

The API binds loopback, validates Host/Origin and requires bearer authentication. Private files use owner-only Unix permissions or protected Windows ACLs. No external assistant private sessions are accessed or modified. Manual handoff files provide the integration boundary. See [privacy](docs/privacy.md) and [security](SECURITY.md).

## Verification and development

```sh
cargo test --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --release --locked
cd apps/dashboard
npm run typecheck
npm run lint
npm run build
npm audit
npm test
npm run build:hosted
npm run test:hosted
npm run test:browser
python3 ../../scripts/security-check.py --staged
```

`python3 scripts/acceptance.py target/debug/contextpilot` exercises local persistence and backup recovery. `python3 scripts/demo.py target/debug/contextpilot` creates an isolated, clearly fictional demo without fabricated provider usage. JSON Schemas derive from Rust types; they are not a full OpenAPI specification. Deterministic HTTP mocks verify adapters without keys. Current evidence and external runtime/release prerequisites are recorded in [verification](docs/verification.md) and [implementation](docs/implementation.md).

MIT license. The public source repository and static hosting release contain application code and synthetic fixtures, not personal workspace records. No paid plan or service is required for the full local application. Further documentation: [CLI](docs/cli-reference.md), [API](docs/api.md), [architecture](docs/architecture.md), [compaction/recovery](docs/context-compaction.md), and [troubleshooting](docs/troubleshooting.md).

## Repository publication and PHP 0 operation

Real provider credentials belong only in the local backend environment; `.env.example` contains comments, and the app does not automatically load it. Never put AI keys in `NEXT_PUBLIC_*` variables. The sole hosted build flag is the non-secret `NEXT_PUBLIC_CONTEXTPILOT_MODE=hosted`.

Personal screenshots/reports, `.contextpilot`, databases, bearer codes, private environment files, exports, logs, caches and build artifacts are ignored. Synthetic test fixtures remain in source. Keep private data backups outside the Git staging set. The source scanner prints locations rather than secret values.

GitHub Actions checks are manual (`workflow_dispatch`) so pushing this repository does not automatically start hosted checks. Run the full checks locally at PHP 0; starting a hosted workflow may consume your account's included minutes. No paid runner, hosted database, analytics, telemetry or provider API is enabled. Cloudflare Pages static hosting uses a limited free tier; current limits and eligibility must be rechecked for later releases.

## Contributing and license

Feedback, bug reports and contributions are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md), use synthetic fixtures when testing, and do not include credentials or private project context in issues or pull requests.

ContextPilot is open source under the [MIT license](LICENSE). Core local tools need no paid AI API; optional external providers may charge separately when explicitly configured and used. The live app uses Cloudflare Pages static hosting, subject to its free-tier limits.

ContextPilot remains a working name; it has not received trademark clearance. See the [naming review](docs/naming-review.md).
