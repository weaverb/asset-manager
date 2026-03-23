import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { useMatch, useNavigate, useParams } from "react-router-dom";
import type {
  Asset,
  AssetImage,
  AssetInput,
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

export function AssetDrawerRoute() {
  const matchNew = useMatch({ path: "/assets/new", end: true });
  const isNew = Boolean(matchNew);
  const navigate = useNavigate();
  const { assetId } = useParams<{ assetId: string }>();
  const { refreshList } = useAssetsList();

  const [editing, setEditing] = useState<AssetInput | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [images, setImages] = useState<AssetImage[]>([]);
  const [imageUrls, setImageUrls] = useState<Record<string, string>>({});
  const [drawerError, setDrawerError] = useState<string | null>(null);
  const [manufacturerGunspecNotice, setManufacturerGunspecNotice] = useState<
    string | null
  >(null);
  const [modelGunspecNotice, setModelGunspecNotice] = useState<string | null>(
    null,
  );
  const [saveNotice, setSaveNotice] = useState<string | null>(null);
  const [deleteConfirmOpen, setDeleteConfirmOpen] = useState(false);
  const saveNoticeTimerRef = useRef<number | null>(null);

  const close = useCallback(() => {
    setDeleteConfirmOpen(false);
    if (saveNoticeTimerRef.current) {
      window.clearTimeout(saveNoticeTimerRef.current);
      saveNoticeTimerRef.current = null;
    }
    setSaveNotice(null);
    navigate("/assets");
  }, [navigate]);

  const showSaveNotice = useCallback((message: string) => {
    if (saveNoticeTimerRef.current) {
      window.clearTimeout(saveNoticeTimerRef.current);
    }
    setSaveNotice(message);
    saveNoticeTimerRef.current = window.setTimeout(() => {
      setSaveNotice(null);
      saveNoticeTimerRef.current = null;
    }, 5000);
  }, []);

  useEffect(() => {
    return () => {
      if (saveNoticeTimerRef.current) {
        window.clearTimeout(saveNoticeTimerRef.current);
      }
    };
  }, []);

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
    setDrawerError(null);
    setManufacturerGunspecNotice(null);
    setModelGunspecNotice(null);
    setSelectedId(null);
    setEditing(emptyInput());
    setImages([]);
    setImageUrls({});
  }, [isNew]);

  useEffect(() => {
    if (isNew) return;
    setDrawerError(null);
    setManufacturerGunspecNotice(null);
    setModelGunspecNotice(null);
    setSaveNotice(null);
    if (!assetId) {
      setEditing(null);
      setSelectedId(null);
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        const existing = await invoke<Asset | null>("get_asset", {
          id: assetId,
        });
        if (cancelled) return;
        if (!existing) {
          setDrawerError("Asset not found.");
          setEditing(null);
          setSelectedId(null);
          return;
        }
        setSelectedId(existing.id);
        setEditing(assetToInput(existing));
      } catch (e) {
        if (!cancelled) setDrawerError(String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [isNew, assetId]);

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
      setDrawerError("Name is required.");
      return;
    }
    const extra = (selected.extraJson ?? "{}").trim() || "{}";
    try {
      JSON.parse(extra);
    } catch {
      setDrawerError("Extra fields must be valid JSON.");
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
    setDrawerError(null);
    try {
      if (isNew || !selectedId) {
        const created = await invoke<Asset>("create_asset", { input: payload });
        setSelectedId(created.id);
        setEditing(assetToInput(created));
        await refreshList();
        navigate(`/assets/${created.id}`, { replace: true });
        if (closeAfter) {
          close();
        } else {
          window.setTimeout(() => {
            showSaveNotice(
              "Asset created and saved. You can add photos below.",
            );
          }, 0);
        }
      } else {
        const updated = await invoke<Asset>("update_asset", {
          id: selectedId,
          input: payload,
        });
        setEditing(assetToInput(updated));
        await refreshList();
        if (closeAfter) {
          close();
        } else {
          showSaveNotice("Details have been saved.");
        }
      }
    } catch (e) {
      setDrawerError(String(e));
    }
  };

  const runDelete = async () => {
    if (!selectedId) return;
    setDrawerError(null);
    try {
      await invoke("delete_asset", { id: String(selectedId) });
      setDeleteConfirmOpen(false);
      await refreshList();
      close();
    } catch (e) {
      setDrawerError(String(e));
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
    setDrawerError(null);
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
      setDrawerError(String(e));
    }
  };

  const deleteImage = async (imageId: string) => {
    setDrawerError(null);
    try {
      await invoke("delete_asset_image", { imageId });
      setImages((prev) => prev.filter((i) => i.id !== imageId));
      setImageUrls((prev) => {
        const n = { ...prev };
        delete n[imageId];
        return n;
      });
    } catch (e) {
      setDrawerError(String(e));
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
          {drawerError ? (
            <div className="drawer-body">
              <p className="drawer-error">{drawerError}</p>
            </div>
          ) : (
            <div className="drawer-body">
              <p className="muted">Loading…</p>
            </div>
          )}
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
          {drawerError ? (
            <div className="banner error drawer-banner">{drawerError}</div>
          ) : null}
          {saveNotice ? (
            <div className="form-save-notice" role="status">
              {saveNotice}
            </div>
          ) : null}
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
          />
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
