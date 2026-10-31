//! Lists the draft files under `drafts/` (`<slug>.json`, not their records in `drafts/fixtures/`) so that the crate can
//! embed them (`drafts::embedded`), and rebuilds the crate when one changes.

use std::env;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("drafts");
    println!("cargo:rerun-if-changed=drafts");
    let mut files: Vec<PathBuf> = Vec::new();
    if root.is_dir() {
        for entry in fs::read_dir(&root)? {
            let path = entry?.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
                files.push(path);
            }
        }
    }
    files.sort();
    let mut list = String::from("&[\n");
    for path in &files {
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or("a draft file has no name")?;
        let absolute = path.to_string_lossy();
        writeln!(list, "    ({stem:?}, include_str!({absolute:?})),")?;
    }
    list.push_str("]\n");
    fs::write(
        Path::new(&env::var("OUT_DIR")?).join("draft_files.rs"),
        list,
    )?;
    Ok(())
}
