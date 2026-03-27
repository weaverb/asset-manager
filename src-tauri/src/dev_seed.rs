//! Debug-only inventory seeding for local development (`npm run tauri:dev`).

use crate::db::{self, AssetInput};
use rusqlite::{params, Connection};
use std::path::Path;

/// Stored in `app_settings` so we only auto-seed once per dev database.
pub const DEV_SEED_FLAG_KEY: &str = "dev_inventory_seeded";

const TAG_DEV: &str = "dev";
const TAG_RANGE: &str = "range";

/// Seeds sample inventory once per dev database (see `lib.rs` setup).
#[cfg(debug_assertions)]
pub fn ensure_dev_seed(conn: &Connection, images_dir: &Path) -> Result<(), String> {
    if db::get_setting(conn, DEV_SEED_FLAG_KEY)?.as_deref() == Some("1") {
        return Ok(());
    }
    seed_inventory(conn, images_dir)?;
    db::set_setting(conn, DEV_SEED_FLAG_KEY, "1")?;
    Ok(())
}

/// Remove all assets (and image files), then insert a fresh dev sample set.
pub fn drop_and_reseed(conn: &Connection, images_dir: &Path) -> Result<(), String> {
    if !cfg!(debug_assertions) {
        return Err("drop_and_reseed is only available in debug builds.".into());
    }
    wipe_inventory(conn, images_dir)?;
    db::delete_setting(conn, DEV_SEED_FLAG_KEY)?;
    seed_inventory(conn, images_dir)?;
    db::set_setting(conn, DEV_SEED_FLAG_KEY, "1")?;
    Ok(())
}

