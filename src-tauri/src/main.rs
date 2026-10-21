#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};

static CLOSE_TO_TRAY: AtomicBool = AtomicBool::new(true);

#[derive(serde::Serialize)]
struct InstallResult {
    kind: String,
    path: String,
    verified: String,
}

#[derive(serde::Serialize, Clone)]
struct Progress {
    id: String,
    done: u64,
    total: u64,
    speed: f64,
}

/// Dosyayı / kısayolu / kurulum dosyasını Windows'un kendi açıcısıyla açar (UAC dahil çalışır).
fn shell_open(target: &str) -> Result<(), String> {
    #[cfg(windows)]
    Command::new("explorer")
        .arg(target)
        .spawn()
        .map_err(|e| e.to_string())?;
    #[cfg(not(windows))]
    Command::new("xdg-open")
        .arg(target)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn hide(c: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x08000000);
    }
    c
}

fn ps(script: &str, envs: &[(&str, &str)]) -> Result<(), String> {
    let mut c = Command::new("powershell");
    c.args(["-NoProfile", "-NonInteractive", "-Command", script]);
    for (k, v) in envs {
        c.env(k, v);
    }
    let st = hide(&mut c).status().map_err(|e| e.to_string())?;
    if st.success() {
        Ok(())
    } else {
        Err("İşlem başarısız oldu".into())
    }
}

/// Dosyayı indirir, ilerlemeyi "dl-progress" olayıyla bildirir, SHA256 özetini döndürür.
fn download(app: &AppHandle, id: &str, url: &str, out: &Path) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("AltunHub")
        .build()
        .map_err(|e| e.to_string())?;
    let mut resp = client
        .get(url)
        .send()
        .map_err(|_| "Bağlantı kurulamadı".to_string())?
        .error_for_status()
        .map_err(|_| "Sunucu hata verdi".to_string())?;
    let total = resp.content_length().unwrap_or(0);
    let mut file = std::fs::File::create(out).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    let mut done: u64 = 0;
    let mut last = Instant::now();
    let mut last_done: u64 = 0;
    loop {
        let n = resp
            .read(&mut buf)
            .map_err(|_| "İndirme kesildi".to_string())?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        hasher.update(&buf[..n]);
        done += n as u64;
        let el = last.elapsed();
        if el >= Duration::from_millis(250) {
            let speed = (done - last_done) as f64 / el.as_secs_f64();
            let _ = app.emit(
                "dl-progress",
                Progress {
                    id: id.to_string(),
                    done,
                    total,
                    speed,
                },
            );
            last = Instant::now();
            last_done = done;
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    let _ = app.emit(
        "dl-progress",
        Progress {
            id: id.to_string(),
            done,
            total: total.max(done),
            speed: 0.0,
        },
    );
    Ok(format!("{:x}", hasher.finalize()))
}

fn find_exe(dir: &Path, name: &str, depth: u8) -> Option<PathBuf> {
    let rd = std::fs::read_dir(dir).ok()?;
    let mut subs = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            subs.push(p);
        } else if p
            .file_name()
            .map(|n| n.to_string_lossy().eq_ignore_ascii_case(name))
            .unwrap_or(false)
        {
            return Some(p);
        }
    }
    if depth > 0 {
        for s in subs {
            if let Some(f) = find_exe(&s, name, depth - 1) {
                return Some(f);
            }
        }
    }
    None
}

fn find_any_exe(dir: &Path, depth: u8) -> Option<PathBuf> {
    let rd = std::fs::read_dir(dir).ok()?;
    let mut subs = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            subs.push(p);
        } else if p
            .extension()
            .map(|x| x.to_string_lossy().eq_ignore_ascii_case("exe"))
            .unwrap_or(false)
        {
            let n = p
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if !n.contains("unins") {
                return Some(p);
            }
        }
    }
    if depth > 0 {
        for s in subs {
            if let Some(f) = find_any_exe(&s, depth - 1) {
                return Some(f);
            }
        }
    }
    None
}

fn expand_env(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('%') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        if let Some(j) = after.find('%') {
            out.push_str(&std::env::var(&after[..j]).unwrap_or_default());
            rest = &after[j + 1..];
        } else {
            out.push_str(&rest[i..]);
            rest = "";
            break;
        }
    }
    out.push_str(rest);
    out
}

