# Verification and delivery status — 2026-10-18

ContextPilot is a functional local CLI, shared Rust engine and authenticated web dashboard. This continuation implements the remaining offline MVP requirements and verifies the optional provider interfaces without paid requests. MIT was explicitly selected by the owner. No deployment, publication, remote push or remote CI run occurred. The working name remains provisional.

## Implemented and verified

- Bundled model-specific OpenAI text tokenizers with explicit model mappings and an uncertainty-labeled fallback. Text tokenization excludes framing, tools, multimedia and provider-internal tokens; it is not billed usage. Directory estimates count every eligible full file, not the retrieval shortlist.
- Conservative offline prompt optimization, byte-preserved code/CRLF, preservation of requirements and safety repetition, transparent before/after accounting. Deep candidates outside independently validated transformations remain separately labeled proposals for manual review.
- Configured model/context metadata, response reserve, known system/tool overhead, warning/compaction/emergency thresholds, optional automatic compaction and emergency stops.
- Versioned SQLite storage, original-version migration, transactional writes, revision conflicts, concurrent access, retention, session/event deletion, JSON export including project settings/source references, consistent backup/restore and original recovery snapshots.
- Projects and session associations, partial project policies, globally enforced privacy ceilings, explicit managed-session storage and opt-in prompt logging with redaction.
- Persistent metadata-only repository indexes: current content hashes, symbols/imports/dependencies, deterministic ranking, ignores/exclusions, secret/generated/binary exclusions, safe cache invalidation and exact references. Parsable Rust retrieval retains complete containing functions; other/unparseable files use an explicit full-file fallback. File/token policies report omissions. Scans fail above 20,000 eligible files or 128 MiB rather than returning partial totals; actual reads are bounded to 512 KiB per file.
- Source attachment reuse, stale-source replacement and recovery. Concurrent source changes cannot retain conflicting selected versions. A separately reproduced race was fixed and checked with 100 concurrent attachment attempts.
- Structured, checksummed checkpoints preserve supplied changed files, commands, environment, approvals, exclusions, assumptions and recovery facts. Preview is default; stale commits fail; restores obey both current and recovered project privacy policies.
- User pricing/model registries with provenance/dates, cached-read/write pricing, currency separation and unknown-cost counts. Historical usage imports have stable identities and explicit imported measurement/provenance.
- Analytics group project/session/provider/model/day/measurement, account for optimization overhead on comparable bases and retain negative projected net outcomes. Controlled baseline/optimized quality/retry pairs require matching declared conditions; supplied evidence does not establish causal improvements or statistical significance.
- Configured efficiency instructions and supported handoff packages. Owned tool-output processing reduces recognized routine build progress only; it preserves every other redacted line and exit status even above the display target. Complete redacted output can be exported privately. No arbitrary commands are executed.
- Native OS credential-store support with environment precedence and safe errors. Discovery does not read saved keys. Noninteractive stdin avoids echoed/argument credentials.
- OpenAI Responses and Anthropic Messages adapters, explicit capabilities, normalized usage, bounded/time-limited HTTP requests and two layers of external-send opt-in. Anthropic remote counting has mock contract coverage and requires consent. No real account was contacted.
- Authenticated loopback API with Host/Origin validation, 512 KiB request limit, worker-thread storage/index work and generic private errors. JSON Schemas derive from Rust types and are exported in [api-contract.json](api-contract.json); this is not a complete OpenAPI document.
- Static Next.js dashboard uses real engine state for projects, sessions, optimization, retrieval, usage imports, controlled evidence, prices, models and policy. Empty/error/disconnected states and mobile workflow are exercised.

## Executed checks

