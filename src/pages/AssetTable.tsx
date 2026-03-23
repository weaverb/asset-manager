import { useMemo, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import type { Asset, AssetKind, FieldSuggestions } from "../types";
import { ConfirmModal } from "../components/ConfirmModal";
import { TagInput } from "../components/TagInput";
import { KINDS } from "../lib/assetDefaults";
import { invoke } from "../tauri";
import { useAssetsList } from "../context/AssetsListContext";

function PencilIcon() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d="M17 3a2.828 2.828 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5L17 3z" />
    </svg>
  );
}

function TrashIcon() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d="M3 6h18" />
      <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
      <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" />
      <path d="M10 11v6M14 11v6" />
    </svg>
  );
}

type SortKey =
  | "kind"
  | "name"
  | "manufacturer"
  | "model"
  | "caliber"
  | "quantity"
  | "updatedAt"
  | "tags";

type SortDir = "asc" | "desc";

function sortLabel(key: SortKey): string {
  switch (key) {
    case "kind":
      return "Type";
    case "name":
      return "Name";
    case "manufacturer":
      return "Manufacturer";
    case "model":
      return "Model";
    case "caliber":
      return "Caliber";
    case "quantity":
      return "Qty";
    case "updatedAt":
      return "Updated";
    case "tags":
      return "Tags";
    default:
      return key;
  }
}

function compareAssets(a: Asset, b: Asset, key: SortKey, dir: SortDir): number {
  const mul = dir === "asc" ? 1 : -1;
  const av = a[key];
  const bv = b[key];
  if (key === "quantity") {
    const n = (Number(av) - Number(bv)) * mul;
    return n !== 0 ? n : a.name.localeCompare(b.name);
  }
  if (key === "updatedAt") {
    const n = String(av).localeCompare(String(bv)) * mul;
    return n !== 0 ? n : a.name.localeCompare(b.name);
  }
  if (key === "tags") {
    const sa = (a.tags ?? []).join(", ").toLowerCase();
    const sb = (b.tags ?? []).join(", ").toLowerCase();
    const n = sa.localeCompare(sb) * mul;
    return n !== 0 ? n : a.name.localeCompare(b.name);
  }
  const sa = (av ?? "").toString().toLowerCase();
  const sb = (bv ?? "").toString().toLowerCase();
  const n = sa.localeCompare(sb) * mul;
  return n !== 0 ? n : a.name.localeCompare(b.name);
}

