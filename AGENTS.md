# AGENTS.md

Context for AI coding agents working on **Asset Manager**: a **Tauri 2** desktop app with **React 19**, **TypeScript** (strict), **Vite 7**, and **SQLite** (bundled via `rusqlite`) in `src-tauri`. Human-oriented docs live in `README.md`.

## Prerequisites

- **Node.js 24** — repo pins `.node-version` and `package.json` `engines`; use `fnm use` (or equivalent) before installing deps.
- **Rust** — `rustup`-managed toolchain for `src-tauri`.
- **OS / Tauri** — follow [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) (WebView, etc.) for the host platform.

## Setup

```bash
fnm use          # or ensure Node 24
npm install
```

## Commands (from repo root)

| Command | Use |
|--------|-----|
| `npm run tauri:dev` | **Primary:** full app with hot reload; uses `tauri.dev.conf.json` so app data is under **`com.weaverb.assetmanager.dev`**, not production. Rust IPC available only here (not in a plain browser). |
| `npm run dev` | Vite only at `http://localhost:1420` — UI-only; Tauri `invoke` APIs unavailable. |
| `npm run build` | Production frontend: `tsc` + Vite → `dist/`. |
| `npm run tauri build` | Release bundle for current platform (runs `beforeBuildCommand` → `npm run build`). |
| `npm run test:rust` | Rust unit tests (`src-tauri`). |
| `npm run test:rust:coverage` | LLVM line coverage HTML under `target/coverage-rust/html/`; **fails under 80%** lines (per script). |
| `npm run fmt:rust` | Apply `rustfmt` to `src-tauri`. |
| `npm run fmt:rust:check` | Fail if Rust code is not formatted (CI / pre-merge). |
| `npm run lint:rust` | `clippy` on all targets with `-D warnings`. |

**CI:** GitHub Actions runs the build, Rust tests, `fmt:rust:check`, `lint:rust`, and `test:rust:coverage` on pull requests to `main` (see `.github/workflows/ci.yml`). Ubuntu runners install **WebKit/GTK dev packages** (`libwebkit2gtk-4.1-dev`, etc.) so `cargo test` can compile the Tauri dependency graph on Linux—the same set as the release workflow’s Linux build.

**Rust quality (recommended before merging backend-heavy changes):** run `npm run fmt:rust:check` and `npm run lint:rust` (or `npm run fmt:rust` to auto-fix formatting).

There is no ESLint config today; frontend quality is enforced by **TypeScript strict** (`tsconfig.json`) and **`tsc`** as part of `npm run build`.

## Project layout (quick)

- `src/` — React UI, routing, Tauri API usage (`src/tauri.ts`).
- `src-tauri/` — Rust crate (`lib.rs`, commands, DB, GunSpec client).
- `src-tauri/tauri.conf.json` — app metadata, bundle, `beforeDevCommand` / `beforeBuildCommand` (production bundle id **`com.weaverb.assetmanager`**).
- `src-tauri/tauri.dev.conf.json` — merge config for **`npm run tauri:dev`** only; overrides identifier to **`com.weaverb.assetmanager.dev`** so dev and prod SQLite paths differ.

End-user workflows (range days, ammunition checkout, toasts, form behavior) are summarized for humans in **`README.md` → “Using the application”.** When changing those flows, keep README and this section aligned.

## Product behavior (agents)

- **Toasts:** App-wide feedback uses `ToastProvider` (`src/context/ToastContext.tsx`) wrapping `AssetsListProvider` in `AppShell`, so `useToast()` works under the whole shell (including list refresh). Toasts render via `createPortal` to `document.body` with high z-index — prefer `pushToast(message, "error" | "success" | "info")` instead of page-top `banner` blocks for IPC errors, saves, and validation hints.
- **Asset list errors:** `AssetsListContext` no longer exposes `listError`; failed `refreshList` pushes an error toast.
- **Numeric form fields:** Use shared **`DigitsOnlyInput`** and **`parseNonNegInt`** (`src/lib/parseNumeric.ts`) for integers; **`DecimalTextInput`** + **`parseOptionalPrice`** / **`sanitizeDecimalInput`** for money-style decimals. Avoid `type="number"` on asset and range-day quantity-style fields (native controls fight controlled React state while typing).
- **Range days + ammunition (backend):** SQLite table `range_day_firearm_ammo` links planned days to firearm + ammunition assets. IPC includes `set_range_day_firearm_ammunition`, and `complete_range_day` accepts **`ammo_consumption`** (pairs must match assigned links; sums per gun match rounds fired when ammo is assigned; stock checks; decrements ammo `quantity`). See `src-tauri/src/db.rs` for validation rules (single caliber per gun per day, unique ammo asset per day across guns, etc.).
- **Range days (frontend):** `RangeDayDetailPage` — planned section labels **Apply Firearm(s)** from selection count; **Save and close** calls the same planned update then navigates to `/range-days`. GunSpec field notices under autocomplete stay inline (not toasts).
- **Backups:** Settings **Backup** uses `@tauri-apps/plugin-dialog` (`save` / `open`) and IPC `export_backup`, `import_backup`, `inspect_backup_file`. Implementation lives in `src-tauri/src/backup.rs` (ZIP via `ZipWriter` to a temp file, SQLite hot snapshot via `rusqlite` **backup** feature, `AMBK` v1/v2 + BIP39 + AES-GCM). Export/import holds `AppPaths.backup_lock` so only one backup operation runs at a time. After a successful import, the UI should call `refreshList()` from `AssetsListContext`.