fn wipe_inventory(conn: &Connection, images_dir: &Path) -> Result<(), String> {
    conn.execute("DELETE FROM range_days", [])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM assets", [])
        .map_err(|e| e.to_string())?;
    if images_dir.exists() {
        for entry in std::fs::read_dir(images_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let p = entry.path();
            if p.is_dir() {
                let _ = std::fs::remove_dir_all(&p);
            } else {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn asset_input(
    kind: &str,
    name: &str,
    manufacturer: &str,
    model: &str,
    serial: &str,
    caliber: Option<&str>,
    quantity: i64,
    purchase_date: Option<&str>,
    maint_r: Option<i64>,
    maint_d: Option<i64>,
    subtype: Option<&str>,
    notes: &str,
) -> AssetInput {
    AssetInput {
        kind: kind.to_string(),
        name: name.to_string(),
        manufacturer: Some(manufacturer.to_string()),
        model: Some(model.to_string()),
        serial_number: Some(serial.to_string()),
        caliber: caliber.map(|s| s.to_string()),
        quantity: Some(quantity),
        purchase_date: purchase_date.map(|s| s.to_string()),
        purchase_price: Some(0.0),
        notes: Some(notes.to_string()),
        extra_json: Some("{}".into()),
        maintenance_every_n_rounds: maint_r,
        maintenance_every_n_days: maint_d,
        subtype: subtype.map(|s| s.to_string()),
        tags: Some(vec![TAG_DEV.into(), TAG_RANGE.into()]),
    }
}

fn seed_completed_range_day(
    conn: &Connection,
    scheduled_date: &str,
    rounds: &[(String, i64)],
) -> Result<(), String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO range_days (id, scheduled_date, status, notes, completed_at, created_at, updated_at)
         VALUES (?1, ?2, 'completed', NULL, ?3, ?3, ?3)",
        params![id, scheduled_date, now],
    )
    .map_err(|e| e.to_string())?;
    for (asset_id, r) in rounds {
        conn.execute(
            "INSERT INTO range_day_items (range_day_id, asset_id, rounds_fired) VALUES (?1, ?2, ?3)",
            params![id, asset_id, r],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn seed_inventory(conn: &Connection, _images_dir: &Path) -> Result<(), String> {
    // Six firearms (distinct calibers for range-day ammo pairing); varied maintenance settings.
    let g1 = db::create_asset(
        conn,
        asset_input(
            "firearm",
            "Dev Pistol 9mm",
            "DevWorks Mfg",
            "Alpha-100",
            "DEV-G1-9MM",
            Some("9mm"),
            1,
            Some("2019-06-01"),
            Some(1000),
            Some(180),
            Some("pistol"),
            "Near round threshold; schedule from purchase date.",
        ),
    )?;
    let g2 = db::create_asset(
        conn,
        asset_input(
            "firearm",
            "Dev AR .223",
            "Acme Tactical",
            "Bravo Mk II",
            "DEV-G2-223",
            Some(".223 Rem"),
            1,
            Some("2021-03-15"),
            Some(500),
            None,
            Some("semi_auto"),
            "Round-interval only; higher lifetime use.",
        ),
    )?;
    let g3 = db::create_asset(
        conn,
        asset_input(
            "firearm",
            "Dev Bolt .308",
            "Northfield Supply",
            "Charlie Lite",
            "DEV-G3-308",
            Some(".308 Win"),
            1,
            Some("2020-01-01"),
            None,
            Some(14),
            Some("bolt_action"),
            "Short day interval from purchase → overdue dashboard sample.",
        ),
    )?;
    let g4 = db::create_asset(
        conn,
        asset_input(
            "firearm",
            "Dev Shotgun 12ga",
            "Horizon Outdoors",
            "Delta Pro",
            "DEV-G4-12",
            Some("12 ga"),
            1,
            Some("2022-08-20"),
            None,
            None,
            Some("shotgun"),
            "No maintenance intervals (control).",
        ),
    )?;
    let g5 = db::create_asset(
        conn,
        asset_input(
            "firearm",
            "Dev PCC 9mm",
            "DevWorks Mfg",
            "Charlie Lite",
            "DEV-G5-9MM2",
            Some("9mm"),
            1,
            None,
            Some(2000),
            Some(365),
            Some("other"),
            "Both intervals; second 9mm for caliber matching tests.",
        ),
    )?;
    let g6 = db::create_asset(
        conn,
        asset_input(
            "firearm",
            "Dev Carbine 5.56",
            "Acme Tactical",
            "Delta Pro",
            "DEV-G6-556",
            Some("5.56 NATO"),
            1,
            Some("2023-01-10"),
            None,
            Some(120),
            Some("semi_auto"),
            "Day-based reminder from purchase.",
        ),
    )?;

    // At least one ammo asset per firearm caliber (exact caliber strings).
    for (name, cal, qty, ammo_sub) in [
        ("Dev Ammo 9mm (box A)", "9mm", 220_i64, "pistol"),
        ("Dev Ammo 9mm (box B)", "9mm", 180, "pistol"),
        ("Dev Ammo .223", ".223 Rem", 400, "rifle"),
        ("Dev Ammo .308", ".308 Win", 120, "rifle"),
        ("Dev Ammo 12 ga", "12 ga", 75, "shotgun"),
        ("Dev Ammo 5.56", "5.56 NATO", 350, "rifle"),
    ] {
        db::create_asset(
            conn,
            asset_input(
                "ammunition",
                name,
                "Northfield Supply",
                "Bravo Mk II",
                "AMMO-DEV",
                Some(cal),
                qty,
                None,
                None,
                None,
                Some(ammo_sub),
                "Seeded ammunition for dev.",
            ),
        )?;
    }

    // Parts and accessories (two each).
    db::create_asset(
        conn,
        asset_input(
            "part",
            "Dev spare spring",
            "DevWorks Mfg",
            "Alpha-100",
            "PART-1",
            None,
            1,
            None,
            None,
            None,
            None,
            "Seeded part.",
        ),
    )?;
    db::create_asset(
        conn,
        asset_input(
            "part",
            "Dev firing pin",
            "Acme Tactical",
            "Bravo Mk II",
            "PART-2",
            None,
            1,
            None,
            None,
            None,
            None,
            "Seeded part.",
        ),
    )?;
    db::create_asset(
        conn,
        asset_input(
            "accessory",
            "Dev sling",
            "Horizon Outdoors",
            "Delta Pro",
            "ACC-1",
            None,
            1,
            None,
            None,
            None,
            Some("other"),
            "Seeded accessory.",
        ),
    )?;
    db::create_asset(
        conn,
        asset_input(
            "accessory",
            "Dev LPVO scope",
            "Northfield Supply",
            "Charlie Lite",
            "ACC-2",
            None,
            1,
            None,
            None,
            None,
            Some("scope"),
            "Seeded accessory.",
        ),
    )?;

    // Usage counters for dashboard “top firearms” and maintenance widget.
    conn.execute(
        "UPDATE assets SET lifetime_rounds_fired = 1850, rounds_fired_since_maintenance = 920 WHERE id = ?1",
        params![g1.id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE assets SET lifetime_rounds_fired = 2400, rounds_fired_since_maintenance = 120 WHERE id = ?1",
        params![g2.id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE assets SET lifetime_rounds_fired = 400, rounds_fired_since_maintenance = 0 WHERE id = ?1",
        params![g3.id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE assets SET lifetime_rounds_fired = 150, rounds_fired_since_maintenance = 10 WHERE id = ?1",
        params![g4.id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE assets SET lifetime_rounds_fired = 600, rounds_fired_since_maintenance = 400 WHERE id = ?1",
        params![g5.id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE assets SET lifetime_rounds_fired = 80, rounds_fired_since_maintenance = 5 WHERE id = ?1",
        params![g6.id],
    )
    .map_err(|e| e.to_string())?;

    // Completed range days (counts for “top firearms” widget).
    seed_completed_range_day(
        conn,
        "2024-06-10",
        &[(g1.id.clone(), 200), (g2.id.clone(), 150)],
    )?;
    seed_completed_range_day(conn, "2024-07-04", &[(g1.id.clone(), 100)])?;
    seed_completed_range_day(
        conn,
        "2024-08-01",
        &[(g2.id.clone(), 300), (g5.id.clone(), 50)],
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn seed_sets_flag_and_creates_rows() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("db.sqlite");
        let img = dir.path().join("images");
        db::init(&db_path, &img).unwrap();
        let conn = db::open(&db_path).unwrap();
        seed_inventory(&conn, &img).unwrap();
        db::set_setting(&conn, DEV_SEED_FLAG_KEY, "1").unwrap();
        let all = db::list_assets(&conn, None, &[]).unwrap();
        assert_eq!(all.len(), 16);
        let kinds: std::collections::HashSet<_> = all.iter().map(|a| a.kind.as_str()).collect();
        for k in ["firearm", "ammunition", "part", "accessory"] {
            assert!(kinds.contains(k));
        }
        let guns: Vec<_> = all.iter().filter(|a| a.kind == "firearm").collect();
        assert_eq!(guns.len(), 6);
        let ammo: Vec<_> = all.iter().filter(|a| a.kind == "ammunition").collect();
        assert_eq!(ammo.len(), 6);
    }

    #[test]
    fn wipe_clears_assets() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("db.sqlite");
        let img = dir.path().join("images");
        db::init(&db_path, &img).unwrap();
        let conn = db::open(&db_path).unwrap();
        seed_inventory(&conn, &img).unwrap();
        wipe_inventory(&conn, &img).unwrap();
        assert_eq!(db::list_assets(&conn, None, &[]).unwrap().len(), 0);
    }

    #[test]
    #[cfg(debug_assertions)]
    fn ensure_dev_seed_inserts_only_once() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("db.sqlite");
        let img = dir.path().join("images");
        db::init(&db_path, &img).unwrap();
        let conn = db::open(&db_path).unwrap();
        ensure_dev_seed(&conn, &img).unwrap();
        let n1 = db::list_assets(&conn, None, &[]).unwrap().len();
        ensure_dev_seed(&conn, &img).unwrap();
        let n2 = db::list_assets(&conn, None, &[]).unwrap().len();
        assert_eq!(n1, 16);
        assert_eq!(n2, 16);
    }
}
