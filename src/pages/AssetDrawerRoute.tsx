import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useState,
} from "react";
import { useMatch, useNavigate, useParams } from "react-router-dom";
import type {
  Asset,
  AssetImage,
  AssetInput,
  AssetMaintenance,
  FieldSuggestions,
  ImagePayload,
} from "../types";
import { invoke } from "../tauri";
import { AssetForm } from "../components/AssetForm";
import { ConfirmModal } from "../components/ConfirmModal";
import {
  assetToInput,
  emptyInput,
  normalizeTagsForSave,
} from "../lib/assetDefaults";
import { useAssetsList } from "../context/AssetsListContext";
import { useToast } from "../context/ToastContext";

export function AssetDrawerRoute() {
  const matchNew = useMatch({ path: "/assets/new", end: true });
  const isNew = Boolean(matchNew);
  const navigate = useNavigate();
  const { assetId } = useParams<{ assetId: string }>();
  const { refreshList } = useAssetsList();
  const { pushToast } = useToast();

  const [editing, setEditing] = useState<AssetInput | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [images, setImages] = useState<AssetImage[]>([]);
  const [imageUrls, setImageUrls] = useState<Record<string, string>>({});
  const [drawerLoadFailed, setDrawerLoadFailed] = useState(false);
  const [manufacturerGunspecNotice, setManufacturerGunspecNotice] = useState<
    string | null
  >(null);
  const [modelGunspecNotice, setModelGunspecNotice] = useState<string | null>(
    null,
  );
  const [deleteConfirmOpen, setDeleteConfirmOpen] = useState(false);
  const [loadedAsset, setLoadedAsset] = useState<Asset | null>(null);
  const [maintenanceList, setMaintenanceList] = useState<AssetMaintenance[]>(
    [],
  );
  const [maintPerformedAt, setMaintPerformedAt] = useState("");
  const [maintNotes, setMaintNotes] = useState("");
  const [maintSaving, setMaintSaving] = useState(false);

  const close = useCallback(() => {
    setDeleteConfirmOpen(false);
    navigate("/assets");
  }, [navigate]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      if (deleteConfirmOpen) {
        setDeleteConfirmOpen(false);
        return;
      }
      close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [close, deleteConfirmOpen]);

  useLayoutEffect(() => {
    if (!isNew) return;
    setDrawerLoadFailed(false);
    setManufacturerGunspecNotice(null);
    setModelGunspecNotice(null);
    setSelectedId(null);
    setEditing(emptyInput());
    setImages([]);
    setImageUrls({});
    setLoadedAsset(null);
    setMaintenanceList([]);
    setMaintPerformedAt("");
    setMaintNotes("");
  }, [isNew]);

  useEffect(() => {
    if (isNew) return;
    setManufacturerGunspecNotice(null);
    setModelGunspecNotice(null);
    if (!assetId) {
      setEditing(null);
      setSelectedId(null);
      setLoadedAsset(null);
      setDrawerLoadFailed(false);
      return;
    }
    setDrawerLoadFailed(false);
    let cancelled = false;
    void (async () => {
      try {
        const existing = await invoke<Asset | null>("get_asset", {
          id: assetId,
        });
        if (cancelled) return;
        if (!existing) {
          pushToast("Asset not found.", "error");
          setDrawerLoadFailed(true);
          setEditing(null);
          setSelectedId(null);
          setLoadedAsset(null);
          return;
        }
        setDrawerLoadFailed(false);
        setSelectedId(existing.id);
        setEditing(assetToInput(existing));
        setLoadedAsset(existing);
      } catch (e) {
        if (!cancelled) {
          pushToast(String(e), "error");
          setDrawerLoadFailed(true);
          setEditing(null);
          setSelectedId(null);
          setLoadedAsset(null);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [isNew, assetId, pushToast]);

  const selected = editing;

  useEffect(() => {
    if (!selectedId || isNew) {
      setImages([]);
      setImageUrls({});
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        const imgs = await invoke<AssetImage[]>("list_asset_images", {
          assetId: selectedId,
        });
        if (cancelled) return;
        setImages(imgs);
        const next: Record<string, string> = {};
        for (const im of imgs) {
          try {
            const data = await invoke<ImagePayload>("get_image_data", {
              path: im.filePath,
            });
            next[im.id] = `data:${data.mime};base64,${data.dataBase64}`;
          } catch {
            /* skip broken file */
          }
        }
        if (!cancelled) setImageUrls(next);
      } catch {
        if (!cancelled) setImages([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [selectedId, isNew]);

  useEffect(() => {
    if (!selectedId || isNew || selected?.kind !== "firearm") {
      setMaintenanceList([]);
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        const rows = await invoke<AssetMaintenance[]>("list_asset_maintenance", {
          assetId: selectedId,
        });
        if (!cancelled) setMaintenanceList(rows);
      } catch {
        if (!cancelled) setMaintenanceList([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [selectedId, isNew, selected?.kind]);

  const fetchManufacturerSuggestions = useCallback((query: string) => {
    return invoke<FieldSuggestions | string[]>("suggest_manufacturers", {
      query,
    });
  }, []);

  const fetchModelSuggestions = useCallback(
    (query: string) => {
      return invoke<FieldSuggestions | string[]>("suggest_models", {
        manufacturer: selected?.manufacturer ?? "",
        query,
      });
    },
    [selected?.manufacturer],
  );

  const fetchTagSuggestions = useCallback((query: string) => {
    return invoke<FieldSuggestions>("suggest_tags", { query }).then(
      (r) => r.items,
    );
  }, []);

  const save = async (closeAfter: boolean) => {
    if (!selected) return;
    if (!selected.name.trim()) {
      pushToast("Name is required.", "info");
      return;
    }
    const extra = (selected.extraJson ?? "{}").trim() || "{}";
    try {
      JSON.parse(extra);
    } catch {
      pushToast("Extra fields must be valid JSON.", "info");
      return;
    }
    const payload: AssetInput = {
      ...selected,
      name: selected.name.trim(),
      manufacturer: selected.manufacturer?.trim() || null,
      model: selected.model?.trim() || null,
      serialNumber: selected.serialNumber?.trim() || null,
      caliber: selected.caliber?.trim() || null,
      purchaseDate: selected.purchaseDate?.trim() || null,
      notes: selected.notes?.trim() || null,
      extraJson: extra,
      tags: normalizeTagsForSave(selected.tags),
    };
    try {
      if (isNew || !selectedId) {
        const created = await invoke<Asset>("create_asset", { input: payload });
        setSelectedId(created.id);
        setEditing(assetToInput(created));
        setLoadedAsset(created);
        await refreshList();
        navigate(`/assets/${created.id}`, { replace: true });
        if (closeAfter) {
          close();
        } else {
          pushToast(
            "Asset created and saved. You can add photos below.",
            "success",
          );
        }
      } else {
        const updated = await invoke<Asset>("update_asset", {
          id: selectedId,
          input: payload,
        });
        setEditing(assetToInput(updated));
        setLoadedAsset(updated);
        await refreshList();
        if (closeAfter) {
          close();
        } else {
          pushToast("Details have been saved.", "success");
        }
      }
    } catch (e) {
      pushToast(String(e), "error");
    }
  };

  const runDelete = async () => {
    if (!selectedId) return;
    try {
      await invoke("delete_asset", { id: String(selectedId) });
      setDeleteConfirmOpen(false);
      await refreshList();
      close();
    } catch (e) {
      pushToast(String(e), "error");
      setDeleteConfirmOpen(false);
    }
  };

  const onPickImage = async (file: File | null) => {
    if (!file || !selectedId) return;
    const buf = await file.arrayBuffer();
    const bytes = new Uint8Array(buf);
    let binary = "";
    for (let i = 0; i < bytes.length; i++) {
      binary += String.fromCharCode(bytes[i]!);
    }
    const dataBase64 = btoa(binary);
    try {
      await invoke("add_asset_image", {
        assetId: selectedId,
        originalName: file.name,
        dataBase64,
        caption: null,
      });
      const imgs = await invoke<AssetImage[]>("list_asset_images", {
        assetId: selectedId,
      });
      setImages(imgs);
      const next = { ...imageUrls };
      for (const im of imgs) {
        if (next[im.id]) continue;
        try {
          const data = await invoke<ImagePayload>("get_image_data", {
            path: im.filePath,
          });
          next[im.id] = `data:${data.mime};base64,${data.dataBase64}`;
        } catch {
          /* skip */
        }
      }
      setImageUrls(next);
    } catch (e) {
      pushToast(String(e), "error");
    }
  };

  const submitMaintenance = async () => {
    if (!selectedId || selected?.kind !== "firearm") return;
    setMaintSaving(true);
    try {
      await invoke("add_asset_maintenance", {
        assetId: selectedId,
        performedAt: maintPerformedAt.trim() || null,
        notes: maintNotes.trim() || null,
      });
      setMaintPerformedAt("");
      setMaintNotes("");
      const rows = await invoke<AssetMaintenance[]>("list_asset_maintenance", {
        assetId: selectedId,
      });
      setMaintenanceList(rows);
      const refreshed = await invoke<Asset | null>("get_asset", {
        id: selectedId,
      });
      if (refreshed) setLoadedAsset(refreshed);
      pushToast(
        "Maintenance recorded. Rounds since maintenance reset.",
        "success",
      );
    } catch (e) {
      pushToast(String(e), "error");
    } finally {
      setMaintSaving(false);
    }
  };

  const deleteImage = async (imageId: string) => {
    try {
      await invoke("delete_asset_image", { imageId });
      setImages((prev) => prev.filter((i) => i.id !== imageId));
      setImageUrls((prev) => {
        const n = { ...prev };
        delete n[imageId];
        return n;
      });
    } catch (e) {
      pushToast(String(e), "error");
    }
  };

  if (!selected) {
    return (
      <div className="drawer-root" role="presentation">
        <button
          type="button"
          className="drawer-backdrop"
          aria-label="Close"
          onClick={close}
        />
        <aside className="drawer-panel" role="dialog" aria-modal="true">
          <div className="drawer-head">
            <h2 className="drawer-title">Asset</h2>
            <button
              type="button"
              className="drawer-close"
              onClick={close}
              aria-label="Close"
            >
              ×
            </button>
          </div>
          <div className="drawer-body">
            {drawerLoadFailed ? (
              <p className="drawer-error muted">
                Couldn&rsquo;t load this asset. See the notification.
              </p>
            ) : (
              <p className="muted">Loading…</p>
            )}
          </div>
        </aside>
      </div>
    );
  }

  return (
    <div className="drawer-root" role="presentation">
      <button
        type="button"
        className="drawer-backdrop"
        aria-label="Close"
        onClick={close}
      />
      <aside className="drawer-panel" role="dialog" aria-modal="true">
        <div className="drawer-head">
          <h2 className="drawer-title">
            {isNew ? "New asset" : "Edit asset"}
          </h2>
          <button
            type="button"
            className="drawer-close"
            onClick={close}
            aria-label="Close"
          >
            ×
          </button>
        </div>
        <div className="drawer-body">
          <AssetForm
            editing={selected}
            setEditing={(next) => {
              setEditing((prev) => {
                if (prev === null) return null;
                return typeof next === "function" ? next(prev) : next;
              });
            }}
            isNew={isNew}
            selectedId={selectedId}
            onSave={() => void save(false)}
            onSaveAndClose={() => void save(true)}
            onDelete={() => setDeleteConfirmOpen(true)}
            images={images}
            imageUrls={imageUrls}
            onPickImage={(f) => void onPickImage(f)}
            deleteImage={(id) => void deleteImage(id)}
            fetchManufacturerSuggestions={fetchManufacturerSuggestions}
            fetchModelSuggestions={fetchModelSuggestions}
            manufacturerGunspecNotice={manufacturerGunspecNotice}
            setManufacturerGunspecNotice={setManufacturerGunspecNotice}
            modelGunspecNotice={modelGunspecNotice}
            setModelGunspecNotice={setModelGunspecNotice}
            fetchTagSuggestions={fetchTagSuggestions}
            omitFormTitle
            firearmRoundStats={
              !isNew &&
              selected.kind === "firearm" &&
              loadedAsset &&
              loadedAsset.id === selectedId
                ? {
                    lifetime: loadedAsset.lifetimeRoundsFired,
                    sinceMaintenance:
                      loadedAsset.roundsFiredSinceMaintenance,
                  }
                : null
            }
          />
          {!isNew && selectedId && selected.kind === "firearm" ? (
            <section className="maintenance-section">
              <div className="maintenance-section-head">
                <h3>Maintenance</h3>
              </div>
              <p className="muted maintenance-section-lead">
                Adding a record resets &ldquo;rounds since maintenance&rdquo; for
                this firearm.
              </p>
              {maintenanceList.length === 0 ? (
                <p className="muted maintenance-empty">No maintenance entries yet.</p>
              ) : (
                <ul className="maintenance-log">
                  {maintenanceList.map((m) => (
                    <li key={m.id}>
                      <time dateTime={m.performedAt}>
                        {new Date(m.performedAt).toLocaleString()}
                      </time>
                      {m.notes ? (
                        <p className="maint-notes">{m.notes}</p>
                      ) : null}
                    </li>
                  ))}
                </ul>
              )}
              <div className="maintenance-add-form form-grid">
                <label className="span-2">
                  Performed (optional)
                  <input
                    type="datetime-local"
                    value={maintPerformedAt}
                    onChange={(e) => setMaintPerformedAt(e.target.value)}
                  />
                </label>
                <label className="span-2">
                  Notes
                  <textarea
                    rows={3}
                    value={maintNotes}
                    onChange={(e) => setMaintNotes(e.target.value)}
                    placeholder="e.g. Cleaned, inspected spring…"
                  />
                </label>
                <div className="span-2 maintenance-form-actions">
                  <button
                    type="button"
                    className="primary"
                    disabled={maintSaving}
                    onClick={() => void submitMaintenance()}
                  >
                    {maintSaving ? "Saving…" : "Add maintenance"}
                  </button>
                </div>
              </div>
            </section>
          ) : null}
        </div>
      </aside>
      {deleteConfirmOpen ? (
        <ConfirmModal
          title="Delete this asset?"
          message="This removes the asset and its photos from your local inventory. This cannot be undone."
          confirmLabel="Delete"
          onCancel={() => setDeleteConfirmOpen(false)}
          onConfirm={() => void runDelete()}
        />
      ) : null}
    </div>
  );
}
