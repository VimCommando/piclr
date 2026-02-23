#[cfg(feature = "tauri")]
use rfd::FileDialog;
#[cfg(feature = "tauri")]
use tauri::Manager;

#[cfg(feature = "tauri")]
#[tauri::command]
fn pick_directory() -> Option<String> {
    FileDialog::new()
        .pick_folder()
        .map(|path| path.display().to_string())
}

#[cfg(feature = "tauri")]
#[tauri::command]
fn window_minimize(window: tauri::WebviewWindow) -> Result<(), String> {
    window.minimize().map_err(|err| err.to_string())
}

#[cfg(feature = "tauri")]
#[tauri::command]
fn window_toggle_maximize(window: tauri::WebviewWindow) -> Result<(), String> {
    if window.is_maximized().map_err(|err| err.to_string())? {
        window.unmaximize().map_err(|err| err.to_string())
    } else {
        window.maximize().map_err(|err| err.to_string())
    }
}

#[cfg(feature = "tauri")]
#[tauri::command]
fn window_close(window: tauri::WebviewWindow) -> Result<(), String> {
    window.close().map_err(|err| err.to_string())
}

#[cfg(feature = "tauri")]
#[tauri::command]
fn window_start_dragging(window: tauri::WebviewWindow) -> Result<(), String> {
    window.start_dragging().map_err(|err| err.to_string())
}

#[cfg(feature = "tauri")]
pub fn launch(url: String) -> Result<(), tauri::Error> {
    let url = tauri::Url::parse(&url).map_err(|err| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid loopback url: {err}"),
        )
    })?;
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            pick_directory,
            window_minimize,
            window_toggle_maximize,
            window_close,
            window_start_dragging
        ])
        .setup(move |app| {
            let window = app.get_webview_window("main").ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "missing main webview window")
            })?;
            window.navigate(url)?;
            window.set_focus()?;
            Ok(())
        })
        .run(tauri::generate_context!())
}
