import { useEffect, useState, type Dispatch, type SetStateAction } from "react";
import type { AssetImage, AssetInput, AssetKind } from "../types";
import { AutocompleteField } from "./AutocompleteField";
import { DecimalTextInput } from "./DecimalTextInput";
import { DigitsOnlyInput } from "./DigitsOnlyInput";
import { TagInput } from "./TagInput";
import { KINDS } from "../lib/assetDefaults";
import {
  parseNonNegInt,
  parseOptionalPrice,
} from "../lib/parseNumeric";

export type AssetFormProps = {
  editing: AssetInput;
  setEditing: Dispatch<SetStateAction<AssetInput>>;
  isNew: boolean;
  selectedId: string | null;
  onSave: () => void;
  onSaveAndClose: () => void;
  onDelete: () => void;
  images: AssetImage[];
  imageUrls: Record<string, string>;
  onPickImage: (file: File | null) => void;
  deleteImage: (imageId: string) => void;
  fetchManufacturerSuggestions: (
    query: string,
  ) => Promise<import("../types").FieldSuggestions | string[]>;
  fetchModelSuggestions: (
    query: string,
  ) => Promise<import("../types").FieldSuggestions | string[]>;
  manufacturerGunspecNotice: string | null;
  setManufacturerGunspecNotice: (message: string | null) => void;
  modelGunspecNotice: string | null;
  setModelGunspecNotice: (message: string | null) => void;
  fetchTagSuggestions: (query: string) => Promise<string[]>;
  /** When true, only the action buttons show in the top bar (title lives in the drawer). */
  omitFormTitle?: boolean;
  /** Read-only round totals for firearms (from server). */
  firearmRoundStats?: { lifetime: number; sinceMaintenance: number } | null;
};