/// Bir uygulamayı / oyunu / kısayolu başlatır.
#[tauri::command]
fn launch(path: String) -> Result<(), String> {
    if !Path::new(&path).exists() {
        return Err("Dosya bulunamadı, yolu düzenle (✎)".into());
    }
    shell_open(&path)
}

/// Linkleri varsayılan tarayıcıda açar.
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

/// Verilen yolların (ortam değişkenleri açılarak) ilk var olanını döndürür.
#[tauri::command]
fn find_app(paths: Vec<String>) -> Option<String> {
    paths
        .iter()
        .map(|p| expand_env(p))
        .find(|p| !p.is_empty() && Path::new(p).exists())
}

fn install_blocking(
    app: AppHandle,
    id: String,
    url: String,
    name: String,
    exe: String,
    digest: String,
) -> Result<InstallResult, String> {
    if !url.starts_with("https://github.com/Altun667/") {
        return Err("Geçersiz indirme adresi".into());
    }
    let ok = |c: char| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-';
    let sid: String = id.chars().filter(|c| ok(*c)).collect();
    let sname: String = name.chars().filter(|c| ok(*c)).collect();
    if sid.is_empty() || sname.is_empty() {
        return Err("Geçersiz ad".into());
    }
    let local = std::env::var("LOCALAPPDATA").map_err(|_| "LOCALAPPDATA bulunamadı".to_string())?;
    let dir: PathBuf = Path::new(&local).join("AltunHub").join("apps").join(&sid);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join(&sname);

    let got = download(&app, &sid, &url, &file)?;
    let mut verified = "none".to_string();
    let want = digest.trim().to_lowercase();
    let want = want.strip_prefix("sha256:").unwrap_or(&want).to_string();
    if !want.is_empty() {
        if want == got {
            verified = "ok".to_string();
        } else {
            let _ = std::fs::remove_file(&file);
            return Err("Dosya doğrulanamadı (SHA256 uyuşmuyor)".into());
        }
    }

    let lower = sname.to_lowercase();
    let file_s = file.to_string_lossy().to_string();

    if lower.ends_with(".zip") {
        let dir_s = dir.to_string_lossy().to_string();
        ps(
            "Expand-Archive -LiteralPath $env:AH_ZIP -DestinationPath $env:AH_DIR -Force",
            &[("AH_ZIP", file_s.as_str()), ("AH_DIR", dir_s.as_str())],
        )
        .map_err(|_| "Zip açılamadı".to_string())?;
        let found = find_exe(&dir, &exe, 3)
            .or_else(|| find_any_exe(&dir, 3))
            .ok_or_else(|| "Zip içinde exe bulunamadı".to_string())?;
        return Ok(InstallResult {
            kind: "portable".into(),
            path: found.to_string_lossy().to_string(),
            verified,
        });
    }
    if lower.ends_with(".msi")
        || lower.contains("setup")
        || lower.contains("install")
        || lower.contains("kurulum")
    {
        shell_open(&file_s)?;
        return Ok(InstallResult {
            kind: "installer".into(),
            path: String::new(),
            verified,
        });
    }
    Ok(InstallResult {
        kind: "portable".into(),
        path: file_s,
        verified,
    })
}

/// GitHub release dosyasını indirir (ilerleme + SHA256 doğrulama), kurulum dosyasıysa çalıştırır.
#[tauri::command]
async fn install_app(
    app: AppHandle,
    id: String,
    url: String,
    name: String,
    exe: String,
    digest: String,
) -> Result<InstallResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        install_blocking(app, id, url, name, exe, digest)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn exit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn set_close_to_tray(on: bool) {
    CLOSE_TO_TRAY.store(on, Ordering::Relaxed);
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .setup(|app| {
            let show = MenuItem::with_id(app, "show", "AltunHub", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Çıkış / Exit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("AltunHub")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main(app),
                    "quit" => app.exit(0),
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            if std::env::args().any(|a| a == "--minimized") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if CLOSE_TO_TRAY.load(Ordering::Relaxed) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            launch,
            open_url,
            find_app,
            install_app,
            exit_app,
            set_close_to_tray
        ])
        .run(tauri::generate_context!())
        .expect("AltunHub başlatılamadı");
}
