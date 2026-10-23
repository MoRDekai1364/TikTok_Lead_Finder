#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};

#[derive(serde::Serialize)]
struct CoreConnection {
    url: String,
    token: String,
}

fn resolve_data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("LEADFINDER_DATA_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(dir) = std::env::var("LEADFINDER_APP_DATA_DIR") {
        return PathBuf::from(dir).join("data");
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let cwd = std::env::current_dir().ok();
    for start in [cwd, exe_dir].into_iter().flatten() {
        for ancestor in start.ancestors().take(5) {
            if ancestor.join("config").join("default.toml").exists()
                && (ancestor.join("Cargo.toml").exists()
                    || std::env::var("LEADFINDER_PORTABLE").ok().as_deref() == Some("1"))
            {
                return ancestor.join("data");
            }
        }
    }
    match std::env::var("LOCALAPPDATA") {
        Ok(local) => PathBuf::from(local).join("TikTok_Lead_Finder").join("data"),
        Err(_) => PathBuf::from("data"),
    }
}

#[tauri::command]
fn core_connection() -> CoreConnection {
    let token = std::env::var("LEADFINDER_API_TOKEN")
        .ok()
        .filter(|t| !t.trim().is_empty())
        .or_else(|| std::fs::read_to_string(resolve_data_dir().join("api_token")).ok())
        .map(|t| t.trim().to_string())
        .unwrap_or_default();
    let url = std::env::var("LEADFINDER_CORE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:17420".to_string());
    CoreConnection { url, token }
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![core_connection])
        .run(tauri::generate_context!())
        .expect("failed to run tauri desktop application");
}
