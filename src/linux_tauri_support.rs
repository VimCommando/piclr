#[cfg(target_os = "linux")]
use std::path::Path;
#[cfg(target_os = "linux")]
use std::process::Command;

const WEBKIT_LIBS: [&str; 2] = ["libwebkit2gtk-4.1.so.0", "libwebkit2gtk-4.0.so.37"];
const GTK3_LIB: &str = "libgtk-3.so.0";
#[cfg(target_os = "linux")]
const LIB_SEARCH_DIRS: [&str; 10] = [
    "/lib",
    "/usr/lib",
    "/lib64",
    "/usr/lib64",
    "/usr/local/lib",
    "/lib/x86_64-linux-gnu",
    "/usr/lib/x86_64-linux-gnu",
    "/lib/aarch64-linux-gnu",
    "/usr/lib/aarch64-linux-gnu",
    "/usr/lib/arm-linux-gnueabihf",
];

pub fn validate_linux_tauri_prerequisites(
    display: Option<&str>,
    wayland_display: Option<&str>,
    ldconfig_cache: Option<&str>,
) -> Result<(), Vec<&'static str>> {
    let mut missing = Vec::new();

    if display.is_none() && wayland_display.is_none() {
        missing.push("display-session");
    }

    if let Some(cache) = ldconfig_cache {
        if !WEBKIT_LIBS.iter().any(|lib| cache.contains(lib)) {
            missing.push("webkit2gtk");
        }
        if !cache.contains(GTK3_LIB) {
            missing.push("gtk3");
        }
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}

pub fn linux_tauri_prereq_error(missing: &[&str]) -> String {
    let mut reasons = Vec::new();
    if missing.contains(&"display-session") {
        reasons.push("No X11/Wayland session detected (DISPLAY/WAYLAND_DISPLAY are unset)");
    }
    if missing.contains(&"webkit2gtk") {
        reasons.push("WebKitGTK runtime libraries were not detected");
    }
    if missing.contains(&"gtk3") {
        reasons.push("GTK3 runtime libraries were not detected");
    }

    format!(
        "Linux desktop prerequisites are missing:\n- {}\n\nInstall/verify dependencies:\n- Debian-family: sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0\n- Fedora-family: sudo dnf install webkit2gtk4.1 gtk3\n- Arch-family: sudo pacman -S webkit2gtk gtk3\n\nThen re-run: cargo run --features tauri -- <image-dir>\nIf you are in a headless shell, start an X11 or Wayland session first.\nSee README: Linux Tauri Desktop Support.",
        reasons.join("\n- ")
    )
}

#[cfg(target_os = "linux")]
pub fn check_linux_tauri_prerequisites() -> Result<(), String> {
    let display = std::env::var("DISPLAY").ok();
    let wayland_display = std::env::var("WAYLAND_DISPLAY").ok();

    let ldconfig_cache = Command::new("ldconfig")
        .arg("-p")
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                Some(String::from_utf8_lossy(&out.stdout).into_owned())
            } else {
                None
            }
        });

    let mut missing = validate_linux_tauri_prerequisites(
        display.as_deref(),
        wayland_display.as_deref(),
        ldconfig_cache.as_deref(),
    )
    .err()
    .unwrap_or_default();

    // Fallback when ldconfig is unavailable: probe common library paths directly.
    if ldconfig_cache.is_none() {
        if !library_exists_in_paths(&WEBKIT_LIBS, &LIB_SEARCH_DIRS)
            && !missing.contains(&"webkit2gtk")
        {
            missing.push("webkit2gtk");
        }
        if !library_exists_in_paths(&[GTK3_LIB], &LIB_SEARCH_DIRS) && !missing.contains(&"gtk3") {
            missing.push("gtk3");
        }
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(linux_tauri_prereq_error(&missing))
    }
}

#[cfg(not(target_os = "linux"))]
pub fn check_linux_tauri_prerequisites() -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "linux")]
fn library_exists_in_paths(lib_names: &[&str], search_dirs: &[&str]) -> bool {
    search_dirs.iter().any(|dir| {
        lib_names
            .iter()
            .any(|name| Path::new(dir).join(name).exists())
    })
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    use super::library_exists_in_paths;
    use super::{
        GTK3_LIB, WEBKIT_LIBS, linux_tauri_prereq_error, validate_linux_tauri_prerequisites,
    };
    #[cfg(target_os = "linux")]
    use std::fs;
    #[cfg(target_os = "linux")]
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn validates_when_display_and_libs_are_present() {
        let cache = format!("{} {}", WEBKIT_LIBS[0], GTK3_LIB);
        let result = validate_linux_tauri_prerequisites(Some(":0"), None, Some(&cache));
        assert!(result.is_ok());
    }

    #[test]
    fn reports_missing_display_session() {
        let cache = format!("{} {}", WEBKIT_LIBS[0], GTK3_LIB);
        let result = validate_linux_tauri_prerequisites(None, None, Some(&cache));
        assert_eq!(result.unwrap_err(), vec!["display-session"]);
    }

    #[test]
    fn reports_missing_linux_shared_libs() {
        let result = validate_linux_tauri_prerequisites(Some(":0"), None, Some("noise"));
        assert_eq!(result.unwrap_err(), vec!["webkit2gtk", "gtk3"]);
    }

    #[test]
    fn error_message_contains_distro_remediation() {
        let message = linux_tauri_prereq_error(&["display-session", "webkit2gtk", "gtk3"]);
        assert!(message.contains("Debian-family"));
        assert!(message.contains("Fedora-family"));
        assert!(message.contains("Arch-family"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn library_path_probe_detects_existing_files() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("piclr-lib-probe-{unique}"));
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        let lib_name = "libfake-test.so.0";
        let lib_path = temp_dir.join(lib_name);
        fs::write(&lib_path, "").expect("write fake shared object");

        let temp_dir_str = temp_dir.to_string_lossy();
        assert!(library_exists_in_paths(
            &[lib_name],
            &[temp_dir_str.as_ref()]
        ));

        let _ = fs::remove_file(lib_path);
        let _ = fs::remove_dir_all(temp_dir);
    }
}
