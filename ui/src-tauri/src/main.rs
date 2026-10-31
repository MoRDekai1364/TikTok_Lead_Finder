#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use tauri::{Emitter, Manager};

#[derive(serde::Serialize)]
struct CoreConnection {
    url: String,
    token: String,
}

#[derive(serde::Serialize)]
struct DisplayHardwareInfo {
    physical_width: u32,
    physical_height: u32,
    scale_factor: f64,
    logical_width: f64,
    logical_height: f64,
    aspect_ratio: f64,
    monitor_name: Option<String>,
}

#[tauri::command]
fn get_display_hardware_info(window: tauri::Window) -> Option<DisplayHardwareInfo> {
    let monitor = window.current_monitor().ok().flatten()?;
    let size = monitor.size();
    let scale = monitor.scale_factor().max(1.0);
    let logical_w = size.width as f64 / scale;
    let logical_h = size.height as f64 / scale;
    let aspect = if size.height > 0 {
        size.width as f64 / size.height as f64
    } else {
        1.6
    };
    Some(DisplayHardwareInfo {
        physical_width: size.width,
        physical_height: size.height,
        scale_factor: scale,
        logical_width: logical_w,
        logical_height: logical_h,
        aspect_ratio: aspect,
        monitor_name: monitor.name().cloned(),
    })
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

#[tauri::command]
fn open_in_browser(url: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", &url])
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
    }
    Ok(())
}

#[tauri::command]
async fn open_in_webview(app: tauri::AppHandle, url: String) -> Result<(), String> {
    if let Ok(parsed) = url.parse::<url::Url>() {
        let label = format!("webview_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0));
        let win = tauri::WebviewWindowBuilder::new(&app, label, tauri::WebviewUrl::External(parsed))
            .title("LeadFinder - Преглед на източник")
            .inner_size(1100.0, 750.0)
            .resizable(true)
            .build();
        if let Err(e) = win {
            return Err(e.to_string());
        }
        return Ok(());
    }
    Err("Invalid URL".to_string())
}

#[tauri::command]
fn terminate_application(app: tauri::AppHandle) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/IM", "leadfinder.exe", "/T"])
            .creation_flags(0x08000000)
            .status();
    }
    app.exit(0);
    std::process::exit(0);
}

#[tauri::command]
fn terminate_desktop(app: tauri::AppHandle) {
    terminate_application(app);
}

#[tauri::command]
fn exit_and_run_background(app: tauri::AppHandle) {
    app.exit(0);
    std::process::exit(0);
}

fn ensure_core_running() {
    use std::net::TcpStream;
    use std::time::Duration;
    if TcpStream::connect_timeout(&"127.0.0.1:17420".parse().unwrap(), Duration::from_millis(200)).is_ok() {
        return;
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let cwd = std::env::current_dir().ok();
    for start in [cwd, exe_dir].into_iter().flatten() {
        for ancestor in start.ancestors().take(5) {
            let candidates = [
                ancestor.join("leadfinder.exe"),
                ancestor.join("release_stage").join("leadfinder.exe"),
                ancestor.join("target").join("release").join("leadfinder.exe"),
            ];
            for cand in candidates {
                if cand.exists() {
                    let _ = std::process::Command::new(cand)
                        .arg("serve")
                        .current_dir(ancestor)
                        .spawn();
                    std::thread::sleep(Duration::from_millis(500));
                    return;
                }
            }
        }
    }
}

fn main() {
    ensure_core_running();
    tauri::Builder::default()
        .setup(|app| {
            if let Some(win) = app.get_webview_window("main") {
                if let Ok(Some(monitor)) = win.current_monitor() {
                    let size = monitor.size();
                    let scale = monitor.scale_factor().max(1.0);
                    let logical_w = size.width as f64 / scale;
                    let aspect = if size.height > 0 {
                        size.width as f64 / size.height as f64
                    } else {
                        1.0
                    };
                    if aspect >= 1.55 && (size.width >= 1920 || logical_w >= 1360.0) {
                        let _ = win.maximize();
                    }
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            core_connection,
            get_display_hardware_info,
            open_in_browser,
            open_in_webview,
            terminate_application,
            terminate_desktop,
            exit_and_run_background
        ])
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.emit("request-exit-prompt", ());
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run tauri desktop application");
}
