import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { ConfirmModal } from "./components/ConfirmModal";
import { SilhouetteSvgBrowserModal } from "./components/SilhouetteSvgBrowserModal";
import { AssetsListProvider, useAssetsList } from "./context/AssetsListContext";
import { ToastProvider, useToast } from "./context/ToastContext";
import type { AppSettings } from "./types";
import { invoke } from "./tauri";

function GearIcon() {
  return (
    <svg
      width="20"
      height="20"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.75"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d="M12 15a3 3 0 100-6 3 3 0 000 6z" />
      <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-2 2 2 2 0 01-2-2v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06a1.65 1.65 0 00.33-1.82 1.65 1.65 0 00-1.51-1H3a2 2 0 01-2-2 2 2 0 012-2h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06a1.65 1.65 0 001.82.33H9a1.65 1.65 0 001-1.51V3a2 2 0 012-2 2 2 0 012 2v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06a1.65 1.65 0 00-.33 1.82V9a1.65 1.65 0 001.51 1H21a2 2 0 012 2 2 2 0 01-2 2h-.09a1.65 1.65 0 00-1.51 1z" />
    </svg>
  );
}

function AppShellInner() {
  const location = useLocation();
  const navigate = useNavigate();
  const onAssets = location.pathname.startsWith("/assets");
  const onRangeDays = location.pathname.startsWith("/range-days");
  const { query, setQuery } = useAssetsList();
  const { pushToast } = useToast();

  const [settingsOpen, setSettingsOpen] = useState(false);
  const [gunspecKeyDraft, setGunspecKeyDraft] = useState("");
  const [devReseedConfirmOpen, setDevReseedConfirmOpen] = useState(false);
  const [svgBrowserOpen, setSvgBrowserOpen] = useState(false);

  useEffect(() => {
    if (!settingsOpen) {
      setDevReseedConfirmOpen(false);
      setSvgBrowserOpen(false);
    }
  }, [settingsOpen]);

  const openSettings = async () => {
    try {
      const s = await invoke<AppSettings>("get_app_settings");
      setGunspecKeyDraft(s.gunspecApiKey ?? "");
      setSettingsOpen(true);
    } catch (e) {
      pushToast(String(e), "error");
    }
  };

  const saveSettings = async () => {
    try {
      await invoke("save_app_settings", {
        settings: { gunspecApiKey: gunspecKeyDraft.trim() },
      });
      setSettingsOpen(false);
      pushToast("Settings saved.", "success");
    } catch (e) {
      pushToast(String(e), "error");
    }
  };

  return (
    <div className="app">
      <header className="top top--floating">
        <div className="brand">
          <h1>Asset Manager</h1>
        </div>
        <div className="toolbar toolbar--floating">
          <nav className="main-nav" aria-label="Main">
            <NavLink
              to="/"
              end
              className={({ isActive }) =>
                isActive ? "nav-link active" : "nav-link"
              }
            >
              Dashboard
            </NavLink>
            <NavLink
              to="/assets"
              className={({ isActive }) =>
                isActive ? "nav-link active" : "nav-link"
              }
            >
              All assets
            </NavLink>
            <NavLink
              to="/range-days"
              className={({ isActive }) =>
                isActive ? "nav-link active" : "nav-link"
              }
            >
              Range days
            </NavLink>
          </nav>
          {onAssets ? (
            <>
              <input
                className="search floating-header-search"
                placeholder="Search (full text)…"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
              />
              <button
                type="button"
                className="primary floating-header-new"
                onClick={() => navigate("/assets/new")}
              >
                New asset
              </button>
            </>
          ) : null}
          {onRangeDays ? (
            <button
              type="button"
              className="primary floating-header-new"
              onClick={() => navigate("/range-days/new")}
            >
              New range day
            </button>
          ) : null}
          <button
            type="button"
            className="icon-button"
            onClick={() => void openSettings()}
            aria-label="Settings"
            title="Settings"
          >
            <GearIcon />
          </button>
        </div>
      </header>

      {settingsOpen
        ? createPortal(
            <div
              className="modal-backdrop settings-modal-backdrop"
              role="presentation"
              onMouseDown={(e) => {
                if (e.target === e.currentTarget) setSettingsOpen(false);
              }}
            >
              <div
                className="modal"
                role="dialog"
                aria-labelledby="settings-title"
                onMouseDown={(e) => e.stopPropagation()}
              >
                <div className="modal-head">
                  <h2 id="settings-title">Settings</h2>
                  <button
                    type="button"
                    className="modal-close"
                    onClick={() => setSettingsOpen(false)}
                    aria-label="Close"
                  >
                    ×
                  </button>
                </div>
                <p className="modal-lead">
                  Optional{" "}
                  <button
                    type="button"
                    className="link-inline"
                    onClick={() => void openUrl("https://gunspec.io")}
                  >
                    GunSpec.io
                  </button>{" "}
                  API key enables manufacturer and model suggestions from their
                  catalog. Without a key, suggestions come only from values
                  already in your inventory.
                </p>
                <label className="modal-field">
                  GunSpec API key
                  <input
                    type="password"
                    value={gunspecKeyDraft}
                    onChange={(e) => setGunspecKeyDraft(e.target.value)}
                    placeholder="Paste key (optional)"
                    autoComplete="off"
                    spellCheck={false}
                  />
                </label>
                {import.meta.env.DEV ? (
                  <div className="modal-field dev-only-settings">
                    <h3 className="dev-settings-title">Development</h3>
                    <p className="modal-lead dev-settings-lead">
                      Removes every asset and image file, then inserts the same
                      dev seed used on first{" "}
                      <code className="mono-inline">tauri dev</code> run (two
                      rows per type). Release builds cannot run this.
                    </p>
                    <button
                      type="button"
                      className="danger"
                      onClick={() => setDevReseedConfirmOpen(true)}
                    >
                      Drop &amp; reseed database
                    </button>
                    <button
                      type="button"
                      className="primary ghost"
                      onClick={() => setSvgBrowserOpen(true)}
                    >
                      Open silhouette SVG preview…
                    </button>
                  </div>
                ) : null}
                <div className="modal-actions">
                  <button type="button" onClick={() => setSettingsOpen(false)}>
                    Cancel
                  </button>
                  <button
                    type="button"
                    className="primary"
                    onClick={() => void saveSettings()}
                  >
                    Save
                  </button>
                </div>
              </div>
            </div>,
            document.body,
          )
        : null}

      {import.meta.env.DEV ? (
        <SilhouetteSvgBrowserModal
          open={svgBrowserOpen}
          onClose={() => setSvgBrowserOpen(false)}
        />
      ) : null}

      {devReseedConfirmOpen ? (
        <ConfirmModal
          title="Drop and reseed?"
          message="Delete ALL assets and photos from this database, then insert the dev sample set again. This cannot be undone."
          confirmLabel="Drop and reseed"
          onCancel={() => setDevReseedConfirmOpen(false)}
          onConfirm={() => {
            setDevReseedConfirmOpen(false);
            void (async () => {
              try {
                await invoke("dev_drop_and_reseed");
                pushToast(
                  "Done. Open “All assets” again (or reload the page) to see seeded rows.",
                  "success",
                );
              } catch (e) {
                pushToast(String(e), "error");
              }
            })();
          }}
        />
      ) : null}

      <div className="app-body app-body--floating">
        <Outlet />
      </div>
    </div>
  );
}

export function AppShell() {
  return (
    <ToastProvider>
      <AssetsListProvider>
        <AppShellInner />
      </AssetsListProvider>
    </ToastProvider>
  );
}
