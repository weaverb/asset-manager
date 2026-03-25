import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import type { DashboardStats, RangeDaySummary } from "../types";
import { invoke } from "../tauri";
import { useToast } from "../context/ToastContext";
import { AmmoCaliberDonut } from "../components/dashboard/AmmoCaliberDonut";
import { UpcomingMaintenanceWidget } from "../components/dashboard/UpcomingMaintenanceWidget";
import { TopFirearmsWidget } from "../components/dashboard/TopFirearmsWidget";

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

export function DashboardPage() {
  const { pushToast } = useToast();
  const [rangeDays, setRangeDays] = useState<RangeDaySummary[]>([]);
  const [rangeListFailed, setRangeListFailed] = useState(false);
  const [stats, setStats] = useState<DashboardStats | null>(null);
  const [statsFailed, setStatsFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setRangeListFailed(false);
    void (async () => {
      try {
        const list = await invoke<RangeDaySummary[]>("list_range_days");
        if (!cancelled) setRangeDays(list);
      } catch (e) {
        if (!cancelled) {
          pushToast(String(e), "error");
          setRangeListFailed(true);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [pushToast]);

  useEffect(() => {
    let cancelled = false;
    setStatsFailed(false);
    void (async () => {
      try {
        const s = await invoke<DashboardStats>("get_dashboard_stats");
        if (!cancelled) setStats(s);
      } catch (e) {
        if (!cancelled) {
          pushToast(String(e), "error");
          setStatsFailed(true);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [pushToast]);

  const today = useMemo(() => new Date().toISOString().slice(0, 10), []);

  const upcomingPlanned = useMemo(() => {
    return rangeDays
      .filter(
        (d) => d.status === "planned" && d.scheduledDate >= today,
      )
      .sort((a, b) => a.scheduledDate.localeCompare(b.scheduledDate))
      .slice(0, 5);
  }, [rangeDays, today]);

  const recentCompleted = useMemo(() => {
    return rangeDays
      .filter((d) => d.status === "completed")
      .slice(0, 5);
  }, [rangeDays]);

  const ammoTotalRounds = useMemo(() => {
    if (!stats) return 0;
    return stats.ammoByCaliber.reduce((sum, r) => sum + r.rounds, 0);
  }, [stats]);

  return (
    <main className="page-main dashboard-page">
      <div className="dashboard-layout">
        <div className="dashboard-main-col">
          <div className="dashboard-intro">
            <h2>Dashboard</h2>
            <p className="dashboard-lead">
              Overview and shortcuts. Open your inventory to add or edit assets,
              or plan a range day to log rounds fired.
            </p>
            <Link to="/assets" className="dashboard-cta">
              View all assets
            </Link>
          </div>

          <section className="dashboard-range-section">
            <div className="dashboard-section-head">
              <h3>Range days</h3>
              <div className="dashboard-section-actions">
                <Link to="/range-days" className="link-inline">
                  View all
                </Link>
                <Link
                  to="/range-days/new"
                  className="dashboard-cta dashboard-cta--small"
                >
                  New range day
                </Link>
              </div>
            </div>
            {rangeListFailed ? (
              <p className="muted">
                Couldn&rsquo;t load range days. See the notification.
              </p>
            ) : (
              <>
                <h4 className="dashboard-subheading">Upcoming (planned)</h4>
                {upcomingPlanned.length === 0 ? (
                  <p className="muted">No upcoming planned days.</p>
                ) : (
                  <ul className="dashboard-range-list">
                    {upcomingPlanned.map((d) => (
                      <li key={d.id}>
                        <Link to={`/range-days/${d.id}`}>
                          {d.scheduledDate}
                        </Link>
                        <span className="muted">
                          {d.itemCount} firearm{d.itemCount === 1 ? "" : "s"}
                        </span>
                      </li>
                    ))}
                  </ul>
                )}
                <h4 className="dashboard-subheading">Recent completed</h4>
                {recentCompleted.length === 0 ? (
                  <p className="muted">No completed days yet.</p>
                ) : (
                  <ul className="dashboard-range-list">
                    {recentCompleted.map((d) => (
                      <li key={d.id}>
                        <Link to={`/range-days/${d.id}`}>
                          {d.scheduledDate}
                        </Link>
                        <span className={`status-pill status-pill--${d.status}`}>
                          {statusLabel(d.status)}
                        </span>
                      </li>
                    ))}
                  </ul>
                )}
              </>
            )}
          </section>
        </div>

        <aside className="dashboard-widgets-col" aria-label="Dashboard widgets">
          {statsFailed ? (
            <p className="muted dashboard-widget">
              Couldn&rsquo;t load dashboard stats. See the notification.
            </p>
          ) : stats ? (
            <>
              <section className="dashboard-widget">
                <h3 className="dashboard-widget-title">Ammo by caliber</h3>
                <p className="dashboard-widget-total" aria-live="polite">
                  {ammoTotalRounds.toLocaleString()} rounds total
                </p>
                <p className="muted dashboard-widget-lead">
                  Rounds on hand (inventory quantity), grouped by caliber.
                </p>
                <AmmoCaliberDonut rows={stats.ammoByCaliber} />
              </section>
              <section className="dashboard-widget">
                <h3 className="dashboard-widget-title">Upcoming maintenance</h3>
                <p className="muted dashboard-widget-lead">
                  Firearms within 10% of a round interval or day interval (or
                  overdue). Open the asset to adjust intervals or log service.
                </p>
                <UpcomingMaintenanceWidget rows={stats.upcomingMaintenance} />
              </section>
              <section className="dashboard-widget">
                <h3 className="dashboard-widget-title">Top firearms</h3>
                <p className="muted dashboard-widget-lead">
                  By lifetime rounds fired, then completed range day count.
                </p>
                <TopFirearmsWidget rows={stats.topFirearms} />
              </section>
            </>
          ) : (
            <p className="muted dashboard-widget">Loading widgets…</p>
          )}
        </aside>
      </div>
    </main>
  );
}
