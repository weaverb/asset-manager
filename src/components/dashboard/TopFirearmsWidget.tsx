import { Link } from "react-router-dom";
import type { DashboardTopFirearmRow } from "../../types";

export function TopFirearmsWidget({
  rows,
}: {
  rows: DashboardTopFirearmRow[];
}) {
  if (rows.length === 0) {
    return (
      <p className="muted dashboard-widget-empty">
        No firearms yet. Add a firearm and complete range days to see usage
        here.
      </p>
    );
  }

  return (
    <ol className="dashboard-top-firearms-list">
      {rows.map((r, i) => (
        <li key={r.assetId}>
          <span className="dashboard-top-rank">{i + 1}.</span>
          <div className="dashboard-top-body">
            <Link to={`/assets/${r.assetId}`} className="dashboard-top-link">
              {r.name}
            </Link>
            <span className="muted dashboard-top-meta">
              {r.lifetimeRoundsFired.toLocaleString()} rounds ·{" "}
              {r.completedRangeDayCount} completed range day
              {r.completedRangeDayCount === 1 ? "" : "s"}
            </span>
          </div>
        </li>
      ))}
    </ol>
  );
}
