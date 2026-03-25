import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import type { RangeDaySummary } from "../types";
import { invoke } from "../tauri";
import { useToast } from "../context/ToastContext";

function statusLabel(status: string): string {
  switch (status) {
    case "planned":
      return "Planned";
    case "completed":
      return "Completed";
    case "cancelled":
      return "Cancelled";
    default:
      return status;
  }
}

export function RangeDaysPage() {
  const { pushToast } = useToast();
  const [rows, setRows] = useState<RangeDaySummary[]>([]);
  const [listFailed, setListFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setListFailed(false);
    void (async () => {
      try {
        const list = await invoke<RangeDaySummary[]>("list_range_days");
        if (!cancelled) setRows(list);
      } catch (e) {
        if (!cancelled) {
          pushToast(String(e), "error");
          setListFailed(true);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [pushToast]);

  return (
    <main className="page-main range-days-page">
      <div className="page-head-row">
        <h2>Range days</h2>
        <Link to="/range-days/new" className="dashboard-cta dashboard-cta--small">
          New range day
        </Link>
      </div>
      <p className="dashboard-lead">
        Schedule trips, attach firearms, then complete a day to log rounds fired.
      </p>
      {listFailed ? (
        <p className="muted">
          Couldn&rsquo;t load range days. See the notification.
        </p>
      ) : rows.length === 0 ? (
        <p className="muted">No range days yet. Create one to get started.</p>
      ) : (
        <div className="table-wrap">
          <table className="data-table">
            <thead>
              <tr>
                <th>Date</th>
                <th>Status</th>
                <th>Firearms</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <tr key={r.id}>
                  <td>{r.scheduledDate}</td>
                  <td>
                    <span className={`status-pill status-pill--${r.status}`}>
                      {statusLabel(r.status)}
                    </span>
                  </td>
                  <td>{r.itemCount}</td>
                  <td>
                    <Link to={`/range-days/${r.id}`} className="link-inline">
                      Open
                    </Link>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </main>
  );
}
