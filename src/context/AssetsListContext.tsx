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
import { useToast } from "./ToastContext";

export type AssetsListContextValue = {
  assets: Asset[];
  refreshList: () => Promise<void>;
  query: string;
  setQuery: (q: string) => void;
  kindFilter: AssetKind | "all";
  setKindFilter: (k: AssetKind | "all") => void;
  tagFilters: string[];
  setTagFilters: (t: string[]) => void;
};

const AssetsListContext = createContext<AssetsListContextValue | null>(null);

export function AssetsListProvider({ children }: { children: ReactNode }) {
  const location = useLocation();
  const onAssetsRoute = location.pathname.startsWith("/assets");
  const { pushToast } = useToast();

  const [query, setQuery] = useState("");
  const [kindFilter, setKindFilter] = useState<AssetKind | "all">("all");
  const [tagFilters, setTagFilters] = useState<string[]>([]);
  const [assets, setAssets] = useState<Asset[]>([]);

  const refreshList = useCallback(async () => {
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
      pushToast(String(e), "error");
    }
  }, [query, kindFilter, tagFilters, pushToast]);

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
    }),
    [assets, refreshList, query, kindFilter, tagFilters],
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