| Check | Actual result |
| --- | --- |
| `cargo test --workspace --locked` | **72 passed**, no failures: core 32, app/storage/API/analytics 31, providers 9. One native-vault test is intentionally ignored in the ordinary suite and explicitly executed separately. Doc tests pass. |
| Explicit `cargo test -p contextpilot-providers --test native_vault -- --ignored` | **1 passed** on macOS arm64: unique non-secret native Keychain entry created/read/deleted, deletion confirmed. Existing provider entries were never read or changed. |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed, warnings denied |
| `cargo build --release --locked` | Passed on macOS arm64, Rust 1.99.1 |
| Release CLI acceptance | Passed: init, file/directory tokenization, optimize, sessions, budgets, compaction/restore, persistence, backup recovery, export/delete, effective tokenizer/exclusions and bounded retrieval |
| Dashboard lint / strict TypeScript / static production build | Passed, Next.js 16.4.2 |
| Real-engine Playwright against release binary | **3 passed**: original durable session/recovery workflow, connection-code help/error recovery, and extended project/retrieval/import/evaluation/policy workflow; no mocked engine or paid calls |
| Mobile browser coverage | Original six tabs fit a 390 px viewport; workflow checks no browser errors, failed requests or unintended external requests |
| Rust dependency audit | **0 vulnerabilities**, 305 lockfile dependencies. Three informational maintenance advisories affect transitive Linux keyring components (`zbus_names`, `zvariant`, `zvariant_derive`), whose upstream functionality is being merged into zbus 6; these are not vulnerability findings. No advisory suppression was added. |
| Full npm audit, including development dependencies | **0 vulnerabilities**; obsolete Next ESLint wrapper removed in favor of direct recommended JavaScript/TypeScript/Hooks/accessibility rules |
| Source credential-pattern scan | Passed; best-effort known-pattern scan, not a complete security audit |
| Intel macOS / Linux x86_64 / Windows x86_64 | Full workspace builds and binary links passed; file formats checked. [Platform evidence](platform-verification.md) distinguishes compilation from execution. |
| Packaged macOS runtime | Freshly extracted bundle CLI/API/static-asset smoke check passed; Rust and Node are not needed to run it |

Browser tests launch a fresh real loopback server on a random port, read its private token file, remove provider environment credentials, disable tracing/video and clean up temporary data. Native-vault verification uses an isolated non-secret service name and is opt-in. Dependency notices include available upstream licenses for build and alternate-platform dependencies; see `THIRD-PARTY-NOTICES.txt`.

## Genuine external validation/release gates

1. **Live provider verification:** an authorized account credential, available model and explicit paid-call consent are required. Deterministic HTTP adapters/counting are tested; account authentication and real billing were not exercised. Saved provider vault entries were deliberately not inspected.
2. **Other OS runtime validation:** Linux, Windows and Intel macOS execution/keychain/permissions/installer behavior require those runtimes. Full compilation/linking passed. Rosetta is unavailable here; no Linux/Windows runtime was present. Native CI jobs are supplied.
3. **Distribution identity:** primary sources show existing ContextPilot projects in closely related areas. A distinct cleared name is needed before commercial launch; [naming review](naming-review.md) records evidence without claiming trademark clearance. Public signed/notarized releases additionally need the owner's release identity/certificates. The supplied local macOS bundle is unsigned.

These gates do not prevent using the local offline application.

## Explicit scope and accounting limits

Whole-file fallback is conservative where a safe syntax scope cannot be established; oversized excerpts are visibly omitted. Symbols/import relationships outside the Rust scope parser use deterministic heuristics. General semantic summarization, embeddings, a TUI, autonomous command execution, streaming, hosted infrastructure and automatic updates are optional extensions rather than required MVP dependencies. External agent concurrency/verbosity/verification preferences are instructions; ContextPilot does not mutate or control another product's private sessions.

Checkpoint facts and evaluation evidence must be supplied; the application does not infer complete command outcomes or task quality from arbitrary transcripts. Unknown model limits, prices, multimodal/internal/tool token categories remain unknown until supported evidence is provided. User-marked verified metadata is user-supplied provenance, not independent attestation. A shorter prompt alone does not prove lower total cost or better task quality.

Local content/backups are not encrypted. Redaction is best effort; same-user processes can access local files/tokens. Managed session/recovery content is retained explicitly until deletion; event retention has its own policy. SQLite/filesystem/backups may retain deleted bytes. JSON export is an inspection export; use SQLite backup for full raw snapshot recovery. Dependency scans and targeted reviews are not a claim of a complete security audit.

The original requirements remain in [product-specification.txt](product-specification.txt). [files-changed.txt](files-changed.txt) lists deliverable source files; build caches and test databases are excluded from archives.

## Interface refresh

The subsequent local dashboard redesign is recorded in [ui-refresh.md](ui-refresh.md), including updated wording, branding, responsive checks and three passing real-engine browser workflows.
