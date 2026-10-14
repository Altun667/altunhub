#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::Command;

/// Bir uygulamayı / oyunu / kısayolu başlatır.
#[tauri::command]
fn launch(path: String) -> Result<(), String> {
    if !std::path::Path::new(&path).exists() {
        return Err("Dosya bulunamadı, yolu düzenle (✎)".into());
    }
    #[cfg(windows)]
    Command::new("explorer")
        .arg(&path)
        .spawn()
        .map_err(|e| e.to_string())?;
    #[cfg(not(windows))]
    Command::new("xdg-open")
        .arg(&path)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Sosyal medya linklerini varsayılan tarayıcıda açar.
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Geçersiz link".into());
    }
    #[cfg(windows)]
    Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", &url])
        .spawn()
        .map_err(|e| e.to_string())?;
    #[cfg(not(windows))]
    Command::new("xdg-open")
        .arg(&url)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![launch, open_url])
        .run(tauri::generate_context!())
        .expect("AltunHub başlatılamadı");
}
