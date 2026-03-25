use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

const MAX_TAG_LEN: usize = 64;

pub fn init(db_path: &Path, images_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(images_dir).map_err(|e| e.to_string())?;
    let conn = open(db_path)?;
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS assets (
          id TEXT PRIMARY KEY,
          kind TEXT NOT NULL CHECK(kind IN ('firearm','part','accessory','ammunition')),
          name TEXT NOT NULL,
          manufacturer TEXT,
          model TEXT,
          serial_number TEXT,
          caliber TEXT,
          quantity INTEGER NOT NULL DEFAULT 1,
          purchase_date TEXT,
          purchase_price REAL,
          notes TEXT,
          extra_json TEXT NOT NULL DEFAULT '{}',
          lifetime_rounds_fired INTEGER NOT NULL DEFAULT 0,
          rounds_fired_since_maintenance INTEGER NOT NULL DEFAULT 0,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_assets_kind ON assets(kind);

        CREATE VIRTUAL TABLE IF NOT EXISTS assets_fts USING fts5(
          name,
          manufacturer,
          model,
          serial_number,
          caliber,
          notes,
          content='assets',
          content_rowid='rowid'
        );

        CREATE TRIGGER IF NOT EXISTS assets_ai AFTER INSERT ON assets BEGIN
          INSERT INTO assets_fts(rowid, name, manufacturer, model, serial_number, caliber, notes)
          VALUES (new.rowid, new.name, new.manufacturer, new.model, new.serial_number, new.caliber, new.notes);
        END;

        CREATE TRIGGER IF NOT EXISTS assets_au AFTER UPDATE ON assets BEGIN
          DELETE FROM assets_fts WHERE rowid = old.rowid;
          INSERT INTO assets_fts(rowid, name, manufacturer, model, serial_number, caliber, notes)
          VALUES (new.rowid, new.name, new.manufacturer, new.model, new.serial_number, new.caliber, new.notes);
        END;

        CREATE TRIGGER IF NOT EXISTS assets_ad AFTER DELETE ON assets BEGIN
          DELETE FROM assets_fts WHERE rowid = old.rowid;
        END;

        CREATE TABLE IF NOT EXISTS asset_images (
          id TEXT PRIMARY KEY,
          asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
          file_path TEXT NOT NULL,
          caption TEXT,
          sort_order INTEGER NOT NULL DEFAULT 0,
          created_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_asset_images_asset ON asset_images(asset_id);

        CREATE TABLE IF NOT EXISTS tags (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL COLLATE NOCASE UNIQUE
        );

        CREATE TABLE IF NOT EXISTS asset_tags (
          asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
          tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
          PRIMARY KEY (asset_id, tag_id)
        );

        CREATE INDEX IF NOT EXISTS idx_asset_tags_tag ON asset_tags(tag_id);

        CREATE TABLE IF NOT EXISTS app_settings (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS range_days (
          id TEXT PRIMARY KEY,
          scheduled_date TEXT NOT NULL,
          status TEXT NOT NULL CHECK(status IN ('planned','completed','cancelled')),
          notes TEXT,
          completed_at TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_range_days_scheduled ON range_days(scheduled_date);

        CREATE TABLE IF NOT EXISTS range_day_items (
          range_day_id TEXT NOT NULL REFERENCES range_days(id) ON DELETE CASCADE,
          asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
          rounds_fired INTEGER,
          PRIMARY KEY (range_day_id, asset_id)
        );

        CREATE INDEX IF NOT EXISTS idx_range_day_items_asset ON range_day_items(asset_id);

        CREATE TABLE IF NOT EXISTS range_day_firearm_ammo (
          range_day_id TEXT NOT NULL REFERENCES range_days(id) ON DELETE CASCADE,
          firearm_asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
          ammunition_asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
          rounds_consumed INTEGER,
          PRIMARY KEY (range_day_id, firearm_asset_id, ammunition_asset_id)
        );

        CREATE INDEX IF NOT EXISTS idx_rdfa_range ON range_day_firearm_ammo(range_day_id);
        CREATE INDEX IF NOT EXISTS idx_rdfa_ammo ON range_day_firearm_ammo(ammunition_asset_id);

        CREATE TABLE IF NOT EXISTS asset_maintenance (
          id TEXT PRIMARY KEY,
          asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
          performed_at TEXT NOT NULL,
          notes TEXT,
          created_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_asset_maintenance_asset ON asset_maintenance(asset_id);
        "#,
    )
    .map_err(|e| e.to_string())?;

    ensure_assets_round_columns(&conn)?;
    ensure_assets_maintenance_interval_columns(&conn)?;
    ensure_assets_subtype_column(&conn)?;
    migrate_firearm_rifle_to_bolt_action(&conn)?;
    migrate_ammunition_subtype_default(&conn)?;

    // External-content FTS5 can drift from `assets` (e.g. legacy bugs, interrupted writes).
    // Rebuilding from the content table keeps MATCH results aligned with visible row text.
    let n_assets: i64 = conn
        .query_row("SELECT COUNT(*) FROM assets", [], |r| r.get(0))
        .unwrap_or(0);
    if n_assets > 0 {
        conn.execute("INSERT INTO assets_fts(assets_fts) VALUES('rebuild')", [])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn ensure_assets_round_columns(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(assets)")
        .map_err(|e| e.to_string())?;
    let cols: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    if !cols.iter().any(|c| c == "lifetime_rounds_fired") {
        conn.execute(
            "ALTER TABLE assets ADD COLUMN lifetime_rounds_fired INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| e.to_string())?;
    }
    if !cols.iter().any(|c| c == "rounds_fired_since_maintenance") {
        conn.execute(
            "ALTER TABLE assets ADD COLUMN rounds_fired_since_maintenance INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn ensure_assets_maintenance_interval_columns(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(assets)")
        .map_err(|e| e.to_string())?;
    let cols: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    if !cols.iter().any(|c| c == "maintenance_every_n_rounds") {
        conn.execute(
            "ALTER TABLE assets ADD COLUMN maintenance_every_n_rounds INTEGER",
            [],
        )
        .map_err(|e| e.to_string())?;
    }
    if !cols.iter().any(|c| c == "maintenance_every_n_days") {
        conn.execute(
            "ALTER TABLE assets ADD COLUMN maintenance_every_n_days INTEGER",
            [],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Legacy subtype `rifle` split into `semi_auto` vs `bolt_action`; migrate old rows to bolt.
fn migrate_firearm_rifle_to_bolt_action(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "UPDATE assets SET subtype = 'bolt_action' WHERE kind = 'firearm' AND LOWER(TRIM(COALESCE(subtype, ''))) = 'rifle'",
        [],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Legacy ammunition rows had no subtype; default to `rifle` (prior icon behavior).
fn migrate_ammunition_subtype_default(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "UPDATE assets SET subtype = 'rifle' WHERE kind = 'ammunition' AND (subtype IS NULL OR TRIM(subtype) = '')",
        [],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn ensure_assets_subtype_column(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(assets)")
        .map_err(|e| e.to_string())?;
    let cols: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    if !cols.iter().any(|c| c == "subtype") {
        conn.execute("ALTER TABLE assets ADD COLUMN subtype TEXT", [])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn open(db_path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| e.to_string())?;
    Ok(conn)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub caliber: Option<String>,
    pub quantity: i64,
    pub purchase_date: Option<String>,
    pub purchase_price: Option<f64>,
    pub notes: Option<String>,
    pub extra_json: String,
    #[serde(default)]
    pub lifetime_rounds_fired: i64,
    #[serde(default)]
    pub rounds_fired_since_maintenance: i64,
    #[serde(default)]
    pub maintenance_every_n_rounds: Option<i64>,
    #[serde(default)]
    pub maintenance_every_n_days: Option<i64>,
    #[serde(default)]
    pub subtype: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetInput {
    pub kind: String,
    pub name: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub caliber: Option<String>,
    pub quantity: Option<i64>,
    pub purchase_date: Option<String>,
    pub purchase_price: Option<f64>,
    pub notes: Option<String>,
    pub extra_json: Option<String>,
    #[serde(default)]
    pub maintenance_every_n_rounds: Option<i64>,
    #[serde(default)]
    pub maintenance_every_n_days: Option<i64>,
    #[serde(default)]
    pub subtype: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssetImage {
    pub id: String,
    pub asset_id: String,
    pub file_path: String,
    pub caption: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
}

fn row_to_asset(row: &rusqlite::Row<'_>) -> rusqlite::Result<Asset> {
    Ok(Asset {
        id: row.get(0)?,
        kind: row.get(1)?,
        name: row.get(2)?,
        manufacturer: row.get(3)?,
        model: row.get(4)?,
        serial_number: row.get(5)?,
        caliber: row.get(6)?,
        quantity: row.get(7)?,
        purchase_date: row.get(8)?,
        purchase_price: row.get(9)?,
        notes: row.get(10)?,
        extra_json: row.get(11)?,
        lifetime_rounds_fired: row.get(12)?,
        rounds_fired_since_maintenance: row.get(13)?,
        maintenance_every_n_rounds: row.get(14)?,
        maintenance_every_n_days: row.get(15)?,
        subtype: row.get(16)?,
        created_at: row.get(17)?,
        updated_at: row.get(18)?,
        tags: vec![],
    })
}

fn tag_filter_sql(names: &[String]) -> (bool, String) {
    if names.is_empty() {
        (false, String::new())
    } else {
        let ph = names.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        (
            true,
            format!(
                " AND a.id IN (SELECT DISTINCT at.asset_id FROM asset_tags at INNER JOIN tags t ON t.id = at.tag_id WHERE t.name IN ({ph}))"
            ),
        )
    }
}

fn list_assets_sql_values(
    kind: Option<&str>,
    tag_names: &[String],
) -> (String, Vec<rusqlite::types::Value>) {
    let tags_clean: Vec<String> = tag_names
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let (filter_tags, tag_clause) = tag_filter_sql(&tags_clean);

    let mut sql = String::from(
        "SELECT a.id, a.kind, a.name, a.manufacturer, a.model, a.serial_number, a.caliber, a.quantity, a.purchase_date, a.purchase_price, a.notes, a.extra_json, a.lifetime_rounds_fired, a.rounds_fired_since_maintenance, a.maintenance_every_n_rounds, a.maintenance_every_n_days, a.subtype, a.created_at, a.updated_at FROM assets a WHERE 1=1",
    );
    if filter_tags {
        sql.push_str(&tag_clause);
    }
    if kind.is_some() {
        sql.push_str(" AND a.kind = ?");
    }
    sql.push_str(" ORDER BY a.updated_at DESC");

    let mut ps: Vec<rusqlite::types::Value> = Vec::new();
    for t in tags_clean {
        ps.push(rusqlite::types::Value::Text(t));
    }
    if let Some(k) = kind {
        ps.push(rusqlite::types::Value::Text(k.to_string()));
    }
    (sql, ps)
}

pub fn hydrate_asset_tags(conn: &Connection, assets: &mut [Asset]) -> Result<(), String> {
    if assets.is_empty() {
        return Ok(());
    }
    let ids: Vec<String> = assets.iter().map(|a| a.id.clone()).collect();
    let ph = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT at.asset_id, t.name FROM asset_tags at
         INNER JOIN tags t ON t.id = at.tag_id
         WHERE at.asset_id IN ({ph})
         ORDER BY t.name COLLATE NOCASE"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params_from_iter(ids.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for r in rows {
        let (aid, name) = r.map_err(|e| e.to_string())?;
        map.entry(aid).or_default().push(name);
    }
    for a in assets {
        a.tags = map.remove(&a.id).unwrap_or_default();
    }
    Ok(())
}

fn normalize_tag_names(raw: &[String]) -> Vec<String> {
    let mut seen = HashSet::<String>::new();
    let mut out = Vec::new();
    for t in raw {
        let mut s = t.trim().to_string();
        if s.is_empty() {
            continue;
        }
        let n_chars = s.chars().count();
        if n_chars > MAX_TAG_LEN {
            s = s.chars().take(MAX_TAG_LEN).collect();
        }
        let key = s.to_lowercase();
        if seen.insert(key) {
            out.push(s);
        }
    }
    out
}

fn ensure_tag_id(conn: &Connection, name: &str) -> Result<String, String> {
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM tags WHERE name = ?1 COLLATE NOCASE",
            params![name],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some(id) = existing {
        return Ok(id);
    }
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO tags (id, name) VALUES (?1, ?2)",
        params![id, name],
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

pub fn replace_asset_tags(
    conn: &Connection,
    asset_id: &str,
    tags: &[String],
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM asset_tags WHERE asset_id = ?1",
        params![asset_id],
    )
    .map_err(|e| e.to_string())?;
    let normalized = normalize_tag_names(tags);
    for name in normalized {
        let tid = ensure_tag_id(conn, &name)?;
        conn.execute(
            "INSERT OR IGNORE INTO asset_tags (asset_id, tag_id) VALUES (?1, ?2)",
            params![asset_id, tid],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn list_assets(
    conn: &Connection,
    kind: Option<&str>,
    tag_names: &[String],
) -> Result<Vec<Asset>, String> {
    let (sql, ps) = list_assets_sql_values(kind, tag_names);
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params_from_iter(ps), row_to_asset)
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    hydrate_asset_tags(conn, &mut out)?;
    Ok(out)
}

fn fts_match_query(user: &str) -> String {
    let parts: Vec<String> = user
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .map(|s| format!("\"{}\"*", s.replace('"', "")))
        .collect();
    if parts.is_empty() {
        String::new()
    } else {
        parts.join(" AND ")
    }
}

pub fn search_assets(
    conn: &Connection,
    query: &str,
    tag_names: &[String],
) -> Result<Vec<Asset>, String> {
    let q = query.trim();
    if q.is_empty() {
        return list_assets(conn, None, tag_names);
    }
    let fts = fts_match_query(q);
    if fts.is_empty() {
        return list_assets(conn, None, tag_names);
    }
    let tags_clean: Vec<String> = tag_names
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let (filter_tags, tag_clause) = tag_filter_sql(&tags_clean);

    let mut sql = String::from(
        "SELECT a.id, a.kind, a.name, a.manufacturer, a.model, a.serial_number, a.caliber, a.quantity, a.purchase_date, a.purchase_price, a.notes, a.extra_json, a.lifetime_rounds_fired, a.rounds_fired_since_maintenance, a.maintenance_every_n_rounds, a.maintenance_every_n_days, a.subtype, a.created_at, a.updated_at
         FROM assets a
         INNER JOIN assets_fts ON assets_fts.rowid = a.rowid
         WHERE assets_fts MATCH ?1",
    );
    if filter_tags {
        sql.push_str(&tag_clause);
    }
    sql.push_str(" ORDER BY a.updated_at DESC");

    let mut ps: Vec<rusqlite::types::Value> = vec![rusqlite::types::Value::Text(fts)];
    for t in tags_clean {
        ps.push(rusqlite::types::Value::Text(t));
    }

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params_from_iter(ps), row_to_asset)
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    hydrate_asset_tags(conn, &mut out)?;
    Ok(out)
}

pub fn get_asset(conn: &Connection, id: &str) -> Result<Option<Asset>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, kind, name, manufacturer, model, serial_number, caliber, quantity, purchase_date, purchase_price, notes, extra_json, lifetime_rounds_fired, rounds_fired_since_maintenance, maintenance_every_n_rounds, maintenance_every_n_days, subtype, created_at, updated_at
             FROM assets WHERE id = ?1",
        )
        .map_err(|e| e.to_string())?;
    let row = stmt
        .query_row(params![id], row_to_asset)
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some(a) = row {
        let mut one = vec![a];
        hydrate_asset_tags(conn, &mut one)?;
        return Ok(one.pop());
    }
    Ok(None)
}

pub fn create_asset(conn: &Connection, input: AssetInput) -> Result<Asset, String> {
    validate_kind(&input.kind)?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let quantity = input.quantity.unwrap_or(1).max(0);
    let (maint_r, maint_d) = normalized_maintenance_intervals(&input.kind, &input);
    let subtype = normalized_subtype(&input.kind, &input.subtype)?;
    let extra = input.extra_json.unwrap_or_else(|| "{}".to_string());
    conn.execute(
        r#"INSERT INTO assets (
            id, kind, name, manufacturer, model, serial_number, caliber, quantity,
            purchase_date, purchase_price, notes, extra_json, lifetime_rounds_fired,
            rounds_fired_since_maintenance, maintenance_every_n_rounds, maintenance_every_n_days,
            subtype, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0, 0, ?13, ?14, ?15, ?16, ?17)"#,
        params![
            id,
            input.kind,
            input.name,
            input.manufacturer,
            input.model,
            input.serial_number,
            input.caliber,
            quantity,
            input.purchase_date,
            input.purchase_price,
            input.notes,
            extra,
            maint_r,
            maint_d,
            subtype,
            now,
            now,
        ],
    )
    .map_err(|e| e.to_string())?;
    replace_asset_tags(conn, &id, input.tags.as_deref().unwrap_or(&[]))?;
    get_asset(conn, &id)?.ok_or_else(|| "Failed to load created asset".to_string())
}

pub fn update_asset(conn: &Connection, id: &str, input: AssetInput) -> Result<Asset, String> {
    validate_kind(&input.kind)?;
    get_asset(conn, id)?.ok_or_else(|| format!("Asset not found: {id}"))?;
    let now = chrono::Utc::now().to_rfc3339();
    let quantity = input.quantity.unwrap_or(1).max(0);
    let (maint_r, maint_d) = normalized_maintenance_intervals(&input.kind, &input);
    let subtype = normalized_subtype(&input.kind, &input.subtype)?;
    let extra = input.extra_json.unwrap_or_else(|| "{}".to_string());
    conn.execute(
        r#"UPDATE assets SET
            kind = ?2, name = ?3, manufacturer = ?4, model = ?5, serial_number = ?6,
            caliber = ?7, quantity = ?8, purchase_date = ?9, purchase_price = ?10,
            notes = ?11, extra_json = ?12, maintenance_every_n_rounds = ?13,
            maintenance_every_n_days = ?14, subtype = ?15, updated_at = ?16
        WHERE id = ?1"#,
        params![
            id,
            input.kind,
            input.name,
            input.manufacturer,
            input.model,
            input.serial_number,
            input.caliber,
            quantity,
            input.purchase_date,
            input.purchase_price,
            input.notes,
            extra,
            maint_r,
            maint_d,
            subtype,
            now,
        ],
    )
    .map_err(|e| e.to_string())?;
    if input.tags.is_some() {
        replace_asset_tags(conn, id, input.tags.as_deref().unwrap_or(&[]))?;
    }
    get_asset(conn, id)?.ok_or_else(|| "Failed to load updated asset".to_string())
}

pub fn delete_asset_files(images_dir: &Path, id: &str) {
    let _ = std::fs::remove_dir_all(images_dir.join(id));
}

pub fn delete_asset(conn: &Connection, images_dir: &Path, id: &str) -> Result<(), String> {
    delete_asset_files(images_dir, id);
    conn.execute("DELETE FROM assets WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn validate_kind(kind: &str) -> Result<(), String> {
    match kind {
        "firearm" | "part" | "accessory" | "ammunition" => Ok(()),
        _ => Err(format!("Invalid kind: {kind}")),
    }
}

/// Positive intervals only; cleared for non-firearm assets.
fn normalized_maintenance_intervals(kind: &str, input: &AssetInput) -> (Option<i64>, Option<i64>) {
    if kind != "firearm" {
        return (None, None);
    }
    let r = input.maintenance_every_n_rounds.filter(|&n| n > 0);
    let d = input.maintenance_every_n_days.filter(|&n| n > 0);
    (r, d)
}

fn normalized_subtype(kind: &str, raw: &Option<String>) -> Result<Option<String>, String> {
    match kind {
        "firearm" => {
            const ALLOW: &[&str] = &[
                "pistol",
                "semi_auto",
                "bolt_action",
                "revolver",
                "shotgun",
                "pcc_sub",
                "other",
            ];
            match raw {
                None => Ok(None),
                Some(s) => {
                    let mut t = s.trim().to_lowercase();
                    if t.is_empty() {
                        return Ok(None);
                    }
                    if t == "rifle" {
                        t = "bolt_action".to_string();
                    }
                    if ALLOW.contains(&t.as_str()) {
                        Ok(Some(t))
                    } else {
                        Err(format!("Invalid firearm subtype: {s}"))
                    }
                }
            }
        }
        "accessory" => {
            const ALLOW: &[&str] = &["scope", "reddot", "holographic", "light", "other"];
            match raw {
                None => Ok(None),
                Some(s) => {
                    let t = s.trim().to_lowercase();
                    if t.is_empty() {
                        return Ok(None);
                    }
                    if ALLOW.contains(&t.as_str()) {
                        Ok(Some(t))
                    } else {
                        Err(format!("Invalid accessory subtype: {s}"))
                    }
                }
            }
        }
        "ammunition" => {
            const ALLOW: &[&str] = &["pistol", "rifle", "shotgun", "other"];
            match raw {
                None => Ok(None),
                Some(s) => {
                    let t = s.trim().to_lowercase();
                    if t.is_empty() {
                        return Ok(None);
                    }
                    if ALLOW.contains(&t.as_str()) {
                        Ok(Some(t))
                    } else {
                        Err(format!("Invalid ammunition subtype: {s}"))
                    }
                }
            }
        }
        _ => Ok(None),
    }
}

pub fn list_images(conn: &Connection, asset_id: &str) -> Result<Vec<AssetImage>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, asset_id, file_path, caption, sort_order, created_at FROM asset_images WHERE asset_id = ?1 ORDER BY sort_order ASC, created_at ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![asset_id], |row| {
            Ok(AssetImage {
                id: row.get(0)?,
                asset_id: row.get(1)?,
                file_path: row.get(2)?,
                caption: row.get(3)?,
                sort_order: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub fn add_image(
    conn: &Connection,
    images_dir: &Path,
    asset_id: &str,
    original_name: &str,
    data: &[u8],
    caption: Option<String>,
) -> Result<AssetImage, String> {
    get_asset(conn, asset_id)?.ok_or_else(|| format!("Asset not found: {asset_id}"))?;
    let id = uuid::Uuid::new_v4().to_string();
    let original_path = PathBuf::from(original_name);
    let ext = original_path
        .extension()
        .and_then(|s| s.to_str())
        .filter(|s| s.len() <= 10 && s.chars().all(|c| c.is_alphanumeric()))
        .unwrap_or("bin");
    let dir = images_dir.join(asset_id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{id}.{ext}"));
    std::fs::write(&path, data).map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    let sort_order: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM asset_images WHERE asset_id = ?1",
            params![asset_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let path_str = path.to_string_lossy().to_string();
    conn.execute(
        "INSERT INTO asset_images (id, asset_id, file_path, caption, sort_order, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, asset_id, path_str, caption, sort_order, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(AssetImage {
        id,
        asset_id: asset_id.to_string(),
        file_path: path_str,
        caption,
        sort_order,
        created_at: now,
    })
}

pub fn delete_image(conn: &Connection, images_dir: &Path, image_id: &str) -> Result<(), String> {
    let path: Option<String> = conn
        .query_row(
            "SELECT file_path FROM asset_images WHERE id = ?1",
            params![image_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some(p) = path {
        let _ = std::fs::remove_file(&p);
        if let Some(parent) = Path::new(&p).parent() {
            if parent != images_dir {
                if let Ok(dir) = std::fs::read_dir(parent) {
                    if dir.count() == 0 {
                        let _ = std::fs::remove_dir(parent);
                    }
                }
            }
        }
    }
    conn.execute("DELETE FROM asset_images WHERE id = ?1", params![image_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn read_image_file(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| e.to_string())
}

pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    let mut stmt = conn
        .prepare("SELECT value FROM app_settings WHERE key = ?1")
        .map_err(|e| e.to_string())?;
    stmt.query_row(params![key], |r| r.get(0))
        .optional()
        .map_err(|e| e.to_string())
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_setting(conn: &Connection, key: &str) -> Result<(), String> {
    conn.execute("DELETE FROM app_settings WHERE key = ?1", params![key])
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn like_contains(s: &str) -> String {
    let cleaned: String = s.chars().filter(|c| *c != '%' && *c != '_').collect();
    format!("%{cleaned}%")
}

fn rows_to_strings(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<String>>,
) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

/// Distinct non-empty manufacturer strings from the user's assets (prefix optional).
pub fn distinct_manufacturers(
    conn: &Connection,
    prefix: &str,
    limit: i64,
) -> Result<Vec<String>, String> {
    let prefix = prefix.trim();
    if prefix.is_empty() {
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT manufacturer FROM assets
                 WHERE manufacturer IS NOT NULL AND TRIM(manufacturer) != ''
                 ORDER BY manufacturer COLLATE NOCASE ASC
                 LIMIT ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![limit], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        return rows_to_strings(rows);
    }
    let like = like_contains(prefix);
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT manufacturer FROM assets
             WHERE manufacturer IS NOT NULL AND TRIM(manufacturer) != ''
             AND LOWER(manufacturer) LIKE LOWER(?2)
             ORDER BY manufacturer COLLATE NOCASE ASC
             LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![limit, like], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    rows_to_strings(rows)
}

/// Distinct model strings; optionally scoped to a manufacturer value already on assets.
pub fn distinct_models(
    conn: &Connection,
    manufacturer: Option<&str>,
    prefix: &str,
    limit: i64,
) -> Result<Vec<String>, String> {
    let prefix = prefix.trim();
    let m = manufacturer.map(str::trim).filter(|s| !s.is_empty());

    match (m, prefix.is_empty()) {
        (None, true) => {
            let mut stmt = conn
                .prepare(
                    "SELECT DISTINCT model FROM assets
                     WHERE model IS NOT NULL AND TRIM(model) != ''
                     ORDER BY model COLLATE NOCASE ASC
                     LIMIT ?1",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(params![limit], |r| r.get(0))
                .map_err(|e| e.to_string())?;
            rows_to_strings(rows)
        }
        (None, false) => {
            let like = like_contains(prefix);
            let mut stmt = conn
                .prepare(
                    "SELECT DISTINCT model FROM assets
                     WHERE model IS NOT NULL AND TRIM(model) != ''
                     AND LOWER(model) LIKE LOWER(?2)
                     ORDER BY model COLLATE NOCASE ASC
                     LIMIT ?1",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(params![limit, like], |r| r.get(0))
                .map_err(|e| e.to_string())?;
            rows_to_strings(rows)
        }
        (Some(man), true) => {
            let mut stmt = conn
                .prepare(
                    "SELECT DISTINCT model FROM assets
                     WHERE model IS NOT NULL AND TRIM(model) != ''
                     AND LOWER(TRIM(manufacturer)) = LOWER(?2)
                     ORDER BY model COLLATE NOCASE ASC
                     LIMIT ?1",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(params![limit, man], |r| r.get(0))
                .map_err(|e| e.to_string())?;
            let out = rows_to_strings(rows)?;
            if out.is_empty() {
                return distinct_models(conn, None, prefix, limit);
            }
            Ok(out)
        }
        (Some(man), false) => {
            let like = like_contains(prefix);
            let mut stmt = conn
                .prepare(
                    "SELECT DISTINCT model FROM assets
                     WHERE model IS NOT NULL AND TRIM(model) != ''
                     AND LOWER(TRIM(manufacturer)) = LOWER(?2)
                     AND LOWER(model) LIKE LOWER(?3)
                     ORDER BY model COLLATE NOCASE ASC
                     LIMIT ?1",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(params![limit, man, like], |r| r.get(0))
                .map_err(|e| e.to_string())?;
            let out = rows_to_strings(rows)?;
            if out.is_empty() {
                return distinct_models(conn, None, prefix, limit);
            }
            Ok(out)
        }
    }
}

/// Tag names for autocomplete (prefix optional), case-insensitive contains when prefix non-empty.
pub fn suggest_tag_names(
    conn: &Connection,
    prefix: &str,
    limit: i64,
) -> Result<Vec<String>, String> {
    let prefix = prefix.trim();
    if prefix.is_empty() {
        let mut stmt = conn
            .prepare("SELECT name FROM tags ORDER BY name COLLATE NOCASE ASC LIMIT ?1")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![limit], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        return rows_to_strings(rows);
    }
    let like = like_contains(prefix);
    let mut stmt = conn
        .prepare(
            "SELECT name FROM tags WHERE LOWER(name) LIKE LOWER(?2) ORDER BY name COLLATE NOCASE ASC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![limit, like], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    rows_to_strings(rows)
}

// --- Range days & maintenance ---

fn validate_iso_date(s: &str) -> Result<(), String> {
    if chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_err() {
        return Err("Invalid scheduled date; use YYYY-MM-DD.".into());
    }
    Ok(())
}

fn dedupe_asset_ids(asset_ids: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::<String>::new();
    let mut out = Vec::new();
    for id in asset_ids {
        let t = id.trim().to_string();
        if t.is_empty() {
            continue;
        }
        if seen.insert(t.clone()) {
            out.push(t);
        }
    }
    out
}

fn validate_firearm_assets(conn: &Connection, asset_ids: &[String]) -> Result<(), String> {
    for id in asset_ids {
        let row: Option<(String,)> = conn
            .query_row("SELECT kind FROM assets WHERE id = ?1", params![id], |r| {
                Ok((r.get::<_, String>(0)?,))
            })
            .optional()
            .map_err(|e| e.to_string())?;
        match row {
            None => return Err(format!("Asset not found: {id}")),
            Some((k,)) if k == "firearm" => {}
            Some(_) => return Err("Only firearms can be added to a range day.".into()),
        }
    }
    Ok(())
}

fn ensure_planned_no_same_date_conflict(
    conn: &Connection,
    exclude_range_day_id: Option<&str>,
    scheduled_date: &str,
    asset_ids: &[String],
) -> Result<(), String> {
    for asset_id in asset_ids {
        let conflict = match exclude_range_day_id {
            None => conn
                .query_row(
                    "SELECT 1 FROM range_day_items rdi
                     INNER JOIN range_days rd ON rd.id = rdi.range_day_id
                     WHERE rdi.asset_id = ?1 AND rd.status = 'planned' AND rd.scheduled_date = ?2
                     LIMIT 1",
                    params![asset_id, scheduled_date],
                    |_| Ok(()),
                )
                .optional()
                .map_err(|e| e.to_string())?
                .is_some(),
            Some(ex) => conn
                .query_row(
                    "SELECT 1 FROM range_day_items rdi
                     INNER JOIN range_days rd ON rd.id = rdi.range_day_id
                     WHERE rdi.asset_id = ?1 AND rd.status = 'planned' AND rd.scheduled_date = ?2
                       AND rd.id != ?3
                     LIMIT 1",
                    params![asset_id, scheduled_date, ex],
                    |_| Ok(()),
                )
                .optional()
                .map_err(|e| e.to_string())?
                .is_some(),
        };
        if conflict {
            return Err(
                "This firearm is already on another planned range day for that date.".into(),
            );
        }
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RangeDaySummary {
    pub id: String,
    pub scheduled_date: String,
    pub status: String,
    pub notes: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub item_count: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RangeDayItemDetail {
    pub asset_id: String,
    pub name: String,
    pub kind: String,
    pub rounds_fired: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RangeDayAmmoLink {
    pub firearm_asset_id: String,
    pub firearm_name: String,
    pub ammunition_asset_id: String,
    pub ammunition_name: String,
    pub ammunition_caliber: Option<String>,
    pub quantity_on_hand: i64,
    pub rounds_consumed: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RangeDayDetail {
    pub id: String,
    pub scheduled_date: String,
    pub status: String,
    pub notes: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub items: Vec<RangeDayItemDetail>,
    #[serde(default)]
    pub ammo_links: Vec<RangeDayAmmoLink>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RangeDayRoundEntry {
    pub asset_id: String,
    pub rounds_fired: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RangeDayAmmoConsumptionEntry {
    pub firearm_asset_id: String,
    pub ammunition_asset_id: String,
    pub rounds: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssetMaintenance {
    pub id: String,
    pub asset_id: String,
    pub performed_at: String,
    pub notes: Option<String>,
    pub created_at: String,
}

fn row_to_range_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<RangeDaySummary> {
    Ok(RangeDaySummary {
        id: row.get(0)?,
        scheduled_date: row.get(1)?,
        status: row.get(2)?,
        notes: row.get(3)?,
        completed_at: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
        item_count: row.get(7)?,
    })
}

fn normalize_caliber_key(cal: &Option<String>) -> Option<String> {
    cal.as_ref()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
}

fn asset_kind_and_caliber(conn: &Connection, id: &str) -> Result<(String, Option<String>), String> {
    conn.query_row(
        "SELECT kind, caliber FROM assets WHERE id = ?1",
        params![id],
        |row| Ok((row.get::<_, String>(0)?, row.get(1)?)),
    )
    .map_err(|_| format!("Asset not found: {id}"))
}

fn firearm_on_range_day(
    conn: &Connection,
    range_day_id: &str,
    firearm_id: &str,
) -> Result<bool, String> {
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM range_day_items WHERE range_day_id = ?1 AND asset_id = ?2",
            params![range_day_id, firearm_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(n > 0)
}

fn validate_ammo_set_for_firearm(
    conn: &Connection,
    range_day_id: &str,
    firearm_id: &str,
    ammunition_ids: &[String],
) -> Result<(), String> {
    if ammunition_ids.is_empty() {
        return Ok(());
    }
    if !firearm_on_range_day(conn, range_day_id, firearm_id)? {
        return Err("Firearm is not on this range day.".into());
    }
    let (fk, fcal_opt) = asset_kind_and_caliber(conn, firearm_id)?;
    if fk != "firearm" {
        return Err("Target asset is not a firearm.".into());
    }
    let f_key = normalize_caliber_key(&fcal_opt);
    let mut unified_cal: Option<String> = None;
    for aid in ammunition_ids {
        let (k, cal_opt) = asset_kind_and_caliber(conn, aid)?;
        if k != "ammunition" {
            return Err("Only ammunition assets can be assigned to a firearm.".into());
        }
        let cal_key = normalize_caliber_key(&cal_opt)
            .ok_or_else(|| "Each assigned ammunition asset must have a caliber set.".to_string())?;
        if let Some(ref u) = unified_cal {
            if *u != cal_key {
                return Err(
                    "All ammunition for one firearm on a range day must be the same caliber."
                        .into(),
                );
            }
        } else {
            unified_cal = Some(cal_key.clone());
        }
        if let Some(ref fk_non) = f_key {
            if *fk_non != cal_key {
                return Err("Ammunition caliber does not match this firearm's caliber.".into());
            }
        }
        let other: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM range_day_firearm_ammo WHERE range_day_id = ?1 AND ammunition_asset_id = ?2 AND firearm_asset_id != ?3",
                params![range_day_id, aid, firearm_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if other > 0 {
            return Err(
                "That ammunition is already assigned to another firearm on this range day.".into(),
            );
        }
    }
    Ok(())
}

fn delete_rdfa_firearms_not_in(
    tx: &rusqlite::Transaction<'_>,
    range_day_id: &str,
    keep_firearm_ids: &[String],
) -> Result<(), String> {
    if keep_firearm_ids.is_empty() {
        return Ok(());
    }
    let ph = std::iter::repeat_n("?", keep_firearm_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "DELETE FROM range_day_firearm_ammo WHERE range_day_id = ?1 AND firearm_asset_id NOT IN ({ph})"
    );
    use rusqlite::types::Value;
    let mut vals: Vec<Value> = vec![Value::Text(range_day_id.to_string())];
    for id in keep_firearm_ids {
        vals.push(Value::Text(id.clone()));
    }
    tx.execute(&sql, rusqlite::params_from_iter(vals))
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn set_range_day_firearm_ammunition(
    conn: &Connection,
    range_day_id: &str,
    firearm_asset_id: &str,
    ammunition_asset_ids: Vec<String>,
) -> Result<RangeDayDetail, String> {
    let status: String = conn
        .query_row(
            "SELECT status FROM range_days WHERE id = ?1",
            params![range_day_id],
            |r| r.get(0),
        )
        .map_err(|_| "Range day not found.".to_string())?;
    if status != "planned" {
        return Err("Only planned range days can be edited.".into());
    }
    let unique = dedupe_asset_ids(ammunition_asset_ids);
    validate_ammo_set_for_firearm(conn, range_day_id, firearm_asset_id, &unique)?;
    let now = chrono::Utc::now().to_rfc3339();
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "DELETE FROM range_day_firearm_ammo WHERE range_day_id = ?1 AND firearm_asset_id = ?2",
        params![range_day_id, firearm_asset_id],
    )
    .map_err(|e| e.to_string())?;
    for aid in &unique {
        tx.execute(
            "INSERT INTO range_day_firearm_ammo (range_day_id, firearm_asset_id, ammunition_asset_id, rounds_consumed) VALUES (?1, ?2, ?3, NULL)",
            params![range_day_id, firearm_asset_id, aid],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.execute(
        "UPDATE range_days SET updated_at = ?2 WHERE id = ?1",
        params![range_day_id, now],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    get_range_day(conn, range_day_id)?.ok_or_else(|| "Failed to load range day.".to_string())
}

pub fn list_range_days(conn: &Connection) -> Result<Vec<RangeDaySummary>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT rd.id, rd.scheduled_date, rd.status, rd.notes, rd.completed_at, rd.created_at, rd.updated_at,
                    COALESCE((SELECT COUNT(*) FROM range_day_items i WHERE i.range_day_id = rd.id), 0)
             FROM range_days rd
             ORDER BY rd.scheduled_date DESC, rd.updated_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], row_to_range_summary)
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub fn get_range_day(conn: &Connection, id: &str) -> Result<Option<RangeDayDetail>, String> {
    let day: Option<RangeDaySummary> = conn
        .query_row(
            "SELECT rd.id, rd.scheduled_date, rd.status, rd.notes, rd.completed_at, rd.created_at, rd.updated_at,
                    COALESCE((SELECT COUNT(*) FROM range_day_items i WHERE i.range_day_id = rd.id), 0)
             FROM range_days rd WHERE rd.id = ?1",
            params![id],
            row_to_range_summary,
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(summary) = day else {
        return Ok(None);
    };
    let mut stmt = conn
        .prepare(
            "SELECT rdi.asset_id, a.name, a.kind, rdi.rounds_fired
             FROM range_day_items rdi
             INNER JOIN assets a ON a.id = rdi.asset_id
             WHERE rdi.range_day_id = ?1
             ORDER BY a.name COLLATE NOCASE ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![id], |row| {
            Ok(RangeDayItemDetail {
                asset_id: row.get(0)?,
                name: row.get(1)?,
                kind: row.get(2)?,
                rounds_fired: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut items = Vec::new();
    for r in rows {
        items.push(r.map_err(|e| e.to_string())?);
    }
    let mut stmt_ammo = conn
        .prepare(
            "SELECT rfa.firearm_asset_id, f.name, rfa.ammunition_asset_id, m.name, m.caliber, m.quantity, rfa.rounds_consumed
             FROM range_day_firearm_ammo rfa
             INNER JOIN assets f ON f.id = rfa.firearm_asset_id
             INNER JOIN assets m ON m.id = rfa.ammunition_asset_id
             WHERE rfa.range_day_id = ?1
             ORDER BY f.name COLLATE NOCASE ASC, m.name COLLATE NOCASE ASC",
        )
        .map_err(|e| e.to_string())?;
    let ammo_rows = stmt_ammo
        .query_map(params![id], |row| {
            Ok(RangeDayAmmoLink {
                firearm_asset_id: row.get(0)?,
                firearm_name: row.get(1)?,
                ammunition_asset_id: row.get(2)?,
                ammunition_name: row.get(3)?,
                ammunition_caliber: row.get(4)?,
                quantity_on_hand: row.get(5)?,
                rounds_consumed: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut ammo_links = Vec::new();
    for r in ammo_rows {
        ammo_links.push(r.map_err(|e| e.to_string())?);
    }
    Ok(Some(RangeDayDetail {
        id: summary.id,
        scheduled_date: summary.scheduled_date,
        status: summary.status,
        notes: summary.notes,
        completed_at: summary.completed_at,
        created_at: summary.created_at,
        updated_at: summary.updated_at,
        items,
        ammo_links,
    }))
}

pub fn create_range_day(
    conn: &Connection,
    scheduled_date: String,
    asset_ids: Vec<String>,
) -> Result<RangeDayDetail, String> {
    validate_iso_date(&scheduled_date)?;
    let unique = dedupe_asset_ids(asset_ids);
    if unique.is_empty() {
        return Err("Select at least one firearm.".into());
    }
    validate_firearm_assets(conn, &unique)?;
    ensure_planned_no_same_date_conflict(conn, None, &scheduled_date, &unique)?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO range_days (id, scheduled_date, status, notes, completed_at, created_at, updated_at)
         VALUES (?1, ?2, 'planned', NULL, NULL, ?3, ?3)",
        params![id, scheduled_date, now],
    )
    .map_err(|e| e.to_string())?;
    for aid in &unique {
        tx.execute(
            "INSERT INTO range_day_items (range_day_id, asset_id, rounds_fired) VALUES (?1, ?2, NULL)",
            params![id, aid],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    get_range_day(conn, &id)?.ok_or_else(|| "Failed to load range day.".to_string())
}

pub fn update_range_day_planned(
    conn: &Connection,
    range_day_id: &str,
    scheduled_date: String,
    asset_ids: Vec<String>,
) -> Result<RangeDayDetail, String> {
    validate_iso_date(&scheduled_date)?;
    let status: String = conn
        .query_row(
            "SELECT status FROM range_days WHERE id = ?1",
            params![range_day_id],
            |r| r.get(0),
        )
        .map_err(|_| "Range day not found.".to_string())?;
    if status != "planned" {
        return Err("Only planned range days can be edited.".into());
    }
    let unique = dedupe_asset_ids(asset_ids);
    if unique.is_empty() {
        return Err("Select at least one firearm.".into());
    }
    validate_firearm_assets(conn, &unique)?;
    ensure_planned_no_same_date_conflict(conn, Some(range_day_id), &scheduled_date, &unique)?;
    let now = chrono::Utc::now().to_rfc3339();
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE range_days SET scheduled_date = ?2, updated_at = ?3 WHERE id = ?1",
        params![range_day_id, scheduled_date, now],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "DELETE FROM range_day_items WHERE range_day_id = ?1",
        params![range_day_id],
    )
    .map_err(|e| e.to_string())?;
    for aid in &unique {
        tx.execute(
            "INSERT INTO range_day_items (range_day_id, asset_id, rounds_fired) VALUES (?1, ?2, NULL)",
            params![range_day_id, aid],
        )
        .map_err(|e| e.to_string())?;
    }
    delete_rdfa_firearms_not_in(&tx, range_day_id, &unique)?;
    tx.commit().map_err(|e| e.to_string())?;
    get_range_day(conn, range_day_id)?.ok_or_else(|| "Failed to load range day.".to_string())
}

pub fn complete_range_day(
    conn: &Connection,
    range_day_id: &str,
    notes: Option<String>,
    rounds: Vec<RangeDayRoundEntry>,
    ammo_consumption: Vec<RangeDayAmmoConsumptionEntry>,
) -> Result<RangeDayDetail, String> {
    let now = chrono::Utc::now().to_rfc3339();
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let status: String = tx
        .query_row(
            "SELECT status FROM range_days WHERE id = ?1",
            params![range_day_id],
            |r| r.get(0),
        )
        .map_err(|_| "Range day not found.".to_string())?;
    if status != "planned" {
        return Err("Only a planned range day can be completed.".into());
    }
    let item_kinds: Vec<(String, String)> = {
        let mut stmt = tx
            .prepare(
                "SELECT rdi.asset_id, a.kind FROM range_day_items rdi
                 INNER JOIN assets a ON a.id = rdi.asset_id
                 WHERE rdi.range_day_id = ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![range_day_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| e.to_string())?);
        }
        out
    };
    if item_kinds.is_empty() {
        return Err("Range day has no firearms.".into());
    }
    let mut item_map: HashMap<String, String> = HashMap::new();
    for (aid, k) in item_kinds {
        item_map.insert(aid, k);
    }
    let mut round_map: HashMap<String, i64> = HashMap::new();
    for e in &rounds {
        if !item_map.contains_key(&e.asset_id) {
            return Err(format!(
                "Rounds entry for asset not on this range day: {}",
                e.asset_id
            ));
        }
        round_map.insert(e.asset_id.clone(), e.rounds_fired.max(0));
    }
    let ammo_links: Vec<(String, String)> = {
        let mut stmt = tx
            .prepare(
                "SELECT firearm_asset_id, ammunition_asset_id FROM range_day_firearm_ammo WHERE range_day_id = ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![range_day_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| e.to_string())?);
        }
        out
    };
    let link_set: HashSet<(String, String)> = ammo_links.iter().cloned().collect();
    let mut consumption: HashMap<(String, String), i64> = HashMap::new();
    for c in &ammo_consumption {
        if c.rounds < 0 {
            return Err("Ammunition consumption cannot be negative.".into());
        }
        if !item_map.contains_key(&c.firearm_asset_id) {
            return Err(
                "Ammunition consumption references a firearm not on this range day.".into(),
            );
        }
        *consumption
            .entry((c.firearm_asset_id.clone(), c.ammunition_asset_id.clone()))
            .or_insert(0) += c.rounds;
    }
    for pair in consumption.keys() {
        if !link_set.contains(pair) {
            return Err(
                "Ammunition consumption must match ammunition assigned to each firearm.".into(),
            );
        }
    }
    for (firearm_id, kind) in &item_map {
        if kind != "firearm" {
            continue;
        }
        let r_total = *round_map.get(firearm_id).unwrap_or(&0);
        let links_for_f: Vec<&str> = ammo_links
            .iter()
            .filter(|(f, _)| f == firearm_id)
            .map(|(_, a)| a.as_str())
            .collect();
        if links_for_f.is_empty() {
            continue;
        }
        let mut sum = 0_i64;
        for a in &links_for_f {
            sum += consumption
                .get(&(firearm_id.clone(), (*a).to_string()))
                .copied()
                .unwrap_or(0);
        }
        if sum != r_total {
            return Err(
                "For each firearm, rounds taken from assigned ammunition must equal total rounds fired."
                    .into(),
            );
        }
    }
    let mut deduct_per_ammo: HashMap<String, i64> = HashMap::new();
    for ((_, ammo_id), n) in &consumption {
        *deduct_per_ammo.entry(ammo_id.clone()).or_insert(0) += n;
    }
    for (ammo_id, need) in &deduct_per_ammo {
        if *need == 0 {
            continue;
        }
        let (kind, qty): (String, i64) = tx
            .query_row(
                "SELECT kind, quantity FROM assets WHERE id = ?1",
                params![ammo_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|_| format!("Ammunition asset not found: {ammo_id}"))?;
        if kind != "ammunition" {
            return Err("Inventory deduction target is not ammunition.".into());
        }
        if qty < *need {
            return Err(format!(
                "Not enough inventory: need {need} rounds from one assigned ammunition asset, have {qty} on hand."
            ));
        }
    }
    for (asset_id, kind) in &item_map {
        let r = *round_map.get(asset_id).unwrap_or(&0);
        tx.execute(
            "UPDATE range_day_items SET rounds_fired = ?2 WHERE range_day_id = ?1 AND asset_id = ?3",
            params![range_day_id, r, asset_id],
        )
        .map_err(|e| e.to_string())?;
        if r > 0 && kind == "firearm" {
            tx.execute(
                "UPDATE assets SET lifetime_rounds_fired = lifetime_rounds_fired + ?2,
                 rounds_fired_since_maintenance = rounds_fired_since_maintenance + ?2,
                 updated_at = ?3 WHERE id = ?1",
                params![asset_id, r, now],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    for (firearm_id, ammo_id) in &ammo_links {
        let used = consumption
            .get(&(firearm_id.clone(), ammo_id.clone()))
            .copied()
            .unwrap_or(0);
        tx.execute(
            "UPDATE range_day_firearm_ammo SET rounds_consumed = ?4 WHERE range_day_id = ?1 AND firearm_asset_id = ?2 AND ammunition_asset_id = ?3",
            params![range_day_id, firearm_id, ammo_id, used],
        )
        .map_err(|e| e.to_string())?;
    }
    for (ammo_id, need) in deduct_per_ammo {
        if need == 0 {
            continue;
        }
        tx.execute(
            "UPDATE assets SET quantity = quantity - ?2, updated_at = ?3 WHERE id = ?1 AND kind = 'ammunition'",
            params![ammo_id, need, now],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.execute(
        "UPDATE range_days SET status = 'completed', notes = ?2, completed_at = ?3, updated_at = ?3 WHERE id = ?1",
        params![range_day_id, notes, now],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    get_range_day(conn, range_day_id)?.ok_or_else(|| "Failed to load range day.".to_string())
}

pub fn cancel_range_day(conn: &Connection, range_day_id: &str) -> Result<(), String> {
    let n = conn
        .execute(
            "UPDATE range_days SET status = 'cancelled', updated_at = ?2 WHERE id = ?1 AND status = 'planned'",
            params![range_day_id, chrono::Utc::now().to_rfc3339()],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("Range day not found or not planned.".into());
    }
    Ok(())
}

pub fn delete_range_day(conn: &Connection, range_day_id: &str) -> Result<(), String> {
    let n = conn
        .execute(
            "DELETE FROM range_days WHERE id = ?1 AND status = 'planned'",
            params![range_day_id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("Range day not found or not planned.".into());
    }
    Ok(())
}

pub fn add_asset_maintenance(
    conn: &Connection,
    asset_id: &str,
    performed_at: Option<String>,
    notes: Option<String>,
) -> Result<AssetMaintenance, String> {
    let kind: String = conn
        .query_row(
            "SELECT kind FROM assets WHERE id = ?1",
            params![asset_id],
            |r| r.get(0),
        )
        .map_err(|_| "Asset not found.".to_string())?;
    if kind != "firearm" {
        return Err("Maintenance log is only for firearms.".into());
    }
    let now = chrono::Utc::now().to_rfc3339();
    let perf = performed_at
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| now.clone());
    let id = uuid::Uuid::new_v4().to_string();
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO asset_maintenance (id, asset_id, performed_at, notes, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, asset_id, perf, notes, now],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE assets SET rounds_fired_since_maintenance = 0, updated_at = ?2 WHERE id = ?1",
        params![asset_id, now],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(AssetMaintenance {
        id,
        asset_id: asset_id.to_string(),
        performed_at: perf,
        notes,
        created_at: now,
    })
}

pub fn list_asset_maintenance(
    conn: &Connection,
    asset_id: &str,
) -> Result<Vec<AssetMaintenance>, String> {
    get_asset(conn, asset_id)?.ok_or_else(|| "Asset not found.".to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, asset_id, performed_at, notes, created_at FROM asset_maintenance
             WHERE asset_id = ?1 ORDER BY performed_at DESC, created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![asset_id], |row| {
            Ok(AssetMaintenance {
                id: row.get(0)?,
                asset_id: row.get(1)?,
                performed_at: row.get(2)?,
                notes: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

// --- Dashboard ---

const DASHBOARD_TOP_FIREARMS: i64 = 8;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DashboardAmmoCaliberRow {
    pub caliber: String,
    pub rounds: i64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DashboardUpcomingMaintenanceRow {
    pub asset_id: String,
    pub name: String,
    pub summary: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DashboardTopFirearmRow {
    pub asset_id: String,
    pub name: String,
    pub lifetime_rounds_fired: i64,
    pub completed_range_day_count: i64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStats {
    pub ammo_by_caliber: Vec<DashboardAmmoCaliberRow>,
    pub upcoming_maintenance: Vec<DashboardUpcomingMaintenanceRow>,
    pub top_firearms: Vec<DashboardTopFirearmRow>,
}

fn parse_date_anchor(s: &str) -> Option<chrono::NaiveDate> {
    let s = s.trim();
    if s.len() >= 10 {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(&s[..10], "%Y-%m-%d") {
            return Some(d);
        }
    }
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.date_naive())
}

/// Next calendar maintenance due date: anchor (last maintenance date, else purchase, else created) + interval days.
fn maintenance_anchor_date(
    conn: &Connection,
    asset_id: &str,
    purchase_date: &Option<String>,
    created_at: &str,
) -> Result<Option<chrono::NaiveDate>, String> {
    let last: Option<String> = conn
        .query_row(
            "SELECT performed_at FROM asset_maintenance WHERE asset_id = ?1 ORDER BY performed_at DESC, created_at DESC LIMIT 1",
            params![asset_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some(ref s) = last {
        if let Some(d) = parse_date_anchor(s) {
            return Ok(Some(d));
        }
    }
    if let Some(ref pd) = purchase_date {
        if let Some(d) = parse_date_anchor(pd) {
            return Ok(Some(d));
        }
    }
    Ok(parse_date_anchor(created_at))
}

fn rounds_maintenance_matches(s: i64, t: i64) -> bool {
    if t <= 0 {
        return false;
    }
    let threshold = ((t as f64) * 0.9).floor() as i64;
    s >= threshold
}

fn days_maintenance_matches(days_left: i64, interval: i64) -> bool {
    if interval <= 0 {
        return false;
    }
    if days_left < 0 {
        return true;
    }
    let window = ((interval as f64) * 0.1).ceil() as i64;
    days_left <= window
}

fn build_maintenance_summary(
    s: i64,
    t_r: Option<i64>,
    t_d: Option<i64>,
    days_left: Option<i64>,
    round_hit: bool,
    day_hit: bool,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    if round_hit {
        if let Some(t) = t_r {
            parts.push(format!("{s} / {t} rounds since maintenance"));
        }
    }
    if day_hit {
        if let (Some(dleft), Some(iv)) = (days_left, t_d) {
            if dleft < 0 {
                parts.push(format!(
                    "Scheduled maintenance overdue by {} day(s)",
                    -dleft
                ));
            } else if dleft == 0 {
                parts.push("Scheduled maintenance due today".to_string());
            } else {
                parts.push(format!(
                    "Scheduled maintenance in {dleft} day(s) (every {iv} days)"
                ));
            }
        }
    }
    parts.join("; ")
}

#[derive(Clone)]
struct MaintSortable {
    row: DashboardUpcomingMaintenanceRow,
    rank: u8,
    days_left_key: i64,
    round_pressure: i64,
}

pub fn get_dashboard_stats(conn: &Connection) -> Result<DashboardStats, String> {
    let ammo_by_caliber = dashboard_ammo_by_caliber(conn)?;
    let top_firearms = dashboard_top_firearms(conn, DASHBOARD_TOP_FIREARMS)?;
    let upcoming_maintenance = dashboard_upcoming_maintenance(conn)?;
    Ok(DashboardStats {
        ammo_by_caliber,
        upcoming_maintenance,
        top_firearms,
    })
}

fn dashboard_ammo_by_caliber(conn: &Connection) -> Result<Vec<DashboardAmmoCaliberRow>, String> {
    let mut stmt = conn
        .prepare(
            r#"SELECT
                 CASE WHEN TRIM(COALESCE(caliber, '')) = '' THEN 'Unknown' ELSE TRIM(caliber) END,
                 SUM(quantity)
               FROM assets
               WHERE kind = 'ammunition'
               GROUP BY 1
               HAVING SUM(quantity) > 0
               ORDER BY SUM(quantity) DESC"#,
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(DashboardAmmoCaliberRow {
                caliber: row.get(0)?,
                rounds: row.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

fn dashboard_top_firearms(
    conn: &Connection,
    limit: i64,
) -> Result<Vec<DashboardTopFirearmRow>, String> {
    let sql = format!(
        r#"SELECT a.id, a.name, a.lifetime_rounds_fired,
            COALESCE((
              SELECT COUNT(DISTINCT rdi.range_day_id)
              FROM range_day_items rdi
              INNER JOIN range_days rd ON rd.id = rdi.range_day_id
              WHERE rdi.asset_id = a.id AND rd.status = 'completed'
            ), 0) AS day_count
           FROM assets a
           WHERE a.kind = 'firearm'
           ORDER BY a.lifetime_rounds_fired DESC, day_count DESC, a.name COLLATE NOCASE
           LIMIT {limit}"#
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(DashboardTopFirearmRow {
                asset_id: row.get(0)?,
                name: row.get(1)?,
                lifetime_rounds_fired: row.get(2)?,
                completed_range_day_count: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

fn dashboard_upcoming_maintenance(
    conn: &Connection,
) -> Result<Vec<DashboardUpcomingMaintenanceRow>, String> {
    let today = chrono::Utc::now().date_naive();
    let mut stmt = conn
        .prepare(
            r#"SELECT id, name, rounds_fired_since_maintenance,
                 maintenance_every_n_rounds, maintenance_every_n_days,
                 purchase_date, created_at
               FROM assets
               WHERE kind = 'firearm'
                 AND (maintenance_every_n_rounds IS NOT NULL OR maintenance_every_n_days IS NOT NULL)"#,
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<i64>>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut sortable: Vec<MaintSortable> = Vec::new();
    for r in rows {
        let (id, name, s, t_r, t_d, purchase_date, created_at) = r.map_err(|e| e.to_string())?;
        let anchor = maintenance_anchor_date(conn, &id, &purchase_date, &created_at)?;
        let days_left = match (t_d, anchor) {
            (Some(iv), Some(ad)) if iv > 0 => {
                let next = ad + chrono::Duration::days(iv);
                Some((next - today).num_days())
            }
            _ => None,
        };

        let round_hit = t_r.is_some_and(|t| rounds_maintenance_matches(s, t));
        let day_hit = match (days_left, t_d) {
            (Some(dl), Some(iv)) => days_maintenance_matches(dl, iv),
            _ => false,
        };
        if !round_hit && !day_hit {
            continue;
        }

        let summary = build_maintenance_summary(s, t_r, t_d, days_left, round_hit, day_hit);
        let row = DashboardUpcomingMaintenanceRow {
            asset_id: id.clone(),
            name: name.clone(),
            summary,
        };

        let round_over = t_r.is_some_and(|t| t > 0 && s >= t);
        let rank: u8 = if day_hit && days_left.is_some_and(|d| d < 0) {
            0
        } else if round_over {
            1
        } else if day_hit {
            2
        } else {
            3
        };

        let days_left_key = days_left.unwrap_or(i64::MAX);
        let round_pressure = t_r.map(|t| (s * 1000 / t.max(1)).min(10_000)).unwrap_or(0);

        sortable.push(MaintSortable {
            row,
            rank,
            days_left_key,
            round_pressure,
        });
    }

    sortable.sort_by(|a, b| {
        a.rank
            .cmp(&b.rank)
            .then_with(|| a.days_left_key.cmp(&b.days_left_key))
            .then_with(|| b.round_pressure.cmp(&a.round_pressure))
            .then_with(|| a.row.name.cmp(&b.row.name))
    });

    Ok(sortable.into_iter().map(|m| m.row).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn setup() -> (tempfile::TempDir, Connection, PathBuf) {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let img_dir = dir.path().join("images");
        init(&db_path, &img_dir).unwrap();
        let conn = open(&db_path).unwrap();
        (dir, conn, img_dir)
    }

    fn sample_input(kind: &str, name: &str) -> AssetInput {
        AssetInput {
            kind: kind.into(),
            name: name.into(),
            manufacturer: Some("MfgCo".into()),
            model: Some("Mod-A".into()),
            serial_number: Some("SN1".into()),
            caliber: Some("9mm".into()),
            quantity: Some(1),
            purchase_date: None,
            purchase_price: Some(1.0),
            notes: Some("note".into()),
            extra_json: Some("{}".into()),
            maintenance_every_n_rounds: None,
            maintenance_every_n_days: None,
            subtype: None,
            tags: None,
        }
    }

    #[test]
    fn create_list_get_update_delete_roundtrip() {
        let (_d, conn, img) = setup();
        let a = create_asset(&conn, sample_input("firearm", "Alpha")).unwrap();
        assert_eq!(a.name, "Alpha");
        let list = list_assets(&conn, None, &[]).unwrap();
        assert_eq!(list.len(), 1);
        let g = get_asset(&conn, &a.id).unwrap().unwrap();
        assert_eq!(g.kind, "firearm");
        let mut upd = sample_input("firearm", "Alpha2");
        upd.manufacturer = Some("Other".into());
        let u = update_asset(&conn, &a.id, upd).unwrap();
        assert_eq!(u.name, "Alpha2");
        assert_eq!(u.manufacturer.as_deref(), Some("Other"));
        delete_asset(&conn, &img, &a.id).unwrap();
        assert!(get_asset(&conn, &a.id).unwrap().is_none());
    }

    #[test]
    fn list_assets_filter_kind() {
        let (_d, conn, _img) = setup();
        create_asset(&conn, sample_input("firearm", "F")).unwrap();
        create_asset(&conn, sample_input("ammunition", "A")).unwrap();
        let f = list_assets(&conn, Some("firearm"), &[]).unwrap();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, "firearm");
    }

    #[test]
    fn invalid_kind_rejected() {
        let (_d, conn, _img) = setup();
        let mut bad = sample_input("firearm", "x");
        bad.kind = "spaceship".into();
        assert!(create_asset(&conn, bad).is_err());
    }

    #[test]
    fn search_assets_finds_term() {
        let (_d, conn, _img) = setup();
        create_asset(&conn, sample_input("firearm", "UniqueBoltName")).unwrap();
        let hits = search_assets(&conn, "UniqueBolt", &[]).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "UniqueBoltName");
    }

    #[test]
    fn search_empty_returns_all() {
        let (_d, conn, _img) = setup();
        create_asset(&conn, sample_input("part", "P")).unwrap();
        let all = search_assets(&conn, "  ", &[]).unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn settings_roundtrip_and_delete() {
        let (_d, conn, _img) = setup();
        assert_eq!(get_setting(&conn, "k").unwrap(), None);
        set_setting(&conn, "k", "v").unwrap();
        assert_eq!(get_setting(&conn, "k").unwrap().as_deref(), Some("v"));
        delete_setting(&conn, "k").unwrap();
        assert_eq!(get_setting(&conn, "k").unwrap(), None);
    }

    #[test]
    fn distinct_manufacturers_and_models() {
        let (_d, conn, _img) = setup();
        let mut a = sample_input("firearm", "F1");
        a.manufacturer = Some("Zeus Arms".into());
        a.model = Some("Z100".into());
        create_asset(&conn, a).unwrap();
        let mut b = sample_input("firearm", "F2");
        b.manufacturer = Some("Zeus Arms".into());
        b.model = Some("Z200".into());
        create_asset(&conn, b).unwrap();
        let m = distinct_manufacturers(&conn, "zeus", 10).unwrap();
        assert!(m.iter().any(|s| s.contains("Zeus")));
        let models = distinct_models(&conn, Some("Zeus Arms"), "", 10).unwrap();
        assert!(models.iter().any(|s| s.starts_with('Z')));
    }

    #[test]
    fn distinct_models_fallback_when_manufacturer_no_match() {
        let (_d, conn, _img) = setup();
        let mut a = sample_input("accessory", "X");
        a.manufacturer = Some("Solo".into());
        a.model = Some("OnlyModel".into());
        create_asset(&conn, a).unwrap();
        let m = distinct_models(&conn, Some("Nobody"), "", 10).unwrap();
        assert!(m.contains(&"OnlyModel".into()));
    }

    #[test]
    fn add_and_delete_image() {
        let (_d, conn, img) = setup();
        let a = create_asset(&conn, sample_input("firearm", "Pic")).unwrap();
        let im = add_image(&conn, &img, &a.id, "x.png", &[1, 2, 3], None).unwrap();
        assert_eq!(im.asset_id, a.id);
        let list = list_images(&conn, &a.id).unwrap();
        assert_eq!(list.len(), 1);
        delete_image(&conn, &img, &im.id).unwrap();
        assert!(list_images(&conn, &a.id).unwrap().is_empty());
    }

    #[test]
    fn add_image_rejects_unknown_asset() {
        let (_d, conn, img) = setup();
        let r = add_image(&conn, &img, "missing-id", "x.png", &[0], None);
        assert!(r.is_err());
    }

    #[test]
    fn read_image_file_roundtrip() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("blob.dat");
        fs::write(&p, b"hello").unwrap();
        let bytes = read_image_file(p.to_str().unwrap()).unwrap();
        assert_eq!(bytes, b"hello");
    }

    #[test]
    fn quantity_defaults_and_clamps() {
        let (_d, conn, _img) = setup();
        let mut inp = sample_input("ammunition", "Ammo");
        inp.quantity = Some(-5);
        let a = create_asset(&conn, inp).unwrap();
        assert_eq!(a.quantity, 0);
    }

    #[test]
    fn update_missing_asset_fails() {
        let (_d, conn, _img) = setup();
        let r = update_asset(&conn, "not-a-uuid", sample_input("firearm", "x"));
        assert!(r.is_err());
    }

    #[test]
    fn asset_tags_roundtrip_and_list_filter() {
        let (_d, conn, _img) = setup();
        let mut tagged = sample_input("firearm", "Tagged");
        tagged.tags = Some(vec!["Hunting".into(), "Rifle".into()]);
        let a = create_asset(&conn, tagged).unwrap();
        assert_eq!(a.tags, vec!["Hunting", "Rifle"]);
        let plain = create_asset(&conn, sample_input("firearm", "Plain")).unwrap();
        assert!(plain.tags.is_empty());

        let hunt = list_assets(&conn, None, &["Hunting".into()]).unwrap();
        assert_eq!(hunt.len(), 1);
        assert_eq!(hunt[0].id, a.id);

        let either = list_assets(&conn, None, &["Hunting".into(), "Nope".into()]).unwrap();
        assert_eq!(either.len(), 1);

        let mut upd = sample_input("firearm", "Tagged");
        upd.tags = Some(vec!["Pistol".into()]);
        let u = update_asset(&conn, &a.id, upd).unwrap();
        assert_eq!(u.tags, vec!["Pistol"]);
        let no_hunt = list_assets(&conn, None, &["Hunting".into()]).unwrap();
        assert!(no_hunt.is_empty());

        let names = suggest_tag_names(&conn, "pis", 10).unwrap();
        assert!(names.iter().any(|s| s == "Pistol"));
    }

    #[test]
    fn search_respects_tag_filter() {
        let (_d, conn, _img) = setup();
        let mut x = sample_input("accessory", "Alpha Gadget");
        x.tags = Some(vec!["Gear".into()]);
        create_asset(&conn, x).unwrap();
        create_asset(&conn, sample_input("accessory", "Beta Gadget")).unwrap();
        let hits = search_assets(&conn, "Gadget", &["Gear".into()]).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "Alpha Gadget");
    }

    #[test]
    fn range_day_complete_increments_round_counters() {
        let (_d, conn, _img) = setup();
        let g1 = create_asset(&conn, sample_input("firearm", "Gun1")).unwrap();
        let g2 = create_asset(&conn, sample_input("firearm", "Gun2")).unwrap();
        let day = create_range_day(
            &conn,
            "2030-01-15".into(),
            vec![g1.id.clone(), g2.id.clone()],
        )
        .unwrap();
        assert_eq!(day.status, "planned");
        complete_range_day(
            &conn,
            &day.id,
            Some("Good day".into()),
            vec![
                RangeDayRoundEntry {
                    asset_id: g1.id.clone(),
                    rounds_fired: 50,
                },
                RangeDayRoundEntry {
                    asset_id: g2.id.clone(),
                    rounds_fired: 30,
                },
            ],
            vec![],
        )
        .unwrap();
        let a1 = get_asset(&conn, &g1.id).unwrap().unwrap();
        assert_eq!(a1.lifetime_rounds_fired, 50);
        assert_eq!(a1.rounds_fired_since_maintenance, 50);
        let done = get_range_day(&conn, &day.id).unwrap().unwrap();
        assert_eq!(done.status, "completed");
        assert_eq!(done.notes.as_deref(), Some("Good day"));
        assert!(complete_range_day(&conn, &day.id, None, vec![], vec![]).is_err());
    }

    #[test]
    fn range_day_same_date_conflict() {
        let (_d, conn, _img) = setup();
        let g = create_asset(&conn, sample_input("firearm", "G")).unwrap();
        create_range_day(&conn, "2030-02-01".into(), vec![g.id.clone()]).unwrap();
        let r = create_range_day(&conn, "2030-02-01".into(), vec![g.id.clone()]);
        assert!(r.is_err());
        create_range_day(&conn, "2030-02-02".into(), vec![g.id.clone()]).unwrap();
    }

    #[test]
    fn range_day_update_excludes_self_from_conflict() {
        let (_d, conn, _img) = setup();
        let g = create_asset(&conn, sample_input("firearm", "G")).unwrap();
        let day = create_range_day(&conn, "2030-03-01".into(), vec![g.id.clone()]).unwrap();
        update_range_day_planned(&conn, &day.id, "2030-03-01".into(), vec![g.id.clone()]).unwrap();
    }

    #[test]
    fn maintenance_resets_since_counter_not_lifetime() {
        let (_d, conn, _img) = setup();
        let g = create_asset(&conn, sample_input("firearm", "G")).unwrap();
        let day = create_range_day(&conn, "2030-04-01".into(), vec![g.id.clone()]).unwrap();
        complete_range_day(
            &conn,
            &day.id,
            None,
            vec![RangeDayRoundEntry {
                asset_id: g.id.clone(),
                rounds_fired: 100,
            }],
            vec![],
        )
        .unwrap();
        add_asset_maintenance(&conn, &g.id, None, Some("Cleaned".into())).unwrap();
        let a = get_asset(&conn, &g.id).unwrap().unwrap();
        assert_eq!(a.lifetime_rounds_fired, 100);
        assert_eq!(a.rounds_fired_since_maintenance, 0);
        let day2 = create_range_day(&conn, "2030-04-02".into(), vec![g.id.clone()]).unwrap();
        complete_range_day(
            &conn,
            &day2.id,
            None,
            vec![RangeDayRoundEntry {
                asset_id: g.id.clone(),
                rounds_fired: 25,
            }],
            vec![],
        )
        .unwrap();
        let a2 = get_asset(&conn, &g.id).unwrap().unwrap();
        assert_eq!(a2.lifetime_rounds_fired, 125);
        assert_eq!(a2.rounds_fired_since_maintenance, 25);
    }

    #[test]
    fn range_day_ammo_caliber_must_match_firearm() {
        let (_d, conn, _img) = setup();
        let mut gun = sample_input("firearm", "G9");
        gun.caliber = Some("9mm".into());
        let g = create_asset(&conn, gun).unwrap();
        let mut bad_ammo = sample_input("ammunition", "Ammo223");
        bad_ammo.caliber = Some(".223 Rem".into());
        bad_ammo.quantity = Some(100);
        let a = create_asset(&conn, bad_ammo).unwrap();
        let day = create_range_day(&conn, "2031-01-01".into(), vec![g.id.clone()]).unwrap();
        let r = set_range_day_firearm_ammunition(&conn, &day.id, &g.id, vec![a.id.clone()]);
        assert!(r.is_err());
    }

    #[test]
    fn range_day_complete_deducts_assigned_ammunition() {
        let (_d, conn, _img) = setup();
        let mut gun = sample_input("firearm", "G9");
        gun.caliber = Some("9mm".into());
        let g = create_asset(&conn, gun).unwrap();
        let mut am1 = sample_input("ammunition", "Box1");
        am1.caliber = Some("9mm".into());
        am1.quantity = Some(100);
        let m1 = create_asset(&conn, am1).unwrap();
        let mut am2 = sample_input("ammunition", "Box2");
        am2.caliber = Some("9mm".into());
        am2.quantity = Some(50);
        let m2 = create_asset(&conn, am2).unwrap();
        let day = create_range_day(&conn, "2031-02-01".into(), vec![g.id.clone()]).unwrap();
        set_range_day_firearm_ammunition(&conn, &day.id, &g.id, vec![m1.id.clone(), m2.id.clone()])
            .unwrap();
        complete_range_day(
            &conn,
            &day.id,
            None,
            vec![RangeDayRoundEntry {
                asset_id: g.id.clone(),
                rounds_fired: 30,
            }],
            vec![
                RangeDayAmmoConsumptionEntry {
                    firearm_asset_id: g.id.clone(),
                    ammunition_asset_id: m1.id.clone(),
                    rounds: 10,
                },
                RangeDayAmmoConsumptionEntry {
                    firearm_asset_id: g.id.clone(),
                    ammunition_asset_id: m2.id.clone(),
                    rounds: 20,
                },
            ],
        )
        .unwrap();
        let q1 = get_asset(&conn, &m1.id).unwrap().unwrap().quantity;
        let q2 = get_asset(&conn, &m2.id).unwrap().unwrap().quantity;
        assert_eq!(q1, 90);
        assert_eq!(q2, 30);
    }

    #[test]
    fn dashboard_ammo_by_caliber_sums_quantity() {
        let (_d, conn, _img) = setup();
        let mut a1 = sample_input("ammunition", "B1");
        a1.caliber = Some("9mm".into());
        a1.quantity = Some(100);
        create_asset(&conn, a1).unwrap();
        let mut a2 = sample_input("ammunition", "B2");
        a2.caliber = Some("9mm".into());
        a2.quantity = Some(50);
        create_asset(&conn, a2).unwrap();
        let mut a3 = sample_input("ammunition", "B3");
        a3.caliber = Some(".223 Rem".into());
        a3.quantity = Some(20);
        create_asset(&conn, a3).unwrap();
        let stats = get_dashboard_stats(&conn).unwrap();
        let nine: i64 = stats
            .ammo_by_caliber
            .iter()
            .find(|r| r.caliber == "9mm")
            .map(|r| r.rounds)
            .unwrap_or(0);
        assert_eq!(nine, 150);
    }

    #[test]
    fn dashboard_upcoming_lists_firearm_near_round_threshold() {
        let (_d, conn, _img) = setup();
        let mut gun = sample_input("firearm", "G");
        gun.maintenance_every_n_rounds = Some(100);
        let g = create_asset(&conn, gun).unwrap();
        conn.execute(
            "UPDATE assets SET rounds_fired_since_maintenance = 95 WHERE id = ?1",
            params![g.id],
        )
        .unwrap();
        let stats = get_dashboard_stats(&conn).unwrap();
        assert_eq!(stats.upcoming_maintenance.len(), 1);
        assert_eq!(stats.upcoming_maintenance[0].asset_id, g.id);
        assert!(stats.upcoming_maintenance[0].summary.contains("95"));
    }

    #[test]
    fn dashboard_top_firearms_orders_by_lifetime_rounds() {
        let (_d, conn, _img) = setup();
        let g1 = create_asset(&conn, sample_input("firearm", "A")).unwrap();
        let g2 = create_asset(&conn, sample_input("firearm", "B")).unwrap();
        conn.execute(
            "UPDATE assets SET lifetime_rounds_fired = 200 WHERE id = ?1",
            params![g2.id],
        )
        .unwrap();
        conn.execute(
            "UPDATE assets SET lifetime_rounds_fired = 50 WHERE id = ?1",
            params![g1.id],
        )
        .unwrap();
        let stats = get_dashboard_stats(&conn).unwrap();
        assert!(stats.top_firearms.len() >= 2);
        assert_eq!(stats.top_firearms[0].name, "B");
        assert_eq!(stats.top_firearms[0].lifetime_rounds_fired, 200);
    }

    #[test]
    fn asset_subtype_firearm_and_accessory() {
        let (_d, conn, _img) = setup();
        let mut inp = sample_input("firearm", "F");
        inp.subtype = Some("revolver".into());
        let a = create_asset(&conn, inp).unwrap();
        assert_eq!(a.subtype.as_deref(), Some("revolver"));

        let mut bad = sample_input("firearm", "F2");
        bad.subtype = Some("scope".into());
        assert!(create_asset(&conn, bad).is_err());

        let mut pcc = sample_input("firearm", "F3");
        pcc.subtype = Some("pcc_sub".into());
        let p = create_asset(&conn, pcc).unwrap();
        assert_eq!(p.subtype.as_deref(), Some("pcc_sub"));

        let mut acc = sample_input("accessory", "A");
        acc.subtype = Some("reddot".into());
        let b = create_asset(&conn, acc).unwrap();
        assert_eq!(b.subtype.as_deref(), Some("reddot"));

        let mut ammo = sample_input("ammunition", "Ammo");
        ammo.subtype = Some("shotgun".into());
        let c = create_asset(&conn, ammo).unwrap();
        assert_eq!(c.subtype.as_deref(), Some("shotgun"));

        let mut bad_ammo = sample_input("ammunition", "Ammo2");
        bad_ammo.subtype = Some("scope".into());
        assert!(create_asset(&conn, bad_ammo).is_err());
    }

    #[test]
    fn asset_subtype_cleared_for_part_even_if_sent() {
        let (_d, conn, _img) = setup();
        let mut inp = sample_input("part", "P");
        inp.subtype = Some("pistol".into());
        let a = create_asset(&conn, inp).unwrap();
        assert_eq!(a.subtype, None);
    }
}
