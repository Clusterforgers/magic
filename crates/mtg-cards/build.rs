//! Lists the rules files under `cards/` so that the crate can embed them (`builtin`), and rebuilds
//! the crate when one changes.

use std::env;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

fn collect(dir: &Path, found: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, found)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            found.push(path);
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let root = Path::new(&env::var("CARGO_MANIFEST_DIR")?).join("cards");
    println!("cargo:rerun-if-changed=cards");
    let mut files = Vec::new();
    collect(&root, &mut files)?;
    files.sort();
    let mut list = String::from("&[\n");
    for path in &files {
        let relative = path
            .strip_prefix(&root)?
            .to_string_lossy()
            .replace('\\', "/");
        let shown = format!("cards/{relative}");
        let absolute = path.to_string_lossy();
        writeln!(list, "    ({shown:?}, include_str!({absolute:?})),")?;
    }
    list.push_str("]\n");
    fs::write(
        Path::new(&env::var("OUT_DIR")?).join("builtin_files.rs"),
        list,
    )?;
    Ok(())
}
