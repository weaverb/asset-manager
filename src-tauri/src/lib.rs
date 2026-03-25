mod commands;
mod db;
mod dev_seed;
mod gunspec;

use commands::AppPaths;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let resolver = app.path();
            let base = resolver
                .app_data_dir()
                .map_err(|e| format!("Failed to resolve app data dir: {e}"))?;
            std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
            let db_path = base.join("asset_manager.db");
            let images_dir = base.join("images");
            db::init(&db_path, &images_dir)?;
            #[cfg(debug_assertions)]
            {
                let conn = db::open(&db_path)?;
                dev_seed::ensure_dev_seed(&conn, &images_dir)?;
            }
            app.manage(AppPaths {
                db_path,
                images_dir,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_assets,
            commands::search_assets,
            commands::get_asset,
            commands::create_asset,
            commands::update_asset,
            commands::delete_asset,
            commands::list_asset_images,
            commands::add_asset_image,
            commands::delete_asset_image,
            commands::get_image_data,
            commands::get_app_settings,
            commands::save_app_settings,
            commands::suggest_manufacturers,
            commands::suggest_models,
            commands::suggest_tags,
            commands::dev_drop_and_reseed,
            commands::list_range_days,
            commands::get_range_day,
            commands::create_range_day,
            commands::update_range_day_planned,
            commands::complete_range_day,
            commands::set_range_day_firearm_ammunition,
            commands::cancel_range_day,
            commands::delete_range_day,
            commands::add_asset_maintenance,
            commands::list_asset_maintenance,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod lib_tests {
    #[test]
    fn dev_seed_flag_key_documented() {
        assert_eq!(super::dev_seed::DEV_SEED_FLAG_KEY, "dev_inventory_seeded");
    }
}
