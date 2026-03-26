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
   npm run tauri:dev
   ```

   This uses [`src-tauri/tauri.dev.conf.json`](src-tauri/tauri.dev.conf.json) so the dev app has bundle id **`com.weaverb.assetmanager.dev`** and does **not** share its SQLite database with a production install (**`com.weaverb.assetmanager`**). The UI is served at `http://localhost:1420` inside the desktop shell; hot reload applies to the frontend, and Rust changes trigger a rebuild as usual for Tauri.

### Commands reference

| Command | Purpose |
|---------|---------|
| `npm run tauri:dev` | **Main command for local testing:** full app with dev tooling and a **separate** app data dir from production (see [Where data is stored](#where-data-is-stored)). |
| `npm run dev` | Run only the Vite dev server (browser at `http://localhost:1420`). The Tauri APIs will not work in the browser; use this for quick UI-only checks. |
| `npm run build` | Production build of the frontend to `dist/` (`tsc` + Vite). |
| `npm run tauri build` | Build the installable app for your current platform (runs `npm run build` first per `tauri.conf.json`). |
| `npm run preview` | Preview the production frontend build locally (no Tauri). |
| `npm run test:rust` | Run **Rust unit tests** for the Tauri crate (`src-tauri`). |
| `npm run test:rust:coverage` | Generate a **line-coverage** report for those tests (see [Backend tests and coverage](#backend-tests-and-coverage)). |

### CI and releases (GitHub)

Pull requests to `main` run **[CI](https://github.com/weaverb/asset-manager/actions/workflows/ci.yml)** (`npm run build`, Rust tests, formatting, Clippy, and coverage).

Pushes to `main` run **[Release](https://github.com/weaverb/asset-manager/actions/workflows/release.yml)**: the same checks, then [release-please](https://github.com/googleapis/release-please) opens or updates a **release pull request** when there are releasable [Conventional Commits](https://www.conventionalcommits.org/). Merging that PR triggers a **GitHub Release** tagged `vX.Y.Z` and uploads **macOS**, **Linux**, and **Windows** installers built with [tauri-action](https://github.com/tauri-apps/tauri-action). Human-readable changes accumulate in [`CHANGELOG.md`](CHANGELOG.md).

After a corrective `fix` lands on `main` (for example to repair release automation), **merge the next release-please PR** (often a patch like **v0.2.1**) so new installers are built with the correct embedded version.

The repository needs **Settings → Actions → General → Workflow permissions → Read and write** for the default `GITHUB_TOKEN` so those workflows can manage pull requests and release assets.

### Development sample data (debug builds)

On **`npm run tauri:dev`** (debug builds only), the app may **seed the database once** with a richer sample set: **six** firearms (mixed calibers and maintenance settings), **six** ammunition rows (at least one per firearm caliber), two parts, and two accessories, plus a few **completed** range days for usage stats. Whether seeding has already run is stored in SQLite (`app_settings` key `dev_inventory_seeded`), so **later dev sessions do not insert duplicates**.

To wipe inventory and seed again, open **Settings** while running **`npm run tauri:dev`**: under the **Development** section (shown only in Vite dev builds), use **Drop & reseed database**. That action is not available in release builds.

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

The frontend only has access to Rust commands when it runs **inside the Tauri window**. If you open `http://localhost:1420` in a normal browser, or use `npm run dev` without Tauri, IPC is not available. Use **`npm run tauri:dev`** for full local testing.

### Where data is stored

The SQLite database and image files live under the OS app data directory. Exact paths follow Tauri’s `app_data_dir` for your platform:

- **Production** installs (e.g. from a release `.dmg` / `.msi`): bundle id **`com.weaverb.assetmanager`** — on macOS, e.g. `~/Library/Application Support/com.weaverb.assetmanager/`.
- **Local development** (`npm run tauri:dev`): bundle id **`com.weaverb.assetmanager.dev`** — e.g. `~/Library/Application Support/com.weaverb.assetmanager.dev/`, so your dev DB never collides with production.

Identifiers intentionally **omit a `.app` suffix** so the app data directory name does not end in `.app` (macOS Finder otherwise treats the folder like a broken application bundle when opened from the GUI).

Changing the production identifier does **not** migrate data from older folders (e.g. `com.assetmanager.app` or `com.weaverb.assetmanager.app`). Copy `asset_manager.db` and the `images/` folder manually if you need to move data.

App preferences (including an optional external API key) are stored in the same SQLite database in an `app_settings` table.

### Using the application

**Dashboard**

- The home view uses two columns on wide windows: **range-day shortcuts** on the left and **widgets** on the right (they stack on smaller screens).
- **Ammo by caliber** is a donut chart of **rounds on hand** (sum of ammunition **Quantity**), grouped by **caliber** (blank calibers count as “Unknown”).
- **Upcoming maintenance** lists **firearms** that have a maintenance interval set and are **within 10%** of that threshold: either **rounds since last maintenance** versus **Maintenance every N rounds**, or **days until the next due date** versus **Maintenance every N days**. The next due date is computed from the **most recent maintenance record** on that gun; if there are none, from **purchase date**, otherwise from when the asset was **created**. **Overdue** day-based maintenance appears too. Names link to **All assets** and open that firearm in the edit drawer.
- **Top firearms** ranks guns by **lifetime rounds fired** (from completed range days), then by how many **completed** range days include that firearm. Names also link to the asset drawer.

**Assets**

- Open **All assets** to browse, search, filter by type and tags, and create or edit rows in the side drawer. The table’s first column shows a **silhouette icon** from the asset **kind** and, for **firearms** and **accessories**, the optional **subtype** (e.g. pistol vs rifle, scope vs red dot); the same icon appears next to the drawer title when editing.
- **Firearms** show lifetime rounds and rounds since last maintenance (updated when you complete a range day or add a maintenance record). Optional **Maintenance every N rounds** and **Maintenance every N days** drive the dashboard reminder rules above; leave them blank to disable. The **Maintenance** block at the bottom of the drawer matches the main form layout: optional performed date/time, notes, and **Add maintenance** (which resets “rounds since maintenance” for that gun and anchors the day-based countdown from that performed date).
- **Quantity** and **purchase price** use plain text numeric fields (not the browser’s native number spinners) so values are easy to type and edit.

**Range days**

- From **Range days** (or the dashboard), create a day with a date and the firearms you plan to bring.
- On a planned day, **Edit plan** lets you change the date and the firearm checklist. Use **Apply Firearm** or **Apply Firearms** (label reflects how many are selected) to persist date and guns. **Save and close** does the same update and returns to the range day list. **Cancel range day** or **Delete range day** are destructive actions at the bottom of the form.
- **Ammunition checkout:** assign inventory ammunition to each gun on that day. Only **one caliber** is allowed per firearm; you may assign **several ammunition assets** of that caliber (e.g. different brands). The **same ammunition asset cannot** be assigned to **two different firearms** on the **same** day.
- **Complete range day:** enter rounds fired per firearm. If ammunition was assigned, split those totals across the assigned boxes under **Rounds from inventory**; the sum per gun must match rounds fired. **Use stock** next to a field fills it with the linked ammunition’s current on-hand quantity (or, for the main rounds field, the sum across assigned ammo for that gun). Completing the day increments each firearm’s round counters, records optional notes, stores how many rounds came from each assigned box, and **reduces ammunition quantities** in inventory (the backend checks that you are not consuming more than on hand).

**Notifications**

- Errors, success messages (e.g. saved settings, updated range day), and short informational prompts appear as **toasts** in the **lower-right** of the window so they stay visible even when the page is scrolled. Dismiss a toast with **×**; they also time out automatically.

### Settings and manufacturer/model autocomplete

Open **Settings** from the header to paste a [GunSpec.io](https://gunspec.io) API key. The app does **not** read keys from files such as `api.key` in the repo; you must save the key through Settings so it is stored in SQLite. The key is used only by the Rust backend when calling GunSpec (it is not sent anywhere else).

**Tier limits:** Explorer/free tiers have a **low daily request cap** (on the order of tens of calls per day). The app limits how many manufacturer-list pages it fetches per cache refresh, **deduplicates** identical firearm search queries for several minutes, and **debounces** autocomplete so each keystroke does not always hit the network—but you can still hit **“daily cap exceeded”** if you browse suggestions heavily. If that happens, GunSpec resets at **midnight UTC**; upgrading your GunSpec plan raises the cap. A short notice may appear under the **Model** field when the API returns an error (including rate limits).

**Without an API key:** manufacturer and model fields still offer autocomplete based on values already present in your inventory (distinct values from your assets, with prefix matching as you type).

**With an API key:** those “learned” suggestions are merged with matches from the GunSpec catalog. Learned values are listed first; duplicate suggestions (ignoring case) are dropped.

**Network behavior:** after saving a key, loading manufacturer suggestions may fetch a **small number** of paginated manufacturer requests (capped) to fill an in-memory cache that expires after about an hour. Model suggestions use GunSpec’s firearms **search** endpoint; identical queries are cached briefly to avoid repeat calls. See **Tier limits** above if API results stop appearing.


