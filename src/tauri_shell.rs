#[cfg(feature = "tauri")]
use rfd::FileDialog;
#[cfg(feature = "tauri")]
use tauri::Manager;
#[cfg(all(feature = "tauri", target_os = "macos"))]
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};

#[cfg(all(feature = "tauri", target_os = "macos"))]
const MENU_OPEN_LOCATION_ID: &str = "open-location";

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

    let mut builder = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            pick_directory,
            window_minimize,
            window_toggle_maximize,
            window_close,
            window_start_dragging
        ]);

    #[cfg(target_os = "macos")]
    {
        builder = builder
            .menu(|app| {
                let menu = Menu::default(app)?;
                let open_location =
                    MenuItem::with_id(app, MENU_OPEN_LOCATION_ID, "Open Location", true, Some("CmdOrCtrl+O"))?;
                for item in menu.items()? {
                    let Some(submenu) = item.as_submenu() else {
                        continue;
                    };
                    if submenu.text()? == "File" {
                        let separator = PredefinedMenuItem::separator(app)?;
                        submenu.prepend_items(&[&open_location, &separator])?;
                        break;
                    }
                }
                Ok(menu)
            })
            .on_menu_event(|app, event| {
                if event.id() != MENU_OPEN_LOCATION_ID {
                    return;
                }
                let Some(window) = app.get_webview_window("main") else {
                    tracing::warn!("open location menu action ignored: missing main window");
                    return;
                };
                if let Err(err) = window.eval("window.piclrSelectRootDirectory?.();") {
                    tracing::warn!("failed to run Open Location menu action: {err}");
                }
            });
    }

    builder
        .setup(move |app| {
            let window = app.get_webview_window("main").ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "missing main webview window")
            })?;
            if let Err(err) = window.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)))
            {
                tracing::warn!("failed to set transparent window background: {err}");
            }
            window.navigate(url)?;
            window.set_focus()?;
            Ok(())
        })
        .run(tauri::generate_context!())
}
