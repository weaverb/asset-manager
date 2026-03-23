# Asset Manager

Desktop app for tracking firearms, parts, accessories, and ammunition. Built with **Tauri 2**, **React**, **TypeScript**, **Vite**, and **SQLite** (bundled). Data and photos are stored locally on your machine.

## Prerequisites

Install these before working in this repository:

| Tool | Notes |
|------|--------|
| **Rust** | Install via [rustup](https://rustup.rs/). Required for the Tauri backend (`src-tauri`). |
| **Node.js 24** | This repo pins Node 24 (see `.node-version` and `package.json` → `engines`). Using [fnm](https://github.com/Schniz/fnm) is recommended: `brew install fnm`, then follow the [fnm shell setup](https://github.com/Schniz/fnm#shell-setup) (e.g. `eval "$(fnm env --use-on-cd)"` in `~/.zshrc`). |
| **System libraries** | Tauri needs platform-specific dependencies (WebView, etc.). Follow the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS. |

Optional but helpful:

- [VS Code](https://code.visualstudio.com/) with the [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) and [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) extensions.

## Local development setup

From the repository root (`asset-manager/`):

1. **Use Node 24** (if you use fnm, `cd` into the project so `.node-version` is picked up, or run `fnm use`):

   ```bash
   fnm use
   node -v   # should report v24.x
   ```

2. **Install JavaScript dependencies:**

   ```bash
   npm install
   ```

3. **Run the app in development mode** (starts the Vite dev server and opens the native window):

   ```bash
   npm run tauri dev
   ```

   The UI is served at `http://localhost:1420` inside the desktop shell; hot reload applies to the frontend, and Rust changes trigger a rebuild as usual for Tauri.

### Commands reference

| Command | Purpose |
|---------|---------|
| `npm run tauri dev` | **Main command for local testing:** run the full app with dev tooling. |
| `npm run dev` | Run only the Vite dev server (browser at `http://localhost:1420`). The Tauri APIs will not work in the browser; use this for quick UI-only checks. |
| `npm run build` | Production build of the frontend to `dist/` (`tsc` + Vite). |
| `npm run tauri build` | Build the installable app for your current platform (runs `npm run build` first per `tauri.conf.json`). |
| `npm run preview` | Preview the production frontend build locally (no Tauri). |
| `npm run test:rust` | Run **Rust unit tests** for the Tauri crate (`src-tauri`). |
| `npm run test:rust:coverage` | Generate a **line-coverage** report for those tests (see [Backend tests and coverage](#backend-tests-and-coverage)). |

### Development sample data (debug builds)

On **`npm run tauri dev`** (debug builds only), the app may **seed the database once** with a small set of sample assets (a couple of rows per kind: firearm, part, accessory, ammunition). Whether seeding has already run is stored in SQLite (`app_settings` key `dev_inventory_seeded`), so **later dev sessions do not insert duplicates**.

To wipe inventory and seed again, open **Settings** while running **`npm run tauri dev`**: under the **Development** section (shown only in Vite dev builds), use **Drop & reseed database**. That action is not available in release builds.

### Backend tests and coverage

Rust tests live next to the code under `src-tauri/src/` (`#[cfg(test)]` modules). Run them from the repo root:

```bash
npm run test:rust
```

For **HTML line coverage** (with a **minimum 80%** lines threshold):

1. Install [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) once: `cargo install cargo-llvm-cov` (the first run may also prompt `rustup` to add the `llvm-tools-preview` component).
2. Run:

   ```bash
   npm run test:rust:coverage
   ```

3. Open **`target/coverage-rust/html/index.html`** in a browser.

This project uses **LLVM source-based coverage** via `cargo llvm-cov` rather than `cargo tarpaulin`, because tarpaulin’s instrumentation can fail to link the Tauri/macOS dependency chain (e.g. Swift-backed build scripts) on some setups.

### Troubleshooting

**`TypeError: Cannot read properties of undefined (reading 'invoke')` (or the app shows “Run the desktop app”)**

The frontend only has access to Rust commands when it runs **inside the Tauri window**. If you open `http://localhost:1420` in a normal browser, or use `npm run dev` without Tauri, IPC is not available. Use **`npm run tauri dev`** for full local testing.

### Where data is stored

The SQLite database and image files live under the OS app data directory (e.g. `~/Library/Application Support/com.assetmanager.app/` on macOS). Exact paths follow Tauri’s `app_data_dir` for your platform.

App preferences (including an optional external API key) are stored in the same SQLite database in an `app_settings` table.

### Settings and manufacturer/model autocomplete

Open **Settings** from the header to paste a [GunSpec.io](https://gunspec.io) API key. The app does **not** read keys from files such as `api.key` in the repo; you must save the key through Settings so it is stored in SQLite. The key is used only by the Rust backend when calling GunSpec (it is not sent anywhere else).

**Tier limits:** Explorer/free tiers have a **low daily request cap** (on the order of tens of calls per day). The app limits how many manufacturer-list pages it fetches per cache refresh, **deduplicates** identical firearm search queries for several minutes, and **debounces** autocomplete so each keystroke does not always hit the network—but you can still hit **“daily cap exceeded”** if you browse suggestions heavily. If that happens, GunSpec resets at **midnight UTC**; upgrading your GunSpec plan raises the cap. A short notice may appear under the **Model** field when the API returns an error (including rate limits).

**Without an API key:** manufacturer and model fields still offer autocomplete based on values already present in your inventory (distinct values from your assets, with prefix matching as you type).

**With an API key:** those “learned” suggestions are merged with matches from the GunSpec catalog. Learned values are listed first; duplicate suggestions (ignoring case) are dropped.

**Network behavior:** after saving a key, loading manufacturer suggestions may fetch a **small number** of paginated manufacturer requests (capped) to fill an in-memory cache that expires after about an hour. Model suggestions use GunSpec’s firearms **search** endpoint; identical queries are cached briefly to avoid repeat calls. See **Tier limits** above if API results stop appearing.


