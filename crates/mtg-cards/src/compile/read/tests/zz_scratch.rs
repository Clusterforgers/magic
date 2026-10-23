//! Goal: scratch.
use super::*;

#[test]
fn dump() {
    let Ok(path) = std::env::var("R2A_JSON") else {
        return;
    };
    for line in std::fs::read_to_string(path).unwrap().lines() {
        if line.trim().is_empty() {
            continue;
        }
        let parsed: Result<Ability, _> = serde_json::from_str(line);
        match parsed {
            Ok(a) => {
                println!("JSON: {line}");
                for v in render_variants(&a) {
                    println!("  => {v}");
                }
            }
            Err(e) => println!("BAD {e}: {line}"),
        }
    }
}
