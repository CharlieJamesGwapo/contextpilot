# ContextPilot product UX pass

Scope: improve the existing Next.js static dashboard over the unchanged Rust/Axum/SQLite engine. No migrations, external requests, publication, pushes, or user-project edits. Preserve existing workspace records and checkpoint contracts.

1. Audit: read instructions, API/domain models, UI and tests; run baseline tests, lint, typecheck, build, Rust format/clippy. Record source hashes and real record integrity without exposing content.
2. Brand/system: retain and refine original compass/brackets vector; consistent app icon, favicon and header. Compact forest/mint dark theme and accessible neutral light theme, persisted browser preference.
3. Navigation/home: real hash history, accessible narrow navigation, Projects section, real latest project/session/checkpoint and continue actions. Optional skippable local onboarding only for new workspaces.
4. Core workflows: searchable sessions; distinguish initial brief from dated saved updates; checkpoint metadata/inspection/full JSON plus continuation export; safe restore explanation. Preview file results before attachment, filter/select exports without pretending API supports arbitrary-file attachment. Retain optional provider and advanced tools.
5. Usage/guidance: actual context budget, source file/message contribution when measurable, original/output local estimates, no fabricated provider costs or savings.
6. Accessibility/responsive: keyboard/focus/landmarks/live messages, contrast checks, reduced motion and all eight requested widths.
7. Verification: isolated real-engine browser tests, Rust checks, frontend lint/type/build; compare original record hashes and source manifest; show running local app.

Design: calm, compact developer workspace. Clear next actions replace promotional decoration; plain-English labels with advanced details available on demand. All UI sanitization is display-only; exports and stored records retain original information.
