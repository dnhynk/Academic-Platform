//! The runtime's exact application-command capability manifest.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "desktop-runtime")]
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["desktop_request_v1"])),
    )?;
    Ok(())
}
