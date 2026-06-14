use tauri::AppHandle;

use super::AppInfo;

#[tauri::command]
pub async fn get_app_info(app: AppHandle) -> AppInfo {
    let package = app.package_info();
    AppInfo {
        name: package.name.clone(),
        version: package.version.to_string(),
        platform: std::env::consts::OS.to_string(),
        is_dev: cfg!(debug_assertions),
    }
}
