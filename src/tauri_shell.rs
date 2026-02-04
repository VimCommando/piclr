#[cfg(feature = "tauri")]
pub fn launch(url: String) -> Result<(), tauri::Error> {
    let url = url
        .parse()
        .map_err(|err| tauri::Error::Runtime(err.to_string()))?;
    tauri::Builder::default()
        .setup(move |app| {
            tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::External(url))
                .title("piclr")
                .inner_size(1200.0, 800.0)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
}