export function AssetForm({
  editing,
  setEditing,
  isNew,
  selectedId,
  onSave,
  onSaveAndClose,
  onDelete,
  images,
  imageUrls,
  onPickImage,
  deleteImage,
  fetchManufacturerSuggestions,
  fetchModelSuggestions,
  manufacturerGunspecNotice,
  setManufacturerGunspecNotice,
  modelGunspecNotice,
  setModelGunspecNotice,
  fetchTagSuggestions,
  omitFormTitle = false,
  firearmRoundStats = null,
}: AssetFormProps) {
  const listKey = isNew ? "new" : (selectedId ?? "");
  const [quantityText, setQuantityText] = useState(() =>
    String(editing.quantity ?? 1),
  );
  const [priceText, setPriceText] = useState(() =>
    editing.purchasePrice != null && Number.isFinite(editing.purchasePrice)
      ? String(editing.purchasePrice)
      : "",
  );

  useEffect(() => {
    setQuantityText(String(editing.quantity ?? 1));
    setPriceText(
      editing.purchasePrice != null && Number.isFinite(editing.purchasePrice)
        ? String(editing.purchasePrice)
        : "",
    );
  }, [listKey]);

  return (
    <>
      <div
        className={
          omitFormTitle ? "detail-head detail-head-actions-only" : "detail-head"
        }
      >
        {!omitFormTitle ? (
          <h2>{isNew ? "New asset" : "Edit asset"}</h2>
        ) : null}
        <div className="detail-actions">
          {!isNew && selectedId ? (
            <button type="button" className="danger" onClick={onDelete}>
              Delete
            </button>
          ) : null}
          <div className="save-actions">
            <button type="button" className="primary" onClick={onSave}>
              Save
            </button>
            <button type="button" className="primary ghost" onClick={onSaveAndClose}>
              Save and close
            </button>
          </div>
        </div>
      </div>

      <div className="form-grid">
        <label>
          Type
          <select
            value={editing.kind}
            onChange={(e) =>
              setEditing({
                ...editing,
                kind: e.target.value as AssetKind,
              })
            }
          >
            {KINDS.map((k) => (
              <option key={k.value} value={k.value}>
                {k.label}
              </option>
            ))}
          </select>
        </label>
        <label className="span-2">
          Name
          <input
            value={editing.name}
            onChange={(e) =>
              setEditing({ ...editing, name: e.target.value })
            }
          />
        </label>
        <div className="autocomplete-cell">
          <AutocompleteField
            label="Manufacturer"
            value={editing.manufacturer ?? ""}
            onChange={(manufacturer) =>
              setEditing({ ...editing, manufacturer })
            }
            fetchSuggestions={fetchManufacturerSuggestions}
            onRemoteNotice={setManufacturerGunspecNotice}
          />
          {manufacturerGunspecNotice ? (
            <p className="field-notice">{manufacturerGunspecNotice}</p>
          ) : null}
        </div>
        <div className="autocomplete-cell">
          <AutocompleteField
            label="Model"
            value={editing.model ?? ""}
            onChange={(model) => setEditing({ ...editing, model })}
            fetchSuggestions={fetchModelSuggestions}
            refetchKey={editing.manufacturer ?? ""}
            onRemoteNotice={setModelGunspecNotice}
          />
          {modelGunspecNotice ? (
            <p className="field-notice">{modelGunspecNotice}</p>
          ) : null}
        </div>
        <label>
          Serial
          <input
            value={editing.serialNumber ?? ""}
            onChange={(e) =>
              setEditing({ ...editing, serialNumber: e.target.value })
            }
          />
        </label>
        <label>
          Caliber
          <input
            value={editing.caliber ?? ""}
            onChange={(e) =>
              setEditing({ ...editing, caliber: e.target.value })
            }
          />
        </label>
        <label>
          Quantity
          <DigitsOnlyInput
            aria-label="Quantity"
            value={quantityText}
            onChange={(digits) => {
              setQuantityText(digits);
              setEditing({
                ...editing,
                quantity: parseNonNegInt(digits),
              });
            }}
          />
        </label>
        {editing.kind === "firearm" && firearmRoundStats ? (
          <>
            <div className="readonly-field">
              <span className="readonly-field-label">Lifetime rounds fired</span>
              <span className="readonly-field-value">
                {firearmRoundStats.lifetime.toLocaleString()}
              </span>
            </div>
            <div className="readonly-field">
              <span className="readonly-field-label">
                Rounds since maintenance
              </span>
              <span className="readonly-field-value">
                {firearmRoundStats.sinceMaintenance.toLocaleString()}
              </span>
            </div>
          </>
        ) : null}
        <label>
          Purchase date
          <input
            type="date"
            value={editing.purchaseDate?.slice(0, 10) ?? ""}
            onChange={(e) =>
              setEditing({ ...editing, purchaseDate: e.target.value })
            }
          />
        </label>
        <label>
          Purchase price
          <DecimalTextInput
            aria-label="Purchase price"
            value={priceText}
            onChange={(sanitized) => {
              setPriceText(sanitized);
              setEditing({
                ...editing,
                purchasePrice: parseOptionalPrice(sanitized),
              });
            }}
          />
        </label>
        <label className="span-2">
          Notes
          <textarea
            rows={4}
            value={editing.notes ?? ""}
            onChange={(e) =>
              setEditing({ ...editing, notes: e.target.value })
            }
          />
        </label>
        <div className="span-2 tag-field-cell">
          <TagInput
            label="Tags"
            tags={editing.tags ?? []}
            onChange={(tags) => setEditing({ ...editing, tags })}
            fetchSuggestions={fetchTagSuggestions}
            placeholder="Add tags — suggestions from your library"
          />
        </div>
        <label className="span-2">
          Extra fields (JSON)
          <textarea
            rows={3}
            className="mono"
            value={editing.extraJson ?? "{}"}
            onChange={(e) =>
              setEditing({ ...editing, extraJson: e.target.value })
            }
          />
        </label>
      </div>

      {!isNew && selectedId ? (
        <div className="images-section">
          <div className="images-head">
            <h3>Photos</h3>
            <label className="file-btn">
              Add photo
              <input
                type="file"
                accept="image/*"
                className="hidden-input"
                onChange={(e) =>
                  void onPickImage(e.target.files?.[0] ?? null)
                }
              />
            </label>
          </div>
          <div className="image-grid">
            {images.map((im) => (
              <figure key={im.id} className="thumb">
                {imageUrls[im.id] ? (
                  <img src={imageUrls[im.id]} alt="" />
                ) : (
                  <div className="thumb-fallback">…</div>
                )}
                <figcaption>
                  <button
                    type="button"
                    className="link danger"
                    onClick={() => void deleteImage(im.id)}
                  >
                    Remove
                  </button>
                </figcaption>
              </figure>
            ))}
          </div>
        </div>
      ) : (
        <p className="hint">Save the asset to attach photos.</p>
      )}
    </>
  );
}
