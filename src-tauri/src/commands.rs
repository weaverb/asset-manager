use crate::db::{
    self, Asset, AssetImage, AssetInput, AssetMaintenance, RangeDayAmmoConsumptionEntry,
    RangeDayDetail, RangeDayRoundEntry, RangeDaySummary,
};
use crate::gunspec;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tauri::State;

pub struct AppPaths {
    pub db_path: PathBuf,
    pub images_dir: PathBuf,
}

fn with_conn<R>(
    paths: &AppPaths,
    f: impl FnOnce(&rusqlite::Connection) -> Result<R, String>,
) -> Result<R, String> {
    let conn = db::open(&paths.db_path)?;
    f(&conn)
}

fn merge_suggestions(learned: Vec<String>, remote: Vec<String>, cap: usize) -> Vec<String> {
    let mut seen = HashSet::<String>::new();
    let mut out = Vec::new();
    for s in learned.into_iter().chain(remote.into_iter()) {
        let k = s.to_lowercase();
        if seen.insert(k) {
            out.push(s);
        }
        if out.len() >= cap {
            break;
        }
    }
    out
}

fn ensure_image_path(root: &Path, path: &Path) -> Result<(), String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    if !path.starts_with(&root) {
        return Err("Invalid image path".into());
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub gunspec_api_key: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldSuggestions {
    pub items: Vec<String>,
    pub gunspec_notice: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagePayload {
    pub mime: String,
    pub data_base64: String,
}

fn mime_from_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("heic") => "image/heic",
        _ => "application/octet-stream",
    }
}

pub(crate) fn exec_list_assets(
    paths: &AppPaths,
    kind: Option<String>,
    tag_names: Option<Vec<String>>,
) -> Result<Vec<Asset>, String> {
    let tags = tag_names.unwrap_or_default();
    with_conn(paths, |c| db::list_assets(c, kind.as_deref(), &tags))
}

#[tauri::command]
pub fn list_assets(
    kind: Option<String>,
    tag_names: Option<Vec<String>>,
    paths: State<AppPaths>,
) -> Result<Vec<Asset>, String> {
    exec_list_assets(&paths, kind, tag_names)
}

pub(crate) fn exec_search_assets(
    paths: &AppPaths,
    query: String,
    tag_names: Option<Vec<String>>,
) -> Result<Vec<Asset>, String> {
    let tags = tag_names.unwrap_or_default();
    with_conn(paths, |c| db::search_assets(c, &query, &tags))
}

#[tauri::command]
pub fn search_assets(
    query: String,
    tag_names: Option<Vec<String>>,
    paths: State<AppPaths>,
) -> Result<Vec<Asset>, String> {
    exec_search_assets(&paths, query, tag_names)
}

pub(crate) fn exec_get_asset(paths: &AppPaths, id: String) -> Result<Option<Asset>, String> {
    with_conn(paths, |c| db::get_asset(c, &id))
}

#[tauri::command]
pub fn get_asset(id: String, paths: State<AppPaths>) -> Result<Option<Asset>, String> {
    exec_get_asset(&paths, id)
}

pub(crate) fn exec_create_asset(paths: &AppPaths, input: AssetInput) -> Result<Asset, String> {
    with_conn(paths, |c| db::create_asset(c, input))
}

#[tauri::command]
pub fn create_asset(input: AssetInput, paths: State<AppPaths>) -> Result<Asset, String> {
    exec_create_asset(&paths, input)
}

pub(crate) fn exec_update_asset(
    paths: &AppPaths,
    id: String,
    input: AssetInput,
) -> Result<Asset, String> {
    with_conn(paths, |c| db::update_asset(c, &id, input))
}

#[tauri::command]
pub fn update_asset(
    id: String,
    input: AssetInput,
    paths: State<AppPaths>,
) -> Result<Asset, String> {
    exec_update_asset(&paths, id, input)
}

pub(crate) fn exec_delete_asset(paths: &AppPaths, id: String) -> Result<(), String> {
    with_conn(paths, |c| db::delete_asset(c, &paths.images_dir, &id))
}

#[tauri::command]
pub fn delete_asset(id: String, paths: State<AppPaths>) -> Result<(), String> {
    exec_delete_asset(&paths, id)
}

pub(crate) fn exec_list_asset_images(
    paths: &AppPaths,
    asset_id: String,
) -> Result<Vec<AssetImage>, String> {
    with_conn(paths, |c| db::list_images(c, &asset_id))
}

