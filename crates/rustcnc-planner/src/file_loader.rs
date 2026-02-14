use rustcnc_core::gcode::GCodeFile;
use tracing::info;

/// Load and parse a G-code file asynchronously
pub async fn load_gcode_file(path: &str) -> anyhow::Result<GCodeFile> {
    let path = path.to_string();
    let file = tokio::task::spawn_blocking(move || -> anyhow::Result<GCodeFile> {
        let content = std::fs::read_to_string(&path)?;
        let name = std::path::Path::new(&path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".into());

        info!("Loading G-code file: {} ({} bytes)", name, content.len());
        let file = GCodeFile::parse(name, &content);
        info!(
            "Parsed {} lines ({} non-empty)",
            content.lines().count(),
            file.total_lines
        );
        Ok(file)
    })
    .await??;

    Ok(file)
}

/// Parse G-code from raw string content (for uploaded files)
pub fn parse_gcode_content(name: String, content: &str) -> GCodeFile {
    GCodeFile::parse(name, content)
}
