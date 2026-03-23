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
        "#,
    )
    .map_err(|e| e.to_string())?;

    // External-content FTS5 can drift from `assets` (e.g. legacy bugs, interrupted writes).
    // Rebuilding from the content table keeps MATCH results aligned with visible row text.
    let n_assets: i64 = conn
        .query_row("SELECT COUNT(*) FROM assets", [], |r| r.get(0))
        .unwrap_or(0);
    if n_assets > 0 {
        conn.execute(
            "INSERT INTO assets_fts(assets_fts) VALUES('rebuild')",
            [],
        )
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
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
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

fn list_assets_sql_values(kind: Option<&str>, tag_names: &[String]) -> (String, Vec<rusqlite::types::Value>) {
    let tags_clean: Vec<String> = tag_names
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let (filter_tags, tag_clause) = tag_filter_sql(&tags_clean);

    let mut sql = String::from(
        "SELECT a.id, a.kind, a.name, a.manufacturer, a.model, a.serial_number, a.caliber, a.quantity, a.purchase_date, a.purchase_price, a.notes, a.extra_json, a.created_at, a.updated_at FROM assets a WHERE 1=1",
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

pub fn replace_asset_tags(conn: &Connection, asset_id: &str, tags: &[String]) -> Result<(), String> {
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
        "SELECT a.id, a.kind, a.name, a.manufacturer, a.model, a.serial_number, a.caliber, a.quantity, a.purchase_date, a.purchase_price, a.notes, a.extra_json, a.created_at, a.updated_at
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
            "SELECT id, kind, name, manufacturer, model, serial_number, caliber, quantity, purchase_date, purchase_price, notes, extra_json, created_at, updated_at
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
    let extra = input.extra_json.unwrap_or_else(|| "{}".to_string());
    conn.execute(
        r#"INSERT INTO assets (
            id, kind, name, manufacturer, model, serial_number, caliber, quantity,
            purchase_date, purchase_price, notes, extra_json, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"#,
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
    get_asset(conn, id)?
        .ok_or_else(|| format!("Asset not found: {id}"))?;
    let now = chrono::Utc::now().to_rfc3339();
    let quantity = input.quantity.unwrap_or(1).max(0);
    let extra = input.extra_json.unwrap_or_else(|| "{}".to_string());
    conn.execute(
        r#"UPDATE assets SET
            kind = ?2, name = ?3, manufacturer = ?4, model = ?5, serial_number = ?6,
            caliber = ?7, quantity = ?8, purchase_date = ?9, purchase_price = ?10,
            notes = ?11, extra_json = ?12, updated_at = ?13
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
pub fn distinct_manufacturers(conn: &Connection, prefix: &str, limit: i64) -> Result<Vec<String>, String> {
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
pub fn suggest_tag_names(conn: &Connection, prefix: &str, limit: i64) -> Result<Vec<String>, String> {
    let prefix = prefix.trim();
    if prefix.is_empty() {
        let mut stmt = conn
            .prepare(
                "SELECT name FROM tags ORDER BY name COLLATE NOCASE ASC LIMIT ?1",
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
            "SELECT name FROM tags WHERE LOWER(name) LIKE LOWER(?2) ORDER BY name COLLATE NOCASE ASC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![limit, like], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    rows_to_strings(rows)
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
}
