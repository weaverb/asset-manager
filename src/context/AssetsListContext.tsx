import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import { useLocation } from "react-router-dom";
import type { Asset, AssetKind } from "../types";
import { invoke } from "../tauri";

export type AssetsListContextValue = {
  assets: Asset[];
  refreshList: () => Promise<void>;
  query: string;
  setQuery: (q: string) => void;
  kindFilter: AssetKind | "all";
  setKindFilter: (k: AssetKind | "all") => void;
  tagFilters: string[];
  setTagFilters: (t: string[]) => void;
  listError: string | null;
  setListError: (e: string | null) => void;
};

const AssetsListContext = createContext<AssetsListContextValue | null>(null);

export function AssetsListProvider({ children }: { children: ReactNode }) {
  const location = useLocation();
  const onAssetsRoute = location.pathname.startsWith("/assets");

  const [query, setQuery] = useState("");
  const [kindFilter, setKindFilter] = useState<AssetKind | "all">("all");
  const [tagFilters, setTagFilters] = useState<string[]>([]);
  const [assets, setAssets] = useState<Asset[]>([]);
  const [listError, setListError] = useState<string | null>(null);

  const refreshList = useCallback(async () => {
    setListError(null);
    try {
      const q = query.trim();
      const tagNames = tagFilters.length > 0 ? tagFilters : null;
      const list =
        q.length > 0
          ? await invoke<Asset[]>("search_assets", {
              query: q,
              tagNames,
            })
          : await invoke<Asset[]>("list_assets", {
              kind: kindFilter === "all" ? null : kindFilter,
              tagNames,
            });
      const filtered =
        q.length > 0 && kindFilter !== "all"
          ? list.filter((a) => a.kind === kindFilter)
          : list;
      setAssets(filtered);
    } catch (e) {
      setListError(String(e));
    }
  }, [query, kindFilter, tagFilters]);

  useEffect(() => {
    if (!onAssetsRoute) return;
    const t = window.setTimeout(() => {
      void refreshList();
    }, 200);
    return () => window.clearTimeout(t);
  }, [onAssetsRoute, refreshList]);

  const value = useMemo(
    () => ({
      assets,
      refreshList,
      query,
      setQuery,
      kindFilter,
      setKindFilter,
      tagFilters,
      setTagFilters,
      listError,
      setListError,
    }),
    [assets, refreshList, query, kindFilter, tagFilters, listError],
  );

  return (
    <AssetsListContext.Provider value={value}>{children}</AssetsListContext.Provider>
  );
}

export function useAssetsList() {
  const v = useContext(AssetsListContext);
  if (!v) {
    throw new Error("useAssetsList must be used within AssetsListProvider");
  }
  return v;
}
