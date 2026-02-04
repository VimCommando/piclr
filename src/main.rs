use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use piclr::app::{AppConfig, AppContext};
use piclr::domain::{ActionMapping, AppStateMachine};
use piclr::fs::scan_images;
use piclr::web::router;

#[derive(Parser, Debug)]
#[command(name = "piclr", version, about = "Picture Left/Right")]
struct Cli {
    path: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    let config = AppConfig {
        initial_path: cli.path.clone(),
        ..Default::default()
    };

    let state = AppStateMachine::new(config.queue_mode, ActionMapping {
        left: config.action_mapping.left.clone(),
        right: config.action_mapping.right.clone(),
    }, config.sort_mode);

    let ctx = AppContext::new(state, config);

    if let Some(path) = cli.path {
        let images = scan_images(&path).await;
        let mut guard = ctx.state.write().await;
        guard.transition_to_scanning();
        guard.transition_to_ready(images, Some(path));
        guard.transition_to_viewing();
    }

    let app = router(ctx);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr: SocketAddr = listener.local_addr()?;
    let url = format!("http://{}", addr);
    tracing::info!("Listening on {}", url);

    #[cfg(feature = "tauri")]
    if std::env::var("PICLR_TAURI").is_ok() {
        let server = tokio::spawn(async move { axum::serve(listener, app).await });
        let _ = piclr::tauri_shell::launch(url);
        let _ = server.await;
        return Ok(());
    }

    axum::serve(listener, app).await?;

    Ok(())
}
