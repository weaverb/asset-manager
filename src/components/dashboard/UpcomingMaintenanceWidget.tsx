import { Link } from "react-router-dom";
import type { DashboardUpcomingMaintenanceRow } from "../../types";

export function UpcomingMaintenanceWidget({
  rows,
}: {
  rows: DashboardUpcomingMaintenanceRow[];
}) {
  if (rows.length === 0) {
    return (
      <p className="muted dashboard-widget-empty">
        No firearms are within 10% of a configured maintenance threshold. Set
        intervals on a firearm asset to see reminders here.
      </p>
    );
  }

  return (
    <ul className="dashboard-maint-list">
      {rows.map((r) => (
        <li key={r.assetId}>
          <Link to={`/assets/${r.assetId}`} className="dashboard-maint-link">
            {r.name}
          </Link>
          <p className="dashboard-maint-summary muted">{r.summary}</p>
        </li>
      ))}
    </ul>
  );
}
