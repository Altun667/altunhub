#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(serde::Serialize)]
struct InstallResult {
    kind: String,
    path: String,
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

fn download(url: &str, out: &Path) -> Result<(), String> {
    let o = out.to_string_lossy().to_string();
    ps(
        "$ProgressPreference='SilentlyContinue'; [Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -UseBasicParsing -Uri $env:AH_URL -OutFile $env:AH_OUT",
        &[("AH_URL", url), ("AH_OUT", o.as_str())],
    )
    .map_err(|_| "İndirme başarısız".to_string())
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
    id: String,
    url: String,
    name: String,
    exe: String,
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
    download(&url, &file)?;

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
        });
    }
    Ok(InstallResult {
        kind: "portable".into(),
        path: file_s,
    })
}

/// GitHub release dosyasını indirir. Kurulum dosyasıysa çalıştırır, taşınabilir exe/zip ise AltunHub klasörüne koyar.
#[tauri::command]
async fn install_app(
    id: String,
    url: String,
    name: String,
    exe: String,
) -> Result<InstallResult, String> {
    tauri::async_runtime::spawn_blocking(move || install_blocking(id, url, name, exe))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            launch, open_url, find_app, install_app, exit_app
        ])
        .run(tauri::generate_context!())
        .expect("AltunHub başlatılamadı");
}
