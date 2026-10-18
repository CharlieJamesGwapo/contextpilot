# Local API v1

Default base: `http://127.0.0.1:4317/api/v1`. Every route, including health/schema, requires `Authorization: Bearer <local-token>`. Host must be localhost:PORT or 127.0.0.1:PORT; browser Origin must match that Host. JSON bodies are bounded to 512 KiB. Typed request models reject unknown fields. Credentials are never returned. Authentication fails with 401, Host/Origin with 403; input/body errors use appropriate 400/413/415/422 responses, stale/storage operations generally 400, emergency generation 409, and provider failures 502. Error bodies do not expose upstream details.

## Routes

| Method | Path | Request/result |
| --- | --- | --- |
| GET | `/health` | Health and database schema version |
| GET | `/schema` | JSON Schemas derived from Rust domain types; not a complete OpenAPI document |
| GET | `/status`, `/export` | Persisted owned data, settings and provider discovery |
| GET, POST | `/projects` | List; register `{root,name?}` |
| GET, POST | `/projects/ID/config` | Effective settings; partial configuration override |
| POST | `/projects/ID/index` | Rebuild/validate ignored-aware metadata index using effective exclusions |
| POST | `/projects/ID/context` | `{query,max_files?}`; bounded current excerpts and explicit omissions |
| GET, POST | `/sessions` | List; create `{objective,requirements:[],constraints:[],unresolved_issues:[],next_steps:[],context_limit?}` |
| GET, DELETE | `/sessions/ID` | Read owned session; delete associated owned recovery data |
| POST | `/sessions/ID/project` | `{project_id:string|null}`; link/unlink registered project |
| POST | `/sessions/ID/context` | `{query,max_files?}`; attach current source, reuse identical excerpts, snapshot/remove stale versions |
| GET | `/sessions/ID/budget` | State, measured/estimated usage, reserve, capacity and remaining budget |
| POST | `/sessions/ID/messages` | `{text,category?}`; default category `context` |
| POST | `/sessions/ID/generate` | `{provider,model,max_output_tokens,consent:true}`; external generation |
| GET | `/checkpoints` | Structured owned recovery checkpoints |
| POST | `/checkpoints/ID/restore` | Restore pre-compaction state after saving current state |
| POST | `/tool-output` | `{text,exit_status?}`; conservative redacted output processing and complete status |
| POST | `/prompts/estimate` | `{text}`; uses configured tokenizer and returns labeled Estimate |
| POST | `/prompts/optimize` | `{text}`; conservative local Optimization |
| POST | `/prompts/deep` | `{text,provider,model,max_output_tokens,consent:true}`; externally proposed, locally validated optimization |
| POST | `/contexts/compact/preview` | `{session_id}`; no-write preview and source revision |
| POST | `/contexts/compact` | `{session_id,source_revision}`; transactional commit |
| GET | `/usage/summary` | Provider/imported totals, dated costs by currency and unknown-cost counts |
| GET, DELETE | `/usage/history` | Usage records; delete usage/optimization/compaction/evaluation/prompt-log event data |
| POST | `/usage/import` | Normalized UsageImport with provenance and stable event identity |
| GET | `/analytics` | Grouped metadata, gross/projected net optimization accounting and declared evaluation pairs |
| GET, POST | `/evaluations` | List; record supplied quality/retry/verification evidence |
| GET | `/providers` | Environment presence, vault `not_queried`, capabilities, unverified connection status |
| POST | `/providers/count` | `{provider,model,text,consent:true}`; Anthropic `{input_tokens,measurement:"provider_counted"}` |
| GET, POST | `/config` | Read/replace validated global Config; omitted fields use defaults |
| GET, POST | `/prices` | Dated user-supplied Price records with `verified:false` |
| GET, POST | `/models` | Dated user-supplied ModelMetadata with `verified:false` |
| GET | `/instructions` | Export efficiency guidance and explicit external-integration scope |

JSON provider identifiers are `open_ai` and `anthropic`. Generate/deep/count are the only external model operations; all require global/effective permission and per-request consent. API requests time out after 60 seconds, refuse redirects and do not hold the SQLite mutex during network calls. OpenAI counting returns unsupported. Counting operates on outbound-redacted text and includes no generation usage event.

## Data contracts

`schema`/`GET /schema` are the field-level source of truth. Domain schemas include Estimate, Optimization, Session, Config, Checkpoint, CompactionPreview, Budget, Price, ModelMetadata, UsageImport, RepositoryIndex and FileContext. Database schema version is 2; checkpoint schema version remains 1.

Estimate measurements distinguish `estimated` heuristic fallback, `tokenized_text` exact encoding counts for supported plain text, `provider_counted` remote request counts, and separately recorded `provider_reported`/imported generation usage. Framing and multimodal costs are excluded from offline encoding counts. Optimizations compare the same selected tokenizer and never imply causal task savings.

Config includes context/reserve/system/tool overhead, ordered warning/compact/auto/emergency thresholds, retention, external permission, prompt logging, managed storage, tokenizer provider/model, project exclusions and efficiency profile. Defaults: reserve 4096; overheads 0; thresholds 60/75/85/95; auto-compaction false; retention 30 days; external false; prompt logging false; managed storage true; tokenizer `local`/empty model. Require `0 < warning < compact <= auto < emergency <= 100`, positive known capacity and retention 1..36500 days. Project overrides cannot re-enable globally disabled external access or managed storage.

UsageImport requires provider/model, normalized total input/output tokens, source and external_id; cache-read/cache-write/reasoning subsets and project/session/date may be null/omitted. Input already includes cache subsets; output includes reasoning. Reimporting the same stable identity is idempotent, while conflicting content is rejected. `recorded_at`, if supplied, is RFC3339 and normalized to UTC.

Evaluation fields: task_id, variant (`baseline`/`optimized`), provider, model, evaluation_protocol, quality_score (0..1), retries (nonnegative integer), controlled (boolean), optional project_id/session_id, verification_status (`passed`/`failed`/`unverified`) and verification_evidence. Passed status requires nonempty evidence. Comparisons require exactly one declared controlled baseline/optimized pair with matching task/provider/model/protocol. These are supplied observations, not independently executed experiments or statistical proof.

Prices/model limits require date and source; no current commercial values are bundled. Monetary totals remain separated by currency. Missing category rates, including cache-write billing, remain unpriced.
