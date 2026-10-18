# Static hosting and browser-local architecture

ContextPilot on Cloudflare Pages is a functional browser workspace. The actual Rust core is compiled to WebAssembly for token estimates, conservative prompt optimization, session validation and integrity-checked checkpoint semantics. IndexedDB stores selected project contents, sessions, checkpoints and recovery snapshots on the user's device. No project content is stored on Cloudflare.

| Capability | Browser workspace | Native local application |
| --- | --- | --- |
| Prompt optimization and estimates | Rust WASM on this device | Same Rust core |
| Project context | Explicitly chosen files; browser-local search | Direct filesystem indexing |
| Sessions and checkpoints | IndexedDB with explicit JSON exports/imports | SQLite with recovery snapshots |
| AI services | No connections or API keys | Optional, with configuration and consent |
| Synchronization | Manual backup/checkpoint files | No automatic cloud sync |

The hosted origin never connects to localhost or reads a native connection code. The existing loopback API authentication remains intact. CSP allows only same-origin application/WASM requests; no analytics, external scripts, provider calls, workers, cloud databases or storage bindings are configured. Browser storage is not encrypted at rest and is accessible to people using the same browser profile. Clearing site data removes it, so the UI provides explicit backup export/import. Downloads can contain private context; keep them private. Selected file access is read-only and does not change Git repositories.

## Builds

From `apps/dashboard`:

```sh
npm ci
# Rebuild the checked-in WASM assets after Rust changes: ../../scripts/build-browser-core.sh
npm run build          # full local UI -> out/
npm run build:hosted   # browser workspace -> hosted-out/
```

`build:hosted` sets only the non-secret `NEXT_PUBLIC_CONTEXTPILOT_MODE=hosted` value. Do not configure any provider keys in the hosting dashboard. Runtime hostname checks select browser storage on non-loopback origins even if a normal build is accidentally hosted.

The existing Rust process serves `out/`; `hosted-out/` is separate so a hosted build does not replace the local application's static export. Only the contents of `hosted-out/` may be uploaded. It contains no API Functions. The `public/_headers` file adds static hosting protections on Cloudflare Pages; Rust retains its own existing security behavior.

## Cloudflare Pages (chosen free static target)

Use the Charlie account and a new ContextPilot project only. Do not edit unrelated projects or domains. Do not purchase a domain. Use the assigned `*.pages.dev` hostname; the actual name depends on availability.

For Git integration, use repository root directory `apps/dashboard`, framework **Next.js (Static HTML Export)**, build `npm run build:hosted`, output `hosted-out`, Node 24. Keep environment variables empty unless the non-secret mode flag is supplied directly. Keep Web Analytics and paid add-ons disabled. Do not add Workers, Functions, databases, storage bindings or custom domains.

For a reviewed direct upload, build locally, verify the complete output, and use a pinned Wrangler CLI with metrics disabled:

```sh
WRANGLER_SEND_METRICS=false npx wrangler@4.151.1 pages project create contextpilot --production-branch main
WRANGLER_SEND_METRICS=false npx wrangler@4.151.1 pages deploy hosted-out --project-name contextpilot --branch main
```

Authenticate interactively outside the repository; never paste tokens into source or logs. Check the selected account before project creation. Direct Upload uploads only static assets and does not grant Cloudflare access to other GitHub repositories. A Direct Upload project cannot later be switched to Git integration; that requires a new project. No automatic deployments or private GitHub Actions jobs are enabled by this repository.

Cloudflare's current free Pages limits include 500 builds per month, one concurrent build, 20,000 files per site and a 25 MiB per-file maximum. Static assets do not require paid Functions. Check current provider terms and limits before each release: [Pages limits](https://developers.cloudflare.com/pages/platform/limits/), [pricing](https://developers.cloudflare.com/pages/functions/pricing/), [Direct Upload](https://developers.cloudflare.com/pages/get-started/direct-upload/). This is a limited free tier, not a promise of unlimited eligibility or future pricing.

## Vercel compatibility (not deployed for commercial use)

The correct Vercel root is `apps/dashboard`. Its checked-in `vercel.json` specifies Next.js, `npm ci`, `npm run build:hosted` and `hosted-out`. No Rust process or SQLite store can be supplied by this static deployment.

The requested commercial use is not eligible for Vercel Hobby: [current Hobby policy](https://vercel.com/docs/plans/hobby). A trial is not a permanent PHP 0 solution. No Vercel project, upgrade or deployment is authorized by that configuration alone. Use Cloudflare Pages for this release; do not activate a paid Vercel plan.

## Publication privacy and checks

The private GitHub repository contains code, synthetic fixtures, tests, public SVG marks and documentation. Git ignores local SQLite files, bearer codes, private environment files, metadata caches, logs, build artifacts and locally preserved screenshots/reports containing personal workspace progress. No personal checkpoints or portfolio source are included.

Before pushing run `python3 scripts/security-check.py --staged`, inspect `git diff --cached`, and scan built assets. This is best-effort scanning, not a proof that arbitrary secrets cannot exist. Run Rust tests/format/Clippy, frontend lint/typecheck/local acceptance, the hosted-mode acceptance test, dependency audits and both production builds. Verify actual deployment assets and browser behavior before reporting success.
