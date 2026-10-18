# Provider integrations

Independent text-only adapters support OpenAI Responses generation and Anthropic Messages generation/counting. No model name, current price or commercial context limit is bundled. Discovery makes no network requests and does not query native credential stores.

## Credentials and consent

Backend environment variables `OPENAI_API_KEY` and `ANTHROPIC_API_KEY` take precedence over native secure storage. `providers credentials PROVIDER --stdin` writes a key to macOS Keychain, Windows Credential Manager or Linux Secret Service through `keyring` 4.2; `--remove` deletes that vault entry without changing the environment. Unsupported/locked/denied vault access returns a safe error with no plaintext fallback. Credential stdin must be noninteractive to avoid echoing keys in a terminal. Keys are never supplied as command arguments or returned to dashboard clients. Discovery labels only environment presence and `vault_status:not_queried`; this is not authenticated connection status.

Generation, deep optimization and remote counting require `external_enabled:true` and explicit consent for the selected content. Merely configuring a key/permission sends nothing. Adapters apply best-effort secret redaction, fixed official endpoints, request timeouts, no redirects and bounded response bodies. They do not automatically retry paid operations. An explicitly opted-in macOS Apple Silicon native-vault integration check created/read/deleted a unique nonsecret probe and confirmed cleanup without reading existing provider entries. Other OS vault runtimes remain unverified. Live provider authentication/model availability has not been exercised; routine tests use mocks.

## Interfaces

`ProviderClient::new(Duration)` creates the HTTP client. `generate(provider,model,prompt,max_output_tokens,allow_external)` returns text and optional normalized reported usage. `count_tokens(provider,model,text,allow_external)` supports Anthropic only and returns `{input_tokens,measurement:"provider_counted"}`. OpenAI counting explicitly returns unsupported before credential lookup. Counts describe outbound-redacted text.

OpenAI uses `POST https://api.openai.com/v1/responses`, bearer authentication, text `input`, `max_output_tokens` and `store:false`; it reads message `output_text`. Anthropic uses `POST https://api.anthropic.com/v1/messages`, `x-api-key`, `anthropic-version:2023-06-01`, a user text message and `max_tokens`; it reads text blocks. Anthropic counting uses `POST https://api.anthropic.com/v1/messages/count_tokens` with model/messages and no generation limit. Generation responses are capped at 4 MB; count responses at 64 KB; text requests at 1 MB. Truncated/nonfinal generations are rejected when the provider reports them.

Capabilities explicitly distinguish generation, reported/cache/reasoning usage and provider token counting. Streaming, tool execution, multimedia input, external private-session discovery/mutation and interception of coding assistants are unsupported. Exported continuation/instruction files are manual handoff artifacts.

## Usage semantics and optimization

OpenAI input includes cached tokens; reasoning is a subset of output. Anthropic normalized total input is ordinary input plus cache-read and cache-creation input; these cache subsets are retained separately. Never add subsets again to totals. Missing usage remains unknown. User imports must already follow these normalized semantics and include provenance/stable identity. Cache-write counts lack duration-specific billing rates, so current monetary estimates leave those cases unpriced.

Deep optimization sends preservation instructions and accepts only the independently verified conservative local transformation. Arbitrary semantic rewrites are rejected. Provider optimization overhead is recorded separately; projected net accounting is shown only where measurements are comparable. This does not establish end-to-end task savings or quality improvement.

Local mock HTTP tests verify serialization, consent, auth/rate/upstream errors, malformed responses, bounded parsing, timeout, usage normalization and token counting without real keys. Paid live smoke tests require a valid credential and explicit authorization.

Official interfaces consulted on 18 October 2026:

- [OpenAI Responses API](https://developers.openai.com/api/reference/resources/responses/methods/create)
- [Anthropic Messages](https://platform.claude.com/docs/en/api/messages/create)
- [Anthropic token counting](https://platform.claude.com/docs/en/api/http/messages/count_tokens)
- [Anthropic cache accounting](https://platform.claude.com/docs/en/build-with-claude/prompt-caching)
- [Native keyring API and platform stores](https://docs.rs/keyring/4.2.0/keyring/v1/index.html)