#[tauri::command]
pub fn list_asset_images(
    asset_id: String,
    paths: State<AppPaths>,
) -> Result<Vec<AssetImage>, String> {
    exec_list_asset_images(&paths, asset_id)
}

pub(crate) fn exec_add_asset_image(
    paths: &AppPaths,
    asset_id: String,
    original_name: String,
    data_base64: String,
    caption: Option<String>,
) -> Result<AssetImage, String> {
    let bytes = STANDARD
        .decode(data_base64.trim())
        .map_err(|e| format!("Invalid base64: {e}"))?;
    with_conn(paths, |c| {
        db::add_image(
            c,
            &paths.images_dir,
            &asset_id,
            &original_name,
            &bytes,
            caption,
        )
    })
}

#[tauri::command]
pub fn add_asset_image(
    asset_id: String,
    original_name: String,
    data_base64: String,
    caption: Option<String>,
    paths: State<AppPaths>,
) -> Result<AssetImage, String> {
    exec_add_asset_image(&paths, asset_id, original_name, data_base64, caption)
}

pub(crate) fn exec_delete_asset_image(paths: &AppPaths, image_id: String) -> Result<(), String> {
    with_conn(paths, |c| db::delete_image(c, &paths.images_dir, &image_id))
}

#[tauri::command]
pub fn delete_asset_image(image_id: String, paths: State<AppPaths>) -> Result<(), String> {
    exec_delete_asset_image(&paths, image_id)
}

pub(crate) fn exec_get_app_settings(paths: &AppPaths) -> Result<AppSettings, String> {
    with_conn(paths, |c| {
        let gunspec_api_key = db::get_setting(c, "gunspec_api_key")?.unwrap_or_default();
        Ok(AppSettings { gunspec_api_key })
    })
}

#[tauri::command]
pub fn get_app_settings(paths: State<AppPaths>) -> Result<AppSettings, String> {
    exec_get_app_settings(&paths)
}

pub(crate) fn exec_save_app_settings(
    paths: &AppPaths,
    settings: AppSettings,
) -> Result<(), String> {
    with_conn(paths, |c| {
        db::set_setting(c, "gunspec_api_key", settings.gunspec_api_key.trim())?;
        gunspec::clear_cache();
        Ok(())
    })
}

#[tauri::command]
pub fn save_app_settings(settings: AppSettings, paths: State<AppPaths>) -> Result<(), String> {
    exec_save_app_settings(&paths, settings)
}

pub(crate) fn exec_suggest_manufacturers(
    paths: &AppPaths,
    query: String,
) -> Result<FieldSuggestions, String> {
    with_conn(paths, |c| {
        let key = db::get_setting(c, "gunspec_api_key")?
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let api_opt = key.as_deref();
        let learned = db::distinct_manufacturers(c, &query, 20)?;
        let (remote, gunspec_notice) = if api_opt.is_some() {
            gunspec::suggest_manufacturers(&query, api_opt)
        } else {
            (vec![], None)
        };
        Ok(FieldSuggestions {
            items: merge_suggestions(learned, remote, 30),
            gunspec_notice,
        })
    })
}

#[tauri::command]
pub fn suggest_manufacturers(
    query: String,
    paths: State<AppPaths>,
) -> Result<FieldSuggestions, String> {
    exec_suggest_manufacturers(&paths, query)
}

pub(crate) fn exec_suggest_models(
    paths: &AppPaths,
    manufacturer: String,
    query: String,
) -> Result<FieldSuggestions, String> {
    with_conn(paths, |c| {
        let key = db::get_setting(c, "gunspec_api_key")?
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let api_opt = key.as_deref();
        let m = manufacturer.trim();
        let mf = if m.is_empty() { None } else { Some(m) };
        let learned = db::distinct_models(c, mf, &query, 20)?;
        let (remote, gunspec_notice) = if api_opt.is_some() {
            gunspec::suggest_models(&manufacturer, &query, api_opt)
        } else {
            (vec![], None)
        };
        Ok(FieldSuggestions {
            items: merge_suggestions(learned, remote, 30),
            gunspec_notice,
        })
    })
}

#[tauri::command]
pub fn suggest_models(
    manufacturer: String,
    query: String,
    paths: State<AppPaths>,
) -> Result<FieldSuggestions, String> {
    exec_suggest_models(&paths, manufacturer, query)
}

