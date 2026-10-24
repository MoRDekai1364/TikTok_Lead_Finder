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

fn resolve_default_browser() -> Option<String> {
    if cfg!(target_os = "windows") {
        let subkeys = [
            r"HKCU\Software\Microsoft\Windows\Shell\Associations\UrlAssociations\https\UserChoice",
            r"HKCU\Software\Microsoft\Windows\Shell\Associations\UrlAssociations\http\UserChoice",
        ];
        for sk in subkeys {
            if let Ok(out) = std::process::Command::new("reg")
                .args(["query", sk, "/v", "ProgId"])
                .output()
            {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout);
                    for line in s.lines() {
                        if line.contains("ProgId") {
                            let prog_id = line.split_whitespace().last().unwrap_or("");
                            if !prog_id.is_empty() {
                                let subkey = format!(r"HKCR\{}\shell\open\command", prog_id);
                                if let Ok(c_out) = std::process::Command::new("reg")
                                    .args(["query", &subkey, "/ve"])
                                    .output()
                                {
                                    if c_out.status.success() {
                                        let cs = String::from_utf8_lossy(&c_out.stdout);
                                        for cline in cs.lines() {
                                            if cline.contains("REG_SZ") {
                                                if let Some(cmd) = cline.split("REG_SZ").nth(1) {
                                                    let trimmed = cmd.trim();
                                                    let exe = if trimmed.starts_with('"') {
                                                        trimmed.split('"').nth(1).unwrap_or("").to_string()
                                                    } else {
                                                        trimmed.split_whitespace().next().unwrap_or("").to_string()
                                                    };
                                                    if !exe.is_empty() && std::path::Path::new(&exe).exists() {
                                                        return Some(exe);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        let generic_keys = [
            r"HKCR\https\shell\open\command",
            r"HKCR\http\shell\open\command",
        ];
        for gk in generic_keys {
            if let Ok(c_out) = std::process::Command::new("reg")
                .args(["query", gk, "/ve"])
                .output()
            {
                if c_out.status.success() {
                    let cs = String::from_utf8_lossy(&c_out.stdout);
                    for cline in cs.lines() {
                        if cline.contains("REG_SZ") {
                            if let Some(cmd) = cline.split("REG_SZ").nth(1) {
                                let trimmed = cmd.trim();
                                let exe = if trimmed.starts_with('"') {
                                    trimmed.split('"').nth(1).unwrap_or("").to_string()
                                } else {
                                    trimmed.split_whitespace().next().unwrap_or("").to_string()
                                };
                                if !exe.is_empty() && std::path::Path::new(&exe).exists() {
                                    return Some(exe);
                                }
                            }
                        }
                    }
                }
            }
        }
    } else if cfg!(target_os = "linux") {
        if let Ok(out) = std::process::Command::new("xdg-settings")
            .args(["get", "default-web-browser"])
            .output()
        {
            if out.status.success() {
                let desktop = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let bin_name = desktop.replace(".desktop", "");
                if let Ok(w_out) = std::process::Command::new("which").arg(&bin_name).output() {
                    if w_out.status.success() {
                        let path = String::from_utf8_lossy(&w_out.stdout).trim().to_string();
                        if !path.is_empty() && std::path::Path::new(&path).exists() {
                            return Some(path);
                        }
                    }
                }
            }
        }
    }
    None
}

#[tauri::command]
fn open_in_browser(url: String) -> Result<(), String> {
    if let Some(exe) = resolve_default_browser() {
        let _ = std::process::Command::new(exe).arg(&url).spawn();
        return Ok(());
    }
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
        .invoke_handler(tauri::generate_handler![core_connection, open_in_browser, open_in_webview])
        .run(tauri::generate_context!())
        .expect("failed to run tauri desktop application");
}
