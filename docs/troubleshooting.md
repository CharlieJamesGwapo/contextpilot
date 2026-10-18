# Troubleshooting

- Missing Rust: install stable Rust through the official rustup installer and a platform C compiler. Node is only needed to build static assets.
- Dashboard 404: run npm ci and npm run build inside apps/dashboard, then supply --assets if the CLI has moved away from the source checkout.
- 401: read the current private token file announced by the running CLI and reconnect. Tokens are per server run and stored in browser sessionStorage.
- 403: use the exact http://127.0.0.1:PORT or localhost URL. Cross-origin browser requests and DNS rebinding hostnames are rejected.
- Port occupied/stale token: stop the old process or select another --port. Remove a stale token only after checking its server is stopped.
- No reduction: unique or safety-relevant context is deliberately preserved. A recovery checkpoint can still be useful.
- Stale revision: re-preview after any session append or restore; commit must name that preview's revision.
- Unknown cost/context limit: provide your own valid pricing/configuration. Public API prices do not imply subscription savings.
- Database busy: avoid long-running external database transactions; the application has a 5-second write timeout. Do not hand-edit records while the application is active.
- Provider failure: verify model availability in your own account, credentials, external_enabled and explicit per-call consent. Error text intentionally excludes raw provider bodies.
