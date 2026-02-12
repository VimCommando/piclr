#[cfg(feature = "tauri")]
use tauri::Manager;

#[cfg(feature = "tauri")]
pub fn launch(url: String) -> Result<(), tauri::Error> {
    let url = tauri::Url::parse(&url).map_err(|err| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid loopback url: {err}"),
        )
    })?;
    tauri::Builder::default()
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
