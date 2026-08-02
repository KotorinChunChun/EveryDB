# AGENTS.md

## Cursor Cloud specific instructions

EveryDB is a **Windows-only Tauri desktop app**: a Vite + TypeScript frontend (`src/`,
`index.html`) driving a Rust backend (`src-tauri/`) that caches many data sources into a
local SQLite DB and offers cross-source FTS search. Cloud agents run on Linux, so scope
matters — see below.

### What runs on this Linux VM: the frontend

The frontend is the runnable/testable part here. Standard commands live in `package.json`
(`dev`, `typecheck`, `test`, `docs:check`, `security:check`, `build`); don't duplicate them.
Notes:

- Run the frontend-only quality gate with `npm run check` (typecheck + vitest + docs:check +
  security:check + build). Do **not** run `npm run check:all` or `npm run tauri ...` on
  Linux — those invoke `cargo`/Tauri and will fail (see next section).
- `npm run dev` serves the UI on a **fixed port 1420** (`strictPort`), so only one dev
  server can run at a time.
- **Gotcha — no Tauri backend in a plain browser.** When the UI is opened in a normal
  browser (not the Tauri shell), every `invoke(...)` IPC call fails because
  `window.__TAURI_INTERNALS__` is absent. The bootstrap (`src/app/bootstrap.ts`) swallows
  these failures, so the UI still renders and purely client-side features work (opening the
  "追加"/add-source dialog, switching data-source types, dynamic form rendering, dialogs).
  Backend-dependent features (search, listing/saving/reloading sources, credential storage)
  will silently do nothing. This is expected on Linux, not a bug. Vitest covers frontend
  logic with the Tauri APIs mocked (`src/test/setup.ts`).

### What does NOT run on this Linux VM: the Rust/Tauri backend

`src-tauri/` targets Windows and **cannot be built on Linux**, so `cargo build/test/clippy`,
`npm run tauri dev/build`, and `npm run check:all` all fail here. Reasons:

- The unconditional `windows-future` crate dependency references Windows-only symbols
  (`windows_core::imp::IMarshal`, `windows_threading::submit`) and fails to compile on Linux.
- Runtime features are Windows-specific: DPAPI secret encryption (`src-tauri/src/secret.rs`),
  Windows Hello (`src-tauri/src/winauth.rs`), WebView2, tray icon, and ODBC/Access.

Build, run, and Rust tests for the backend must be done on Windows (the repo's dev docs use
PowerShell). The active Rust toolchain on this VM is set to `stable` (edition2024 is required
by locked deps), but the backend still won't compile on Linux for the reasons above.
