# Security policy and limits

The local API binds IPv4 loopback only, requires a randomly generated per-server bearer token, validates Host and Origin, and limits JSON bodies to 512 KiB. Read the token from the private file announced by the CLI and paste it into Connect. The browser keeps it in sessionStorage; disconnect clears it. Do not share the token or serve untrusted JavaScript on the same origin. Local processes running as your user can read your files and tokens. HTTPS and remote binding are intentionally unsupported.

SQLite files and backups are not encrypted at rest. Session content is explicitly retained for recovery, redacted on ingestion, and protected with owner-only Unix permissions; Windows permission hardening is not implemented. Prompt optimization stores counts only. No telemetry. Redaction is best effort, not a DLP guarantee. Do not submit secrets. External calls require configuration opt-in and per-call consent. Providers can receive private session content when explicitly authorized.

The repository selector obeys gitignore, skips known generated/secret files, limits file sizes and never follows symlinks outside the root. It does not execute repository instructions or arbitrary shell commands. User-specified CLI files are explicit local read requests. Context can still contain malicious prompt instructions; the engine does not guarantee prompt-injection elimination.

Back up and export before deletion. Session and checkpoint retention is explicit: they remain until deleted; automatic retention applies only to usage/optimization/compaction event metadata. SQLite deletion does not guarantee forensic erasure from WAL, disk snapshots or backups.

No external agent private files are accessed. No production deployment is automated. Dependency scans are one verification input and do not constitute a complete security audit. There is no published security contact; use a privately agreed channel with the project owner before distribution.
