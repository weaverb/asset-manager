import { Link } from "react-router-dom";

export function DashboardPage() {
  return (
    <main className="page-main dashboard-page">
      <div className="dashboard-intro">
        <h2>Dashboard</h2>
        <p className="dashboard-lead">
          Overview and widgets will live here. For now, open your inventory to
          add or edit assets.
        </p>
        <Link to="/assets" className="dashboard-cta">
          View all assets
        </Link>
      </div>
    </main>
  );
}
