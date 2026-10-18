# Real backend browser regression

From the repository root, build the Rust binary with `cargo build`. Then from
`apps/dashboard`, run `npm ci` followed by `npm test`.

The test builds the static dashboard and starts the real Rust dashboard server
on a dynamically assigned loopback port with a new temporary data directory.
It reads the generated token file privately, exercises the browser UI without
API mocks, stops the server, and removes its temporary data. Provider keys are
removed from the child process environment, external calls stay disabled, and
the suite makes no paid provider calls. Traces, screenshots, and videos are off
to keep authentication values out of test artifacts.

Set `CONTEXTPILOT_BINARY` to use another built binary. Set `CHROMIUM_PATH` to a
Chromium executable. On macOS an installed Google Chrome is detected; otherwise
install Playwright Chromium with `npx playwright install chromium`.

The suite covers empty state and connection, local prompt optimization and
constraint preservation, editable output and original restoration, session
creation, duplicate context messages, context budgets, checkpoint preview/save
and confirmed restore, settings persistence after reload, authenticated API
requests, no external requests, browser/network errors, and all six tabs at a
390px mobile width. It runs one headless worker.

## Hosted fallback acceptance

Run `npm run build:hosted` then `npm run test:hosted`. This starts a static-only test server with no Rust process, checks the hosted-mode privacy boundary even with a seeded synthetic connection code, and exercises all seven navigation sections at eight responsive widths. It also checks setup clipboard, theme, history, focus and SVG assets.

Set `HOSTED_URL=https://YOUR_PROJECT.pages.dev` to run the same browser tests against an actual deployment. This does not upload files or local workspace data. The private repository contains only a genuine hosted-mode screenshot, not screenshots of user sessions.
