//! Lists the rules files under `cards/` so that the crate can embed them (`builtin`), and rebuilds
//! the crate when one changes. It also digests the sources of the compiler and of the rules data
//! types into the compiler's version (`generated::COMPILER_VERSION`).

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

/// The sources whose change makes a compiled card out of date: the compiler (its tests aside) and
/// the types of the rules data it writes.
const VERSIONED: &[&str] = &[
    "src/compile",
    "src/ability.rs",
    "src/card.rs",
    "src/cost.rs",
    "src/effect.rs",
    "src/keyword.rs",
    "src/mana.rs",
    "src/modify.rs",
    "src/refs.rs",
    "src/trigger.rs",
    "src/value.rs",
];

fn sources(path: &Path, found: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            sources(&entry?.path(), found)?;
        }
    } else if path.extension().is_some_and(|extension| extension == "rs") {
        found.push(path.to_path_buf());
    }
    Ok(())
}

/// Whether the file only tests the compiler.
fn is_test(path: &Path) -> bool {
    path.components()
        .any(|part| part.as_os_str() == "tests" || part.as_os_str() == "tests.rs")
}

/// FNV-1a over the relative path and the text of every versioned source, in path order.
fn compiler_version(manifest: &Path) -> Result<String, Box<dyn Error>> {
    let mut files = Vec::new();
    for versioned in VERSIONED {
        println!("cargo:rerun-if-changed={versioned}");
        sources(&manifest.join(versioned), &mut files)?;
    }
    files.retain(|path| !is_test(path.strip_prefix(manifest).unwrap_or(path)));
    files.sort();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for byte in bytes.iter().chain(&[0]) {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3);
        }
    };
    for path in &files {
        feed(path.strip_prefix(manifest)?.to_string_lossy().as_bytes());
        feed(&fs::read(path)?);
    }
    Ok(format!("{hash:016x}"))
}

fn main() -> Result<(), Box<dyn Error>> {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    println!(
        "cargo:rustc-env=MTG_COMPILER_VERSION={}",
        compiler_version(&manifest)?
    );
    let root = manifest.join("cards");
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
