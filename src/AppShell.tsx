import { open, save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { ConfirmModal } from "./components/ConfirmModal";
import { SilhouetteSvgBrowserModal } from "./components/SilhouetteSvgBrowserModal";
import { AssetsListProvider, useAssetsList } from "./context/AssetsListContext";
import { ToastProvider, useToast } from "./context/ToastContext";
import type {
  AppSettings,
  BackupFileKindDto,
  ExportBackupInvokeResult,
} from "./types";
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
  const { query, setQuery, refreshList } = useAssetsList();
  const { pushToast } = useToast();

  const [settingsOpen, setSettingsOpen] = useState(false);
  const [gunspecKeyDraft, setGunspecKeyDraft] = useState("");
  const [devReseedConfirmOpen, setDevReseedConfirmOpen] = useState(false);
  const [svgBrowserOpen, setSvgBrowserOpen] = useState(false);

  const [backupEncrypt, setBackupEncrypt] = useState(false);
  const [backupWordCount, setBackupWordCount] = useState<12 | 24>(12);
  const [backupPassphrase, setBackupPassphrase] = useState("");
  const [backupPassphraseConfirm, setBackupPassphraseConfirm] = useState("");
  const [mnemonicReveal, setMnemonicReveal] = useState<string | null>(null);

  const [importFilePath, setImportFilePath] = useState<string | null>(null);
  const [importFileKind, setImportFileKind] =
    useState<BackupFileKindDto | null>(null);
  const [importMnemonic, setImportMnemonic] = useState("");
  const [importPassphrase, setImportPassphrase] = useState("");
  const [importConfirmOpen, setImportConfirmOpen] = useState(false);

  useEffect(() => {
    if (!settingsOpen) {
      setDevReseedConfirmOpen(false);
      setSvgBrowserOpen(false);
      setBackupEncrypt(false);
      setBackupWordCount(12);
      setBackupPassphrase("");
      setBackupPassphraseConfirm("");
      setMnemonicReveal(null);
      setImportFilePath(null);
      setImportFileKind(null);
      setImportMnemonic("");
      setImportPassphrase("");
      setImportConfirmOpen(false);
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

  const runExportBackup = async () => {
    if (backupEncrypt) {
      if (backupPassphrase !== backupPassphraseConfirm) {
        pushToast("Optional passphrases do not match.", "error");
        return;
      }
    }
    const defaultPath = backupEncrypt
      ? "asset-manager-backup.ambak"
      : "asset-manager-backup.zip";
    let path: string | null;
    try {
      path = await save({
        defaultPath,
        filters: [
          { name: "ZIP archive", extensions: ["zip"] },
          { name: "Encrypted backup", extensions: ["ambak"] },
        ],
      });
    } catch (e) {
      pushToast(String(e), "error");
      return;
    }
    if (path === null) return;
    try {
      const result = await invoke<ExportBackupInvokeResult>("export_backup", {
        path,
        encrypt: backupEncrypt,
        wordCount: backupWordCount,
        passphrase: backupEncrypt ? backupPassphrase : null,
      });
      pushToast("Backup saved.", "success");
      setBackupPassphrase("");
      setBackupPassphraseConfirm("");
      if (result.mnemonic) {
        setMnemonicReveal(result.mnemonic);
      }
    } catch (e) {
      pushToast(String(e), "error");
    }
  };

  const pickImportBackup = async () => {
    let path: string | string[] | null;
    try {
      path = await open({
        multiple: false,
        filters: [
          {
            name: "Backup",
            extensions: ["zip", "ambak"],
          },
        ],
      });
    } catch (e) {
      pushToast(String(e), "error");
      return;
    }
    if (path === null) return;
    const selected = Array.isArray(path) ? (path[0] ?? null) : path;
    if (!selected) return;
    try {
      const inspected = await invoke<{ kind: BackupFileKindDto }>(
        "inspect_backup_file",
        { path: selected },
      );
      setImportFilePath(selected);
      setImportFileKind(inspected.kind);
      setImportMnemonic("");
      setImportPassphrase("");
    } catch (e) {
      pushToast(String(e), "error");
    }
  };

  const runImportBackup = async () => {
    if (!importFilePath) return;
    if (importFileKind === "ambak" && importMnemonic.trim() === "") {
      pushToast(
        "This backup is encrypted. Enter the recovery phrase.",
        "error",
      );
      return;
    }
    if (importFileKind === "unknown") {
      pushToast("Could not read that file as a backup.", "error");
      return;
    }
    try {
      await invoke("import_backup", {
        path: importFilePath,
        mnemonic:
          importMnemonic.trim() === "" ? null : importMnemonic.trim(),
        passphrase: importPassphrase,
      });
      setImportConfirmOpen(false);
      setImportFilePath(null);
      setImportFileKind(null);
      setImportMnemonic("");
      setImportPassphrase("");
      await refreshList();
      pushToast("Backup restored. Your inventory was replaced.", "success");
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

                <div className="modal-field settings-backup-section">
                  <h3 className="settings-section-title">Backup</h3>
                  <p className="modal-lead settings-backup-lead">
                    Export a ZIP of your database and photos, or an encrypted
                    file protected by a recovery phrase. Importing a backup
                    replaces all inventory and images on this device.
                  </p>
                  <label className="settings-backup-check">
                    <input
                      type="checkbox"
                      checked={backupEncrypt}
                      onChange={(e) => setBackupEncrypt(e.target.checked)}
                    />{" "}
                    Encrypt backup (recovery phrase shown once after export)
                  </label>
                  {backupEncrypt ? (
                    <>
                      <fieldset className="settings-backup-words">
                        <legend className="settings-backup-legend">
                          Recovery phrase length
                        </legend>
                        <label className="settings-backup-radio">
                          <input
                            type="radio"
                            name="backupWordCount"
                            checked={backupWordCount === 12}
                            onChange={() => setBackupWordCount(12)}
                          />{" "}
                          12 words
                        </label>
                        <label className="settings-backup-radio">
                          <input
                            type="radio"
                            name="backupWordCount"
                            checked={backupWordCount === 24}
                            onChange={() => setBackupWordCount(24)}
                          />{" "}
                          24 words
                        </label>
                      </fieldset>
                      <label className="modal-field">
                        Optional passphrase (BIP39, separate from the word list)
                        <input
                          type="password"
                          value={backupPassphrase}
                          onChange={(e) => setBackupPassphrase(e.target.value)}
                          placeholder="Leave blank if not used"
                          autoComplete="new-password"
                          spellCheck={false}
                        />
                      </label>
                      <label className="modal-field">
                        Confirm passphrase
                        <input
                          type="password"
                          value={backupPassphraseConfirm}
                          onChange={(e) =>
                            setBackupPassphraseConfirm(e.target.value)
                          }
                          placeholder="Same as above"
                          autoComplete="new-password"
                          spellCheck={false}
                        />
                      </label>
                    </>
                  ) : null}
                  <div className="settings-backup-actions">
                    <button
                      type="button"
                      className="primary ghost"
                      onClick={() => void runExportBackup()}
                    >
                      Export backup…
                    </button>
                    <button
                      type="button"
                      onClick={() => void pickImportBackup()}
                    >
                      Choose backup to import…
                    </button>
                  </div>
                  {importFilePath ? (
                    <div className="settings-import-staging">
                      <p className="settings-import-file">
                        {importFilePath}
                      </p>
                      <p className="modal-lead settings-backup-lead">
                        {importFileKind === "ambak"
                          ? "Enter the recovery phrase for this backup. Use the same optional passphrase as when you exported, if any."
                          : importFileKind === "zip"
                            ? "Plain ZIP: recovery phrase is not required. Optional passphrase is ignored."
                            : "This file does not look like a valid backup."}
                      </p>
                      {importFileKind === "ambak" ? (
                        <label className="modal-field">
                          Recovery phrase
                          <textarea
                            className="settings-mnemonic-input"
                            value={importMnemonic}
                            onChange={(e) => setImportMnemonic(e.target.value)}
                            rows={3}
                            placeholder="twelve or twenty-four words…"
                            spellCheck={false}
                            autoComplete="off"
                          />
                        </label>
                      ) : null}
                      <label className="modal-field">
                        Optional passphrase (if the backup used one)
                        <input
                          type="password"
                          value={importPassphrase}
                          onChange={(e) => setImportPassphrase(e.target.value)}
                          autoComplete="new-password"
                          spellCheck={false}
                        />
                      </label>
                      <button
                        type="button"
                        className="danger"
                        disabled={importFileKind === "unknown"}
                        onClick={() => setImportConfirmOpen(true)}
                      >
                        Replace all data with this backup…
                      </button>
                    </div>
                  ) : null}
                </div>

                {import.meta.env.DEV ? (
                  <div className="modal-field dev-only-settings">
                    <h3 className="dev-settings-title">Development</h3>
                    <p className="modal-lead dev-settings-lead">
                      Removes every asset and image file, then inserts the same
                      dev seed used on first{" "}
                      <code className="mono-inline">npm run tauri:dev</code>{" "}
                      run (two
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

      {mnemonicReveal
        ? createPortal(
            <div
              className="modal-backdrop settings-modal-backdrop"
              role="presentation"
              onMouseDown={(e) => {
                if (e.target === e.currentTarget) {
                  setMnemonicReveal(null);
                }
              }}
            >
              <div
                className="modal"
                role="dialog"
                aria-labelledby="mnemonic-title"
                onMouseDown={(e) => e.stopPropagation()}
              >
                <div className="modal-head">
                  <h2 id="mnemonic-title">Save your recovery phrase</h2>
                  <button
                    type="button"
                    className="modal-close"
                    onClick={() => setMnemonicReveal(null)}
                    aria-label="Close"
                  >
                    ×
                  </button>
                </div>
                <p className="modal-lead">
                  Store these words in a safe place. Anyone with this phrase
                  (and your passphrase, if you set one) can decrypt the backup.
                  The app cannot recover this phrase later.
                </p>
                <pre className="settings-mnemonic-reveal" translate="no">
                  {mnemonicReveal}
                </pre>
                <div className="modal-actions">
                  <button
                    type="button"
                    onClick={async () => {
                      try {
                        await navigator.clipboard.writeText(mnemonicReveal);
                        pushToast("Copied to clipboard.", "success");
                      } catch (e) {
                        pushToast(String(e), "error");
                      }
                    }}
                  >
                    Copy phrase
                  </button>
                  <button
                    type="button"
                    className="primary"
                    onClick={() => setMnemonicReveal(null)}
                  >
                    Done
                  </button>
                </div>
              </div>
            </div>,
            document.body,
          )
        : null}

      {importConfirmOpen ? (
        <ConfirmModal
          title="Replace all data?"
          message="This removes your current database and photos on this device and replaces them with the selected backup. This cannot be undone."
          confirmLabel="Replace with backup"
          onCancel={() => setImportConfirmOpen(false)}
          onConfirm={() => {
            void runImportBackup();
          }}
        />
      ) : null}

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
