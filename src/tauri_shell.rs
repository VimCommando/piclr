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
pub fn launch(url: String) -> Result<(), tauri::Error> {
    let url = tauri::Url::parse(&url).map_err(|err| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid loopback url: {err}"),
        )
    })?;
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![pick_directory])
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