pub(crate) fn exec_suggest_tags(
    paths: &AppPaths,
    query: String,
) -> Result<FieldSuggestions, String> {
    with_conn(paths, |c| {
        let items = db::suggest_tag_names(c, &query, 40)?;
        Ok(FieldSuggestions {
            items,
            gunspec_notice: None,
        })
    })
}

#[tauri::command]
pub fn suggest_tags(query: String, paths: State<AppPaths>) -> Result<FieldSuggestions, String> {
    exec_suggest_tags(&paths, query)
}

pub(crate) fn exec_dev_drop_and_reseed(paths: &AppPaths) -> Result<(), String> {
    if !cfg!(debug_assertions) {
        return Err("Dev-only: use a debug build (npm run tauri dev).".into());
    }
    let conn = db::open(&paths.db_path)?;
    crate::dev_seed::drop_and_reseed(&conn, &paths.images_dir)
}

#[tauri::command]
pub fn dev_drop_and_reseed(paths: State<AppPaths>) -> Result<(), String> {
    exec_dev_drop_and_reseed(&paths)
}

pub(crate) fn exec_get_image_data(paths: &AppPaths, path: String) -> Result<ImagePayload, String> {
    let path_buf = PathBuf::from(&path);
    ensure_image_path(&paths.images_dir, &path_buf)?;
    let data = db::read_image_file(&path)?;
    let mime = mime_from_path(&path_buf).to_string();
    Ok(ImagePayload {
        mime,
        data_base64: STANDARD.encode(data),
    })
}

#[tauri::command]
pub fn get_image_data(path: String, paths: State<AppPaths>) -> Result<ImagePayload, String> {
    exec_get_image_data(&paths, path)
}

#[tauri::command]
pub fn list_range_days(paths: State<AppPaths>) -> Result<Vec<RangeDaySummary>, String> {
    with_conn(&paths, db::list_range_days)
}

#[tauri::command]
pub fn get_range_day(id: String, paths: State<AppPaths>) -> Result<RangeDayDetail, String> {
    with_conn(&paths, |c| {
        db::get_range_day(c, &id)?.ok_or_else(|| "Range day not found.".into())
    })
}

#[tauri::command]
pub fn create_range_day(
    scheduled_date: String,
    asset_ids: Vec<String>,
    paths: State<AppPaths>,
) -> Result<RangeDayDetail, String> {
    with_conn(&paths, |c| {
        db::create_range_day(c, scheduled_date, asset_ids)
    })
}

#[tauri::command]
pub fn update_range_day_planned(
    id: String,
    scheduled_date: String,
    asset_ids: Vec<String>,
    paths: State<AppPaths>,
) -> Result<RangeDayDetail, String> {
    with_conn(&paths, |c| {
        db::update_range_day_planned(c, &id, scheduled_date, asset_ids)
    })
}

#[tauri::command]
pub fn complete_range_day(
    id: String,
    notes: Option<String>,
    rounds: Vec<RangeDayRoundEntry>,
    ammo_consumption: Vec<RangeDayAmmoConsumptionEntry>,
    paths: State<AppPaths>,
) -> Result<RangeDayDetail, String> {
    with_conn(&paths, |c| {
        db::complete_range_day(c, &id, notes, rounds, ammo_consumption)
    })
}

#[tauri::command]
pub fn set_range_day_firearm_ammunition(
    id: String,
    firearm_asset_id: String,
    ammunition_asset_ids: Vec<String>,
    paths: State<AppPaths>,
) -> Result<RangeDayDetail, String> {
    with_conn(&paths, |c| {
        db::set_range_day_firearm_ammunition(c, &id, &firearm_asset_id, ammunition_asset_ids)
    })
}

#[tauri::command]
pub fn cancel_range_day(id: String, paths: State<AppPaths>) -> Result<(), String> {
    with_conn(&paths, |c| db::cancel_range_day(c, &id))
}

#[tauri::command]
pub fn delete_range_day(id: String, paths: State<AppPaths>) -> Result<(), String> {
    with_conn(&paths, |c| db::delete_range_day(c, &id))
}

#[tauri::command]
pub fn add_asset_maintenance(
    asset_id: String,
    performed_at: Option<String>,
    notes: Option<String>,
    paths: State<AppPaths>,
) -> Result<AssetMaintenance, String> {
    with_conn(&paths, |c| {
        db::add_asset_maintenance(c, &asset_id, performed_at, notes)
    })
}

