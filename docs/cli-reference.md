# CLI reference

`contextpilot --help` and command-specific `--help` are authoritative for argument syntax. Results are JSON except help, errors, startup diagnostics and explicitly exported files. Exit codes: success 0, runtime/input failure 1, Clap argument errors 2. Global `--data-dir PATH` defaults to `.contextpilot` and supports `CONTEXTPILOT_DATA_DIR`; `--json` is accepted for scripting compatibility.

## Offline analysis and projects

```sh
contextpilot tokens task.md
contextpilot tokens --stdin --provider openai --model gpt-4o
contextpilot optimize task.md --mode quick --output NEW_FILE
contextpilot optimize --stdin
contextpilot tool-output build.log --exit-status 1 --output COMPLETE_REDACTED_FILE
contextpilot retrieve 'authentication failure' --root . --max-files 6
contextpilot project add . --name 'My project'
contextpilot project list
contextpilot project index --root .
contextpilot project config PROJECT_ID
contextpilot project config PROJECT_ID --file project-overrides.json
contextpilot project link SESSION_ID PROJECT_ID
contextpilot session retrieve SESSION_ID 'authentication failure' --max-files 6
```

`tool-output FILE [--exit-status N] [--output NEW_FILE]` processes existing text without executing commands. Above the configured target, only recognized routine compiler progress can be reduced (at least ten such lines); other redacted content/status is preserved even if still oversized, with complete redacted file export available.

Tokenization supports exact, explicitly mapped OpenAI text encodings and labeled heuristic fallback for unknown models. It excludes request framing and provider-internal tokens. `tokens DIRECTORY` sums all eligible complete UTF-8 files, returning `files_analyzed`, measurement/method and `scope:eligible_full_files` coverage. Ignore/secret/generated exclusions and the 512 KiB per-file ceiling define eligibility. Exceeding scan limits returns an error rather than a partial total. Repository scanning is capped at 20,000 files / 128 MiB. Parsable Rust excerpts select complete containing functions; other languages/unparseable Rust return complete files with explicit fallback notes. Retrieval respects Git ignore rules, `.contextpilotignore` and configured exclusions. CLI/project/session retrieval applies file-count and aggregate excerpt-token limits; omitted excerpts are explicitly marked. Directory tokenization, `retrieve --root` and `project index --root` use effective project settings when the canonical root exactly matches a registered project, otherwise global settings.

## Sessions and recovery

```sh
contextpilot session create 'Fix login' --requirement 'Test auth' --constraint 'Never deploy' --issue '401 response' --next 'Reproduce' --limit 32000
contextpilot session list
contextpilot session show SESSION_ID
contextpilot session add SESSION_ID --text 'Reproduction failed' --category failure
contextpilot context status --session SESSION_ID
contextpilot compact --session SESSION_ID --preview
contextpilot compact --session SESSION_ID --commit --revision PREVIEW_REVISION
contextpilot checkpoint create --session SESSION_ID
contextpilot checkpoint list
contextpilot checkpoint restore CHECKPOINT_ID --yes
contextpilot checkpoint export CHECKPOINT_ID NEW_FILE
contextpilot handoff --session SESSION_ID NEW_DIRECTORY
contextpilot instructions --output NEW_FILE
```

Repeat requirement/constraint/issue/next flags as needed. Append categories include context, failure, decision, verification, requirement, tool and assistant. `checkpoint create` applies a recoverable checkpoint; preview itself writes nothing. Handoff exports `checkpoint.json`, `continuation.md` and `AGENT-INSTRUCTIONS.md` without modifying an external assistant. Instruction exports are guidance, not an installed external agent hook.

## Provider operations

```sh
contextpilot providers list
contextpilot providers status
contextpilot providers configure
contextpilot providers credentials openai --stdin
contextpilot providers credentials openai --remove
contextpilot providers count anthropic --model YOUR_MODEL --file task.md --allow-external
contextpilot session run SESSION_ID --provider openai --model YOUR_MODEL --max-output 2048 --allow-external
contextpilot optimize task.md --mode deep --provider openai --model YOUR_MODEL --allow-external
```

Credential storage reads the key from noninteractive standard input, never a command argument; an interactive terminal is refused to avoid echoed secrets. Use an interactive secret-input tool or your secret manager; avoid typing literal keys into shell history. Environment keys override the native vault. Discovery does not query the vault or test authentication. External count/run/deep require both `external_enabled:true` and per-command opt-in. Anthropic supports provider counting; OpenAI returns unsupported for that operation.

## Configuration, evidence and administration

`config [--file JSON]` reads/replaces global settings; project configuration applies a partial override. `models [--file JSON]` manages user-supplied dated model limits. `price JSON` adds dated pricing. `import-usage JSON` imports normalized usage with `source` and stable `external_id`; repeating an identical event is idempotent. `evaluate JSON` records existing controlled quality/retry evidence. `report [--output NEW_FILE]` summarizes local metadata; a file export preserves the complete report when the inline profile limit is exceeded. `schema` emits JSON Schemas derived from Rust types.

Other commands: `init [PATH]`, `doctor`, `version`, `status`, `dashboard [--port N] [--assets PATH]`, `export NEW_FILE`, `backup NEW_FILE`, `restore-backup BACKUP NEW_DIRECTORY`, `clear-usage --yes`, and `session delete ID --yes`. File exports refuse existing destinations. No command executes user-supplied shell code.
