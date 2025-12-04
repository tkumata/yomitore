// Quick test to verify badge generation from existing data
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Simulating loading stats.json
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let path = home.join(".config").join("yomitore").join("stats.json");

    if !path.exists() {
        println!("stats.json not found");
        return Ok(());
    }

    let content = std::fs::read_to_string(&path)?;
    println!("Current stats.json:");
    println!("{}", content);

    Ok(())
}
