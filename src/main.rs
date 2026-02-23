use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use piclr::app::{AppConfig, AppContext};
use piclr::domain::{ActionMapping, AppState};
use piclr::fs::{scan_directories, scan_images};
use piclr::web::router;

#[derive(Parser, Debug)]
#[command(name = "piclr", version, about = "Picture Left/Right")]
struct Cli {
    path: Option<PathBuf>,
    #[arg(long)]
    port: Option<u16>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();
    let Cli { path, port } = cli;

    let launch_root = if let Some(path) = path {
        path
    } else {
        std::env::current_dir()?
    };
    let config = AppConfig {
        initial_path: Some(launch_root.clone()),
        ..Default::default()
    };

    let state = AppState::new(
        config.queue_mode,
        ActionMapping {
            left: config.action_mapping.left.clone(),
            right: config.action_mapping.right.clone(),
        },
        config.sort_mode,
    );

    let ctx = AppContext::new(state, config);

    let images = scan_images(&launch_root).await;
    let directories = scan_directories(&launch_root, &launch_root).await;
    let mut guard = ctx.state.write().await;
    guard.transition_to_scanning();
    guard.transition_to_ready(Vec::new(), Some(launch_root.clone()));
    guard.transition_to_viewing();
    guard.state_mut().set_directory_snapshot(
        images,
        directories,
        Some(launch_root.clone()),
        Some(launch_root),
    );
    drop(guard);

    let app = router(ctx);
    let bind_port = port.unwrap_or(0);
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{bind_port}")).await?;
    let addr: SocketAddr = listener.local_addr()?;
    let url = format!("http://{}", addr);
    tracing::info!("Listening on {}", url);

    #[cfg(feature = "tauri")]
    {
        piclr::linux_tauri_support::check_linux_tauri_prerequisites()
            .map_err(std::io::Error::other)?;
        let server = tokio::spawn(async move { axum::serve(listener, app).await });
        piclr::tauri_shell::launch(url)?;
        server.abort();
        return Ok(());
    }

    #[cfg(not(feature = "tauri"))]
    {
        axum::serve(listener, app).await?;
        Ok(())
    }
}
