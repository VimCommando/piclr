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
            tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::External(url))
                .title("piclr")
                .inner_size(1200.0, 800.0)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
}
