import { useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import type {
  Asset,
  RangeDayAmmoConsumptionEntry,
  RangeDayDetail,
  RangeDayRoundEntry,
} from "../types";
import { invoke } from "../tauri";
import { DigitsOnlyInput } from "../components/DigitsOnlyInput";
import { FirearmChecklist } from "../components/FirearmChecklist";
import { ConfirmModal } from "../components/ConfirmModal";
import { useToast } from "../context/ToastContext";
import { parseNonNegInt } from "../lib/parseNumeric";

function normalizeCaliber(cal: string | null | undefined): string | null {
  const t = cal?.trim().toLowerCase();
  return t && t.length > 0 ? t : null;
}

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

export function RangeDayDetailPage() {
  const { rangeDayId } = useParams<{ rangeDayId: string }>();
  const navigate = useNavigate();
  const { pushToast } = useToast();
  const [detail, setDetail] = useState<RangeDayDetail | null>(null);
  const [firearms, setFirearms] = useState<Asset[]>([]);
  const [scheduledDate, setScheduledDate] = useState("");
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [completeNotes, setCompleteNotes] = useState("");
  const [roundFieldText, setRoundFieldText] = useState<Record<string, string>>(
    {},
  );
  const [detailLoadFailed, setDetailLoadFailed] = useState(false);
  const [saving, setSaving] = useState(false);
  const [cancelConfirm, setCancelConfirm] = useState(false);
  const [deleteConfirm, setDeleteConfirm] = useState(false);
  const [ammunition, setAmmunition] = useState<Asset[]>([]);
  const [ammoSavingFirearmId, setAmmoSavingFirearmId] = useState<string | null>(
    null,
  );
  const [ammoFieldText, setAmmoFieldText] = useState<
    Record<string, Record<string, string>>
  >({});

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
  }, []);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const list = await invoke<Asset[]>("list_assets", {
          kind: "ammunition",
          tagNames: null,
        });
        if (!cancelled) setAmmunition(list);
      } catch {
        if (!cancelled) setAmmunition([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [pushToast]);

  useEffect(() => {
    if (!rangeDayId) return;
    setDetailLoadFailed(false);
    let cancelled = false;
    void (async () => {
      try {
        const d = await invoke<RangeDayDetail>("get_range_day", {
          id: rangeDayId,
        });
        if (cancelled) return;
        setDetailLoadFailed(false);
        setDetail(d);
        setScheduledDate(d.scheduledDate);
        setSelectedIds(d.items.map((i) => i.assetId));
        setRoundFieldText((prev) => {
          const next = { ...prev };
          for (const it of d.items) {
            if (next[it.assetId] === undefined) next[it.assetId] = "0";
          }
          return next;
        });
        setCompleteNotes(d.status === "planned" ? "" : (d.notes ?? ""));
      } catch (e) {
        if (!cancelled) {
          pushToast(String(e), "error");
          setDetailLoadFailed(true);
          setDetail(null);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [rangeDayId, pushToast]);

  const ammoLinkKey =
    detail?.ammoLinks
      ?.map((l) => `${l.firearmAssetId}:${l.ammunitionAssetId}`)
      .sort()
      .join("|") ?? "";

  useEffect(() => {
    if (!detail || detail.status !== "planned") return;
    const links = detail.ammoLinks ?? [];
    setAmmoFieldText((prev) => {
      const next: Record<string, Record<string, string>> = {};
      for (const l of links) {
        if (!next[l.firearmAssetId]) next[l.firearmAssetId] = {};
        const v = prev[l.firearmAssetId]?.[l.ammunitionAssetId];
        next[l.firearmAssetId][l.ammunitionAssetId] =
          v !== undefined ? v : "0";
      }
      return next;
    });
  }, [detail?.id, detail?.status, detail?.updatedAt, ammoLinkKey]);

  const saveAmmoSelection = async (firearmId: string, nextIds: string[]) => {
    if (!rangeDayId || !detail || detail.status !== "planned") return;
    setAmmoSavingFirearmId(firearmId);
    try {
      const d = await invoke<RangeDayDetail>("set_range_day_firearm_ammunition", {
        id: rangeDayId,
        firearmAssetId: firearmId,
        ammunitionAssetIds: nextIds,
      });
      setDetail(d);
    } catch (e) {
      pushToast(String(e), "error");
    } finally {
      setAmmoSavingFirearmId(null);
    }
  };

  const toggleAmmo = (firearmId: string, ammoId: string, checked: boolean) => {
    if (!detail) return;
    const links = detail.ammoLinks ?? [];
    const current = links
      .filter((l) => l.firearmAssetId === firearmId)
      .map((l) => l.ammunitionAssetId);
    let next: string[];
    if (checked) {
      if (current.includes(ammoId)) return;
      next = [...current, ammoId];
    } else {
      next = current.filter((id) => id !== ammoId);
    }
    void saveAmmoSelection(firearmId, next);
  };

  const allowedCaliberForFirearm = (firearmId: string): string | null => {
    const links = detail?.ammoLinks ?? [];
    const forGun = links.filter((l) => l.firearmAssetId === firearmId);
    const fromAmmo = normalizeCaliber(forGun[0]?.ammunitionCaliber ?? null);
    if (fromAmmo) return fromAmmo;
    const f = firearms.find((x) => x.id === firearmId);
    return normalizeCaliber(f?.caliber ?? null);
  };

  const selectCandidateAmmo = (firearmId: string): Asset[] => {
    const lock = allowedCaliberForFirearm(firearmId);
    return ammunition.filter((a) => {
      const ac = normalizeCaliber(a.caliber);
      if (!ac) return false;
      if (lock === null) return true;
      return ac === lock;
    });
  };

  const savePlanned = async (closeAfter = false) => {
    if (!rangeDayId || !detail || detail.status !== "planned") return;
    setSaving(true);
    try {
      const d = await invoke<RangeDayDetail>("update_range_day_planned", {
        id: rangeDayId,
        scheduledDate,
        assetIds: selectedIds,
      });
      setDetail(d);
      setRoundFieldText((prev) => {
        const next = { ...prev };
        for (const it of d.items) {
          if (next[it.assetId] === undefined) next[it.assetId] = "0";
        }
        return next;
      });
      pushToast("Range day updated.", "success");
      if (closeAfter) navigate("/range-days");
    } catch (e) {
      pushToast(String(e), "error");
    } finally {
      setSaving(false);
    }
  };

  const applyFirearmsLabel =
    selectedIds.length > 1 ? "Apply Firearms" : "Apply Firearm";

  const submitComplete = async () => {
    if (!rangeDayId || !detail || detail.status !== "planned") return;
    const rounds: RangeDayRoundEntry[] = detail.items.map((it) => ({
      assetId: it.assetId,
      roundsFired: parseNonNegInt(roundFieldText[it.assetId]),
    }));
    const ammoUse: RangeDayAmmoConsumptionEntry[] = [];
    for (const it of detail.items) {
      const inner = ammoFieldText[it.assetId];
      if (!inner) continue;
      for (const [ammId, r] of Object.entries(inner)) {
        ammoUse.push({
          firearmAssetId: it.assetId,
          ammunitionAssetId: ammId,
          rounds: parseNonNegInt(r),
        });
      }
    }
    setSaving(true);
    try {
      const d = await invoke<RangeDayDetail>("complete_range_day", {
        id: rangeDayId,
        notes: completeNotes.trim() || null,
        rounds,
        ammoConsumption: ammoUse,
      });
      setDetail(d);
      pushToast("Range day completed.", "success");
    } catch (e) {
      pushToast(String(e), "error");
    } finally {
      setSaving(false);
    }
  };

  const runCancel = async () => {
    if (!rangeDayId) return;
    setCancelConfirm(false);
    try {
      await invoke("cancel_range_day", { id: rangeDayId });
      navigate("/range-days");
    } catch (e) {
      pushToast(String(e), "error");
    }
  };

  const runDelete = async () => {
    if (!rangeDayId) return;
    setDeleteConfirm(false);
    try {
      await invoke("delete_range_day", { id: rangeDayId });
      navigate("/range-days");
    } catch (e) {
      pushToast(String(e), "error");
    }
  };

  if (!rangeDayId) {
    return (
      <main className="page-main">
        <p className="muted">Missing range day id.</p>
      </main>
    );
  }

  if (!detail && !detailLoadFailed) {
    return (
      <main className="page-main">
        <p className="muted">Loading…</p>
      </main>
    );
  }

  if (!detail) {
    return (
      <main className="page-main">
        <p className="muted">
          Couldn&rsquo;t load this range day. See the notification.
        </p>
        <Link to="/range-days">Back to list</Link>
      </main>
    );
  }

  const isPlanned = detail.status === "planned";

  const fillRoundsFromAssignedStock = (firearmId: string) => {
    const links = (detail?.ammoLinks ?? []).filter(
      (l) => l.firearmAssetId === firearmId,
    );
    const total = links.reduce((s, l) => s + l.quantityOnHand, 0);
    setRoundFieldText((prev) => ({ ...prev, [firearmId]: String(total) }));
  };

  const fillSplitFromAmmoStock = (
    firearmId: string,
    ammunitionAssetId: string,
    quantityOnHand: number,
  ) => {
    setAmmoFieldText((prev) => ({
      ...prev,
      [firearmId]: {
        ...(prev[firearmId] ?? {}),
        [ammunitionAssetId]: String(quantityOnHand),
      },
    }));
  };

  const ammoSplitOk = detail.items.every((it) => {
    const links = (detail.ammoLinks ?? []).filter(
      (l) => l.firearmAssetId === it.assetId,
    );
    if (links.length === 0) return true;
    const sum = links.reduce(
      (s, l) =>
        s +
        parseNonNegInt(
          ammoFieldText[it.assetId]?.[l.ammunitionAssetId],
        ),
      0,
    );
    return sum === parseNonNegInt(roundFieldText[it.assetId]);
  });

  return (
    <main className="page-main range-day-detail-page">
      <div className="page-head-row">
        <h2>Range day</h2>
        <Link to="/range-days" className="link-back">
          Back to list
        </Link>
      </div>
      <div className="range-day-meta">
        <p>
          <strong>Date:</strong> {detail.scheduledDate}{" "}
          <span className={`status-pill status-pill--${detail.status}`}>
            {statusLabel(detail.status)}
          </span>
        </p>
        {detail.completedAt ? (
          <p className="muted">
            Completed {new Date(detail.completedAt).toLocaleString()}
          </p>
        ) : null}
      </div>

      {isPlanned ? (
        <>
          <h3>Edit plan</h3>
          <label className="block-label">
            Scheduled date
            <input
              type="date"
              value={scheduledDate}
              onChange={(e) => setScheduledDate(e.target.value)}
            />
          </label>
          <fieldset className="firearm-fieldset">
            <legend>Firearms</legend>
            <FirearmChecklist
              firearms={firearms}
              selectedIds={selectedIds}
              onChange={setSelectedIds}
            />
          </fieldset>
          <div className="form-actions-row">
            <button
              type="button"
              className="primary ghost"
              disabled={saving || selectedIds.length === 0}
              onClick={() => void savePlanned(false)}
            >
              {saving ? "Saving…" : applyFirearmsLabel}
            </button>
          </div>

          <h3 className="range-day-subhead">Ammunition checkout</h3>
          <p className="muted">
            Assign inventory ammo to each gun. Only one caliber per firearm; you
            can select multiple boxes of that caliber. Each box can only be on one
            gun for this day.
          </p>
          {detail.items.map((it) => {
            const candidates = selectCandidateAmmo(it.assetId);
            return (
              <fieldset className="firearm-fieldset" key={it.assetId}>
                <legend>{it.name}</legend>
                {candidates.length === 0 ? (
                  <p className="muted">
                    No matching ammunition in inventory (needs caliber, and must
                    match the gun once you pick a box).
                  </p>
                ) : (
                  <ul className="firearm-checklist">
                    {candidates.map((a) => {
                      const assigned = (detail.ammoLinks ?? []).some(
                        (l) =>
                          l.firearmAssetId === it.assetId &&
                          l.ammunitionAssetId === a.id,
                      );
                      return (
                        <li key={a.id}>
                          <label className="firearm-check-label">
                            <input
                              type="checkbox"
                              checked={assigned}
                              disabled={ammoSavingFirearmId === it.assetId}
                              onChange={(e) =>
                                toggleAmmo(
                                  it.assetId,
                                  a.id,
                                  e.target.checked,
                                )
                              }
                            />
                            <span>
                              {a.name} ({a.caliber ?? "?"}) — stock{" "}
                              {a.quantity.toLocaleString()}
                            </span>
                          </label>
                        </li>
                      );
                    })}
                  </ul>
                )}
              </fieldset>
            );
          })}

          <hr className="section-rule" />

          <h3>Complete range day</h3>
          <p className="muted">
            Log rounds per firearm. Totals update on each gun for maintenance
            tracking.
          </p>
          <label className="block-label">
            Notes
            <textarea
              rows={4}
              value={completeNotes}
              onChange={(e) => setCompleteNotes(e.target.value)}
              placeholder="Optional notes about this trip…"
            />
          </label>
          <ul className="rounds-input-list">
            {detail.items.map((it) => {
              const assignedLinks = (detail.ammoLinks ?? []).filter(
                (l) => l.firearmAssetId === it.assetId,
              );
              const stockSum = assignedLinks.reduce(
                (s, l) => s + l.quantityOnHand,
                0,
              );
              return (
                <li key={it.assetId}>
                  <label>
                    {it.name}
                    <div className="rounds-input-row">
                      <DigitsOnlyInput
                        aria-label={`Rounds fired for ${it.name}`}
                        value={roundFieldText[it.assetId] ?? "0"}
                        onChange={(digits) =>
                          setRoundFieldText((prev) => ({
                            ...prev,
                            [it.assetId]: digits,
                          }))
                        }
                      />
                      <button
                        type="button"
                        className="ghost rounds-fill-btn"
                        disabled={assignedLinks.length === 0 || stockSum === 0}
                        title={
                          assignedLinks.length === 0
                            ? "Assign ammunition to this firearm first"
                            : `Set to ${stockSum.toLocaleString()} (sum of on-hand for assigned ammo)`
                        }
                        onClick={() => fillRoundsFromAssignedStock(it.assetId)}
                      >
                        Use stock
                      </button>
                    </div>
                  </label>
                </li>
              );
            })}
          </ul>
          {(detail.ammoLinks ?? []).length > 0 ? (
            <>
              <h4 className="dashboard-subheading">Rounds from inventory</h4>
              <p className="muted">
                Split each firearm&rsquo;s total across assigned boxes. The sum per
                gun must match rounds fired above; quantities are deducted from
                inventory.
              </p>
              {detail.items.map((it) => {
                const links = (detail.ammoLinks ?? []).filter(
                  (l) => l.firearmAssetId === it.assetId,
                );
                if (links.length === 0) return null;
                const sum = links.reduce(
                  (s, l) =>
                    s +
                    parseNonNegInt(
                      ammoFieldText[it.assetId]?.[l.ammunitionAssetId],
                    ),
                  0,
                );
                const target = parseNonNegInt(roundFieldText[it.assetId]);
                const ok = sum === target;
                return (
                  <div key={it.assetId} className="ammo-split-block">
                    <p className="ammo-split-head">
                      <strong>{it.name}</strong> — split {sum} / {target}{" "}
                      {!ok ? (
                        <span className="text-warn">(must match)</span>
                      ) : null}
                    </p>
                    <ul className="rounds-input-list">
                      {links.map((l) => (
                        <li key={l.ammunitionAssetId}>
                          <label>
                            {l.ammunitionName} (stock{" "}
                            {l.quantityOnHand.toLocaleString()})
                            <div className="rounds-input-row">
                              <DigitsOnlyInput
                                aria-label={`Rounds from ${l.ammunitionName} for ${it.name}`}
                                value={
                                  ammoFieldText[it.assetId]?.[
                                    l.ammunitionAssetId
                                  ] ?? "0"
                                }
                                onChange={(digits) =>
                                  setAmmoFieldText((prev) => ({
                                    ...prev,
                                    [it.assetId]: {
                                      ...(prev[it.assetId] ?? {}),
                                      [l.ammunitionAssetId]: digits,
                                    },
                                  }))
                                }
                              />
                              <button
                                type="button"
                                className="ghost rounds-fill-btn"
                                disabled={l.quantityOnHand === 0}
                                title={`Set to on-hand quantity (${l.quantityOnHand.toLocaleString()})`}
                                onClick={() =>
                                  fillSplitFromAmmoStock(
                                    it.assetId,
                                    l.ammunitionAssetId,
                                    l.quantityOnHand,
                                  )
                                }
                              >
                                Use stock
                              </button>
                            </div>
                          </label>
                        </li>
                      ))}
                    </ul>
                  </div>
                );
              })}
            </>
          ) : null}
          <div className="form-actions-row">
            <button
              type="button"
              className="primary"
              disabled={
                saving || detail.items.length === 0 || !ammoSplitOk
              }
              onClick={() => void submitComplete()}
            >
              {saving ? "Completing…" : "Complete range day"}
            </button>
          </div>

          <div className="range-day-planned-footer">
            <button
              type="button"
              className="primary ghost"
              disabled={saving || selectedIds.length === 0}
              onClick={() => void savePlanned(true)}
            >
              {saving ? "Saving…" : "Save and close"}
            </button>
            <div className="range-day-planned-footer-danger">
              <button
                type="button"
                className="ghost danger"
                onClick={() => setCancelConfirm(true)}
              >
                Cancel range day
              </button>
              <button
                type="button"
                className="ghost danger"
                onClick={() => setDeleteConfirm(true)}
              >
                Delete range day
              </button>
            </div>
          </div>
        </>
      ) : (
        <>
          {detail.notes ? (
            <section className="range-day-notes">
              <h3>Notes</h3>
              <p className="preserve-lines">{detail.notes}</p>
            </section>
          ) : null}
          <h3>Rounds logged</h3>
          <ul className="rounds-summary-list">
            {detail.items.map((it) => (
              <li key={it.assetId}>
                <strong>{it.name}</strong>
                <span>{it.roundsFired ?? 0} rounds</span>
              </li>
            ))}
          </ul>
          <h3>Ammunition deducted</h3>
          {(detail.ammoLinks ?? []).length === 0 ? (
            <p className="muted">No ammunition was assigned to this day.</p>
          ) : (
            <ul className="ammo-complete-summary">
              {detail.items.map((it) => {
                const links = (detail.ammoLinks ?? []).filter(
                  (l) => l.firearmAssetId === it.assetId,
                );
                if (links.length === 0) return null;
                return (
                  <li key={it.assetId}>
                    <strong>{it.name}</strong>
                    <ul>
                      {links.map((l) => (
                        <li key={l.ammunitionAssetId}>
                          {l.ammunitionName}: {l.roundsConsumed ?? 0} rounds
                        </li>
                      ))}
                    </ul>
                  </li>
                );
              })}
            </ul>
          )}
        </>
      )}

      {cancelConfirm ? (
        <ConfirmModal
          title="Cancel this range day?"
          message="It will be marked cancelled. You can still see it in the list."
          confirmLabel="Cancel range day"
          onCancel={() => setCancelConfirm(false)}
          onConfirm={() => void runCancel()}
        />
      ) : null}
      {deleteConfirm ? (
        <ConfirmModal
          title="Delete this range day?"
          message="This removes the planned day permanently. Completed or cancelled days cannot be deleted here."
          confirmLabel="Delete"
          onCancel={() => setDeleteConfirm(false)}
          onConfirm={() => void runDelete()}
        />
      ) : null}
    </main>
  );
}
