use std::env;
use std::fs;
use std::io::Read;
use std::path::PathBuf;

const DATASTAR_URL: &str =
    "https://cdn.jsdelivr.net/gh/starfederation/datastar@1.0.0-RC.7/bundles/datastar.js";

fn main() {
    println!("cargo:rerun-if-env-changed=PICLR_NO_FETCH");
    println!("cargo:rerun-if-changed=assets/datastar.js");

    ensure_datastar();

    #[cfg(feature = "tauri")]
    tauri_build::build();
}

fn ensure_datastar() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let path = PathBuf::from(manifest_dir)
        .join("assets")
        .join("datastar.js");

    if path.exists() {
        return;
    }

    if env::var("PICLR_NO_FETCH").is_ok() {
        panic!("assets/datastar.js is missing and PICLR_NO_FETCH is set");
    }

    if let Some(parent) = path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            panic!("Failed to create assets directory: {err}");
        }
    }

    let response = ureq::get(DATASTAR_URL).call();
    if let Err(err) = response {
        panic!("Failed to fetch datastar.js: {err}");
    }
    let response = response.unwrap();
    let mut reader = response.into_reader();
    let mut body = Vec::new();
    reader
        .read_to_end(&mut body)
        .unwrap_or_else(|err| panic!("Failed to read datastar.js: {err}"));
    fs::write(&path, body).unwrap_or_else(|err| panic!("Failed to write datastar.js: {err}"));
}
