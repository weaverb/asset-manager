//! Debug-only inventory seeding for local development (`tauri dev`).

use crate::db::{self, AssetInput};
use rusqlite::Connection;
use std::path::Path;

/// Stored in `app_settings` so we only auto-seed once per dev database.
pub const DEV_SEED_FLAG_KEY: &str = "dev_inventory_seeded";

const KINDS: &[&str] = &["firearm", "part", "accessory", "ammunition"];

const MANUFACTURERS: &[&str] = &[
    "DevWorks Mfg",
    "Northfield Supply",
    "Acme Tactical",
    "Horizon Outdoors",
];

const MODELS: &[&str] = &["Alpha-100", "Bravo Mk II", "Charlie Lite", "Delta Pro"];

const CALIBERS: &[&str] = &["9mm", ".223 Rem", ".308 Win", "12 ga", "5.56 NATO"];

const TAG_POOL: &[&str] = &["dev", "range", "hunting", "competition", "home"];

/// If this is a debug build and the DB has not been marked seeded, insert sample rows.
pub fn ensure_dev_seed(conn: &Connection, images_dir: &Path) -> Result<(), String> {
    if !cfg!(debug_assertions) {
        return Ok(());
    }
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

fn seed_inventory(conn: &Connection, _images_dir: &Path) -> Result<(), String> {
    let mut n = 0usize;
    for &kind in KINDS {
        for slot in 0..2 {
            let input = sample_asset_input(kind, n, slot);
            db::create_asset(conn, input)?;
            n += 1;
        }
    }
    Ok(())
}

fn sample_asset_input(kind: &str, global_idx: usize, slot: usize) -> AssetInput {
    let mfg = MANUFACTURERS[global_idx % MANUFACTURERS.len()].to_string();
    let model = MODELS[(global_idx + slot) % MODELS.len()].to_string();
    let caliber = CALIBERS[global_idx % CALIBERS.len()].to_string();
    let quantity = if kind == "ammunition" {
        50 + (global_idx as i64 * 17) % 450
    } else {
        1
    };
    let title = match kind {
        "firearm" => "Firearm",
        "part" => "Part",
        "accessory" => "Accessory",
        "ammunition" => "Ammunition",
        _ => "Asset",
    };
    let name = format!("Dev {} {}-{}", title, global_idx + 1, slot + 1);
    let t1 = TAG_POOL[global_idx % TAG_POOL.len()].to_string();
    let t2 = TAG_POOL[(global_idx + slot + 1) % TAG_POOL.len()].to_string();
    let tag_pair = if t1.eq_ignore_ascii_case(&t2) {
        vec![t1]
    } else {
        vec![t1, t2]
    };
    AssetInput {
        kind: kind.to_string(),
        name,
        manufacturer: Some(mfg),
        model: Some(model),
        serial_number: Some(format!("DEV-{:04}-{}", global_idx, slot)),
        caliber: Some(caliber),
        quantity: Some(quantity),
        purchase_date: None,
        purchase_price: Some(99.0 + (global_idx as f64) * 12.5),
        notes: Some("Seeded for local development.".into()),
        extra_json: Some("{}".into()),
        tags: Some(tag_pair),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn sample_asset_input_ammunition_quantity() {
        let a = sample_asset_input("ammunition", 3, 0);
        assert!(a.quantity.unwrap_or(0) > 1);
        assert_eq!(a.kind, "ammunition");
    }

    #[test]
    fn sample_asset_input_firearm_quantity_one() {
        let a = sample_asset_input("firearm", 0, 0);
        assert_eq!(a.quantity, Some(1));
    }

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
        assert_eq!(all.len(), 8);
        let kinds: std::collections::HashSet<_> = all.iter().map(|a| a.kind.as_str()).collect();
        for k in KINDS {
            assert!(kinds.contains(*k));
        }
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
    fn ensure_dev_seed_inserts_only_once() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("db.sqlite");
        let img = dir.path().join("images");
        db::init(&db_path, &img).unwrap();
        let conn = db::open(&db_path).unwrap();
        if !cfg!(debug_assertions) {
            return;
        }
        ensure_dev_seed(&conn, &img).unwrap();
        let n1 = db::list_assets(&conn, None, &[]).unwrap().len();
        ensure_dev_seed(&conn, &img).unwrap();
        let n2 = db::list_assets(&conn, None, &[]).unwrap().len();
        assert_eq!(n1, 8);
        assert_eq!(n2, 8);
    }
}