## Conventional Commits

Use **[Conventional Commits](https://www.conventionalcommits.org/)** for all commits so history and automated tooling stay parseable.

**Format:** `<type>(<optional-scope>): <description>`

**Common types:** `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`.

**Suggested scopes for this repo** (pick what fits; omit scope if unclear):

| Scope | When |
|--------|------|
| `frontend` | React/TS/Vite under `src/` |
| `tauri` | Tauri config, plugins, window/bundle settings |
| `backend` / `rust` | Rust crate under `src-tauri/src/` |
| `db` | SQLite schema, migrations, persistence |
| `deps` | Only dependency bumps (npm or Cargo) |

**Examples:**

- `feat(frontend): add filter chips to asset table`
- `fix(backend): handle empty manufacturer response from cache`
- `chore(deps): bump vite to 7.x`

**Breaking changes:** add `!` after the type/scope or a `BREAKING CHANGE:` footer in the commit body so Semver major bumps are obvious (e.g. `feat(backend)!: remove legacy IPC command`).

## Semver and releases

This project follows **[Semantic Versioning 2.0.0](https://semver.org/)** (`MAJOR.MINOR.PATCH`).

**Version must stay in sync** in four places before tagging or shipping a release build:

1. `package.json` → `"version"`
2. `src-tauri/Cargo.toml` → `[package] version`
3. `src-tauri/tauri.conf.json` → `"version"`
4. `src-tauri/Cargo.lock` → `[[package]] name = "asset-manager"` → `version` (must match the crate version in `Cargo.toml` when using `--locked` builds)

Tauri’s release flow reads these consistently; drifting versions cause confusing bundles or updater metadata.

**Mapping (typical):**

| Change | Bump |
|--------|------|
| Bug fixes, small behavior fixes compatible with existing data/API | **PATCH** |
| New user-visible features, backward-compatible | **MINOR** |
| Breaking app behavior, incompatible on-disk format, or removed/changed public IPC contract | **MAJOR** |

**Automated releases (GitHub Actions):**

- **[`.github/workflows/ci.yml`](.github/workflows/ci.yml)** — on pull requests to `main`: `npm run build`, Rust tests, `fmt:rust:check`, `lint:rust`, and `test:rust:coverage` (Ubuntu runner).
- **[`.github/workflows/release.yml`](.github/workflows/release.yml)** — on every push to `main`: runs the same checks, then [release-please](https://github.com/googleapis/release-please) (manifest in [`release-please-config.json`](release-please-config.json) / [`.release-please-manifest.json`](.release-please-manifest.json)) opens or updates a **release PR** from Conventional Commits. When that PR is merged, the next push creates a **GitHub Release** (`vX.Y.Z`) and builds **macOS** (Apple Silicon + Intel), **Linux** (x64 `.deb` / `.AppImage` per Tauri defaults), and **Windows** installers via [`tauri-apps/tauri-action`](https://github.com/tauri-apps/tauri-action), attaching artifacts to that release.

**release-please paths:** the Rust component lives under `src-tauri/`, but `extra-files` entries must use a **leading `/`** on paths (e.g. `/package.json`, `/src-tauri/tauri.conf.json`) so release-please updates the **repo root** files. Without that, it looks for `src-tauri/package.json` and `src-tauri/src-tauri/tauri.conf.json`, skips the real files, and **Tauri bundle names** (which read `tauri.conf.json` → `version`) can stay on an old version while the Git tag is already `vX.Y.Z`.

**Maintainer flow:** merge feature work to `main` with Conventional Commits → merge the **release-please** release PR when you want to ship → installers appear on the GitHub Release for the new tag.

**Repository settings:** under **Settings → Actions → General → Workflow permissions**, enable **Read and write** for the `GITHUB_TOKEN` so release-please and tauri-action can open PRs and upload release assets.

**Emergency manual release checklist** (if automation is bypassed):

1. Bump all four version locations above to the same value (including `Cargo.lock` for the `asset-manager` package, e.g. `cargo build --manifest-path src-tauri/Cargo.toml` after editing `Cargo.toml`).
2. Run `npm run build`, `npm run test:rust`, `npm run test:rust:coverage`, `npm run fmt:rust:check`, and `npm run lint:rust`.
3. Run `npm run tauri build` on each target platform you ship.
4. Tag Git: `vX.Y.Z` (leading `v` matches release-please defaults in this repo).

## Agent-specific notes

- **IPC / `invoke`:** Full-stack testing needs `npm run tauri:dev` (or a built app). Browser-only `npm run dev` will show “Run the desktop app” / undefined `invoke` — that is expected.
- **Secrets:** GunSpec API keys are stored via in-app Settings in SQLite, not repo files (see `README.md`).
- **Coverage:** `test:rust:coverage` ignores `lib.rs` and `main.rs` in the threshold regex; extend tests rather than lowering the bar without team agreement.

When instructions here conflict with an explicit user message in chat, **follow the user**.

When making changes assume the user wants all documentation to be updated to reflect the changes.