#[tauri::command]
pub fn list_asset_maintenance(
    asset_id: String,
    paths: State<AppPaths>,
) -> Result<Vec<AssetMaintenance>, String> {
    with_conn(&paths, |c| db::list_asset_maintenance(c, &asset_id))
}

#[tauri::command]
pub fn get_dashboard_stats(paths: State<AppPaths>) -> Result<db::DashboardStats, String> {
    with_conn(&paths, db::get_dashboard_stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::gunspec::test_support;
    use mockito::Matcher;
    use std::fs;
    use tempfile::tempdir;

    fn open_paths() -> (tempfile::TempDir, AppPaths) {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("db.sqlite");
        let images_dir = dir.path().join("images");
        db::init(&db_path, &images_dir).unwrap();
        (
            dir,
            AppPaths {
                db_path,
                images_dir,
            },
        )
    }

    fn sample_asset(name: &str, manufacturer: &str, model: &str) -> db::AssetInput {
        db::AssetInput {
            kind: "firearm".into(),
            name: name.into(),
            manufacturer: Some(manufacturer.into()),
            model: Some(model.into()),
            serial_number: None,
            caliber: None,
            quantity: Some(1),
            purchase_date: None,
            purchase_price: None,
            notes: None,
            extra_json: Some("{}".into()),
            maintenance_every_n_rounds: None,
            maintenance_every_n_days: None,
            subtype: None,
            tags: None,
        }
    }

    #[test]
    fn merge_suggestions_dedupes_case_insensitive() {
        let out = merge_suggestions(
            vec!["A".into(), "b".into()],
            vec!["a".into(), "c".into()],
            10,
        );
        assert_eq!(out, vec!["A", "b", "c"]);
    }

    #[test]
    fn merge_suggestions_respects_cap() {
        let out = merge_suggestions(vec!["1".into(), "2".into(), "3".into()], vec![], 2);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn mime_from_path_common_extensions() {
        assert_eq!(mime_from_path(Path::new("x.jpg")), "image/jpeg");
        assert_eq!(mime_from_path(Path::new("x.JPEG")), "image/jpeg");
        assert_eq!(mime_from_path(Path::new("p.png")), "image/png");
        assert_eq!(mime_from_path(Path::new("w.webp")), "image/webp");
        assert_eq!(mime_from_path(Path::new("a.gif")), "image/gif");
        assert_eq!(mime_from_path(Path::new("h.heic")), "image/heic");
        assert_eq!(
            mime_from_path(Path::new("unknown.bin")),
            "application/octet-stream"
        );
    }

    #[test]
    fn ensure_image_path_rejects_escape() {
        let root = tempdir().unwrap();
        let nested = root.path().join("a");
        fs::create_dir_all(&nested).unwrap();
        let f = nested.join("pic.png");
        fs::write(&f, []).unwrap();
        assert!(ensure_image_path(root.path(), &f).is_ok());
        let outside = std::env::temp_dir().join("outside-tauri-test.bin");
        let _ = fs::write(&outside, []);
        assert!(ensure_image_path(root.path(), &outside).is_err());
        let _ = fs::remove_file(&outside);
    }

    #[test]
    fn exec_crud_and_images_roundtrip() {
        let (_dir, paths) = open_paths();
        let a = exec_create_asset(&paths, sample_asset("Cmd A", "Mfg", "M1")).unwrap();
        let listed = exec_list_assets(&paths, None, None).unwrap();
        assert_eq!(listed.len(), 1);
        let one = exec_get_asset(&paths, a.id.clone()).unwrap().unwrap();
        assert_eq!(one.name, "Cmd A");
        let found = exec_search_assets(&paths, "Cmd".into(), None).unwrap();
        assert_eq!(found.len(), 1);
        let upd = sample_asset("Cmd A2", "Mfg", "M2");
        let u = exec_update_asset(&paths, a.id.clone(), upd).unwrap();
        assert_eq!(u.name, "Cmd A2");
        let img = exec_add_asset_image(
            &paths,
            a.id.clone(),
            "shot.png".into(),
            STANDARD.encode([1u8, 2, 3]),
            Some("cap".into()),
        )
        .unwrap();
        let imgs = exec_list_asset_images(&paths, a.id.clone()).unwrap();
        assert_eq!(imgs.len(), 1);
        let payload = exec_get_image_data(&paths, img.file_path.clone()).unwrap();
        assert_eq!(payload.mime, "image/png");
        assert_eq!(
            STANDARD.decode(payload.data_base64).unwrap(),
            vec![1u8, 2, 3]
        );
        exec_delete_asset_image(&paths, img.id).unwrap();
        assert!(exec_list_asset_images(&paths, a.id.clone())
            .unwrap()
            .is_empty());
        exec_delete_asset(&paths, a.id).unwrap();
        assert!(exec_list_assets(&paths, None, None).unwrap().is_empty());
    }

    #[test]
    fn exec_add_asset_image_rejects_bad_base64() {
        let (_dir, paths) = open_paths();
        let a = exec_create_asset(&paths, sample_asset("X", "Y", "Z")).unwrap();
        let r = exec_add_asset_image(&paths, a.id, "x.png".into(), "not!!!base64".into(), None);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("base64"));
    }

    #[test]
    fn exec_settings_trims_gunspec_key() {
        let (_dir, paths) = open_paths();
        exec_save_app_settings(
            &paths,
            AppSettings {
                gunspec_api_key: "  secret  \n".into(),
            },
        )
        .unwrap();
        let s = exec_get_app_settings(&paths).unwrap();
        assert_eq!(s.gunspec_api_key, "secret");
    }

    #[test]
    fn exec_suggest_manufacturers_learned_without_api_key() {
        let (_dir, paths) = open_paths();
        exec_create_asset(&paths, sample_asset("L", "LearnedBrandCmd", "M")).unwrap();
        let f = exec_suggest_manufacturers(&paths, "learned".into()).unwrap();
        assert!(f.gunspec_notice.is_none());
        assert!(f
            .items
            .iter()
            .any(|s| s.eq_ignore_ascii_case("LearnedBrandCmd")));
    }

    #[test]
    fn exec_suggest_manufacturers_merges_remote_when_key_set() {
        let _lock = test_support::lock_http();
        let (_dir, paths) = open_paths();
        exec_save_app_settings(
            &paths,
            AppSettings {
                gunspec_api_key: "k".into(),
            },
        )
        .unwrap();
        exec_create_asset(&paths, sample_asset("R", "LocalOnlyCmd", "M")).unwrap();
        let mut server = mockito::Server::new();
        let body = r#"{"success":true,"data":[{"id":"z","name":"RemoteCo"}],"pagination":{"page":1,"limit":100,"total":1,"totalPages":1}}"#;
        let _m = server
            .mock("GET", "/v1/manufacturers")
            .match_query(Matcher::Any)
            .with_status(200)
            .with_body(body)
            .create();
        test_support::set_base(Some(server.url()));
        let f = exec_suggest_manufacturers(&paths, "remote".into()).unwrap();
        test_support::reset_remote();
        assert!(f.gunspec_notice.is_none());
        assert!(f.items.iter().any(|s| s == "RemoteCo"));
    }

    #[test]
    fn exec_suggest_models_learned_empty_manufacturer() {
        let (_dir, paths) = open_paths();
        exec_create_asset(&paths, sample_asset("N", "Any", "DeltaCmd")).unwrap();
        let f = exec_suggest_models(&paths, "   ".into(), "delta".into()).unwrap();
        assert!(f.gunspec_notice.is_none());
        assert!(f.items.iter().any(|s| s.contains("DeltaCmd")));
    }

    #[test]
    fn exec_list_assets_filters_kind() {
        let (_dir, paths) = open_paths();
        let mut ammo = sample_asset("Ammo", "A", "B");
        ammo.kind = "ammunition".into();
        exec_create_asset(&paths, sample_asset("Gun", "A", "B")).unwrap();
        exec_create_asset(&paths, ammo).unwrap();
        let guns = exec_list_assets(&paths, Some("firearm".into()), None).unwrap();
        assert_eq!(guns.len(), 1);
        assert_eq!(guns[0].kind, "firearm");
    }

    #[test]
    fn exec_dev_drop_and_reseed_populates_sample_rows() {
        if !cfg!(debug_assertions) {
            return;
        }
        let (_dir, paths) = open_paths();
        exec_create_asset(&paths, sample_asset("One", "X", "Y")).unwrap();
        assert_eq!(exec_list_assets(&paths, None, None).unwrap().len(), 1);
        exec_dev_drop_and_reseed(&paths).unwrap();
        let all = exec_list_assets(&paths, None, None).unwrap();
        assert_eq!(all.len(), 16);
    }
}