export function AssetTable() {
  const {
    assets,
    refreshList,
    kindFilter,
    setKindFilter,
    tagFilters,
    setTagFilters,
    listError,
    setListError,
  } = useAssetsList();
  const navigate = useNavigate();
  const [sortKey, setSortKey] = useState<SortKey>("updatedAt");
  const [sortDir, setSortDir] = useState<SortDir>("desc");
  const [deleteTarget, setDeleteTarget] = useState<Asset | null>(null);

  const filtered = useMemo(() => {
    let rows = assets;
    if (kindFilter !== "all") {
      rows = rows.filter((a) => a.kind === kindFilter);
    }
    return rows;
  }, [assets, kindFilter]);

  const sorted = useMemo(() => {
    const rows = [...filtered];
    rows.sort((a, b) => compareAssets(a, b, sortKey, sortDir));
    return rows;
  }, [filtered, sortKey, sortDir]);

  const toggleSort = (key: SortKey) => {
    if (sortKey === key) {
      setSortDir((d) => (d === "asc" ? "desc" : "asc"));
    } else {
      setSortKey(key);
      setSortDir(key === "updatedAt" ? "desc" : "asc");
    }
  };

  const kindLabel = (k: AssetKind) =>
    KINDS.find((x) => x.value === k)?.label ?? k;

  return (
    <main className="page-main assets-page">
      {listError ? <div className="banner error">{listError}</div> : null}

      <div className="table-wrap assets-table-wrap">
        <table className="asset-table">
          <thead>
            <tr className="asset-table-filters-row">
              <th
                colSpan={9}
                scope="colgroup"
                className="asset-table-filters-cell"
              >
                <div className="assets-filters-inline">
                  <select
                    className="select"
                    value={kindFilter}
                    onChange={(e) => {
                      setListError(null);
                      setKindFilter(e.target.value as AssetKind | "all");
                    }}
                    title="Filter by type"
                  >
                    <option value="all">All types</option>
                    {KINDS.map((k) => (
                      <option key={k.value} value={k.value}>
                        {k.label}
                      </option>
                    ))}
                  </select>
                  <div
                    className="toolbar-tag-filter"
                    title="Show assets that have any of these tags"
                  >
                    <TagInput
                      variant="toolbar"
                      label="Tags"
                      tags={tagFilters}
                      onChange={(next) => {
                        setListError(null);
                        setTagFilters(next);
                      }}
                      fetchSuggestions={async (q) => {
                        const r = await invoke<FieldSuggestions>("suggest_tags", {
                          query: q,
                        });
                        return r.items;
                      }}
                      placeholder="Filter by tag…"
                    />
                  </div>
                </div>
              </th>
            </tr>
            <tr className="asset-table-columns-row">
              {(
                [
                  "kind",
                  "name",
                  "manufacturer",
                  "model",
                  "caliber",
                  "quantity",
                  "tags",
                  "updatedAt",
                ] as SortKey[]
              ).map((key) => (
                <th key={key}>
                  <button
                    type="button"
                    className="th-sort"
                    onClick={() => toggleSort(key)}
                  >
                    {sortLabel(key)}
                    {sortKey === key ? (
                      <span className="sort-ind">
                        {sortDir === "asc" ? " ↑" : " ↓"}
                      </span>
                    ) : null}
                  </button>
                </th>
              ))}
              <th className="col-actions">Actions</th>
            </tr>
          </thead>
          <tbody>
            {sorted.map((a) => (
              <tr key={a.id}>
                <td>{kindLabel(a.kind)}</td>
                <td className="td-strong">{a.name}</td>
                <td>{a.manufacturer ?? "—"}</td>
                <td>{a.model ?? "—"}</td>
                <td>{a.caliber ?? "—"}</td>
                <td>{a.quantity}</td>
                <td className="td-tags">
                  {(a.tags ?? []).length > 0
                    ? (a.tags ?? []).join(", ")
                    : "—"}
                </td>
                <td className="td-muted">
                  {a.updatedAt?.slice(0, 10) ?? "—"}
                </td>
                <td className="col-actions">
                  <div
                    className="row-action-group"
                    role="group"
                    aria-label={`Actions for ${a.name}`}
                  >
                    <button
                      type="button"
                      className="row-action-btn"
                      onClick={() => navigate(`/assets/${a.id}`)}
                      aria-label={`Edit ${a.name}`}
                      title="Edit"
                    >
                      <PencilIcon />
                    </button>
                    <button
                      type="button"
                      className="row-action-btn row-action-btn--danger"
                      onClick={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        setDeleteTarget(a);
                      }}
                      aria-label={`Delete ${a.name}`}
                      title="Delete"
                    >
                      <TrashIcon />
                    </button>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {sorted.length === 0 ? (
          <p className="empty-table">
            No assets match.{" "}
            <Link to="/assets/new">Add an asset</Link> or adjust filters.
          </p>
        ) : null}
      </div>

      {deleteTarget ? (
        <ConfirmModal
          title="Delete this asset?"
          message={`Remove “${deleteTarget.name}” and its photos? This cannot be undone.`}
          confirmLabel="Delete"
          onCancel={() => setDeleteTarget(null)}
          onConfirm={() => {
            setListError(null);
            void (async () => {
              try {
                await invoke("delete_asset", {
                  id: String(deleteTarget.id),
                });
                setDeleteTarget(null);
                await refreshList();
              } catch (e) {
                setListError(String(e));
                setDeleteTarget(null);
              }
            })();
          }}
        />
      ) : null}
    </main>
  );
}
