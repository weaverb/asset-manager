import { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import type { Asset, RangeDayDetail } from "../types";
import { invoke } from "../tauri";
import { FirearmChecklist } from "../components/FirearmChecklist";
import { useToast } from "../context/ToastContext";

function defaultDate(): string {
  return new Date().toISOString().slice(0, 10);
}

export function RangeDayNewPage() {
  const navigate = useNavigate();
  const { pushToast } = useToast();
  const [scheduledDate, setScheduledDate] = useState(defaultDate);
  const [firearms, setFirearms] = useState<Asset[]>([]);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const list = await invoke<Asset[]>("list_assets", {
          kind: "firearm",
          tagNames: null,
        });
        if (!cancelled) setFirearms(list);
      } catch (e) {
        if (!cancelled) pushToast(String(e), "error");
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [pushToast]);

  const create = async () => {
    setSaving(true);
    try {
      const detail = await invoke<RangeDayDetail>("create_range_day", {
        scheduledDate,
        assetIds: selectedIds,
      });
      pushToast("Range day created.", "success");
      navigate(`/range-days/${detail.id}`, { replace: true });
    } catch (e) {
      pushToast(String(e), "error");
    } finally {
      setSaving(false);
    }
  };

  return (
    <main className="page-main range-day-new-page">
      <div className="page-head-row">
        <h2>New range day</h2>
        <Link to="/range-days" className="link-back">
          Back to list
        </Link>
      </div>
      <label className="block-label">
        Scheduled date
        <input
          type="date"
          value={scheduledDate}
          onChange={(e) => setScheduledDate(e.target.value)}
        />
      </label>
      <fieldset className="firearm-fieldset">
        <legend>Firearms to bring</legend>
        <FirearmChecklist
          firearms={firearms}
          selectedIds={selectedIds}
          onChange={setSelectedIds}
        />
      </fieldset>
      <div className="form-actions-row">
        <button
          type="button"
          className="primary"
          disabled={saving || selectedIds.length === 0}
          onClick={() => void create()}
        >
          {saving ? "Creating…" : "Create range day"}
        </button>
      </div>
    </main>
  );
}
