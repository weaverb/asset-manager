import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { isTauri } from "./tauri";
import "./App.css";
import { AppShell } from "./AppShell";
import { AssetDrawerRoute } from "./pages/AssetDrawerRoute";
import { AssetsLayout, EmptyDrawerSlot } from "./pages/AssetsLayout";
import { DashboardPage } from "./pages/DashboardPage";
import { RangeDayDetailPage } from "./pages/RangeDayDetailPage";
import { RangeDayNewPage } from "./pages/RangeDayNewPage";
import { RangeDaysPage } from "./pages/RangeDaysPage";

function NotTauri() {
  return (
    <div className="app">
      <div className="not-tauri">
        <h1>Run the desktop app</h1>
        <p>
          This UI must run inside the Tauri window so it can talk to the Rust
          backend and SQLite. Opening <code>http://localhost:1420</code> in
          Chrome or Safari will not work.
        </p>
        <p className="mono-block">npm run tauri dev</p>
        <p className="muted">
          Use <code>npm run dev</code> only if you are developing UI without IPC
          (Rust commands will stay unavailable).
        </p>
      </div>
    </div>
  );
}

export default function App() {
  if (!isTauri()) {
    return <NotTauri />;
  }

  return (
    <BrowserRouter>
      <Routes>
        <Route element={<AppShell />}>
          <Route index element={<DashboardPage />} />
          <Route path="range-days" element={<RangeDaysPage />} />
          <Route path="range-days/new" element={<RangeDayNewPage />} />
          <Route path="range-days/:rangeDayId" element={<RangeDayDetailPage />} />
          <Route path="assets" element={<AssetsLayout />}>
            <Route index element={<EmptyDrawerSlot />} />
            <Route path="new" element={<AssetDrawerRoute />} />
            <Route path=":assetId" element={<AssetDrawerRoute />} />
          </Route>
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
