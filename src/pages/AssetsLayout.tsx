import { Outlet } from "react-router-dom";
import { AssetTable } from "./AssetTable";

function EmptyDrawerSlot() {
  return null;
}

export function AssetsLayout() {
  return (
    <div className="assets-layout">
      <AssetTable />
      <Outlet />
    </div>
  );
}

export { EmptyDrawerSlot };
