use std::env;
use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = env::var("OUT_DIR")?;
    let dest_path = Path::new(&out_dir).join("help_content.rs");

    let help_content = if Path::new("docs/HELP.md").exists() {
        fs::read_to_string("docs/HELP.md").unwrap_or_default()
    } else {
        String::new()
    };

    let content = format!("pub const HELP_CONTENT: &str = r#\"{help_content}\"#;");

    fs::write(dest_path, content)?;

    // Re-run build script if HELP.md changes
    println!("cargo:rerun-if-changed=docs/HELP.md");

    Ok(())
}
