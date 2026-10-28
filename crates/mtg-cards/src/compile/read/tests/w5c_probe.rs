//! Goal: probe.
use super::*;

#[test]
fn probe() {
    let path = std::env::var("W5C_PROBE").unwrap_or_default();
    let text = std::fs::read_to_string(path).unwrap_or_default();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let wordings = written_variants(&ability(line));
        eprintln!("== {} wordings", wordings.len());
        for wording in wordings.iter().take(12) {
            eprintln!("  {wording}");
        }
    }
}
