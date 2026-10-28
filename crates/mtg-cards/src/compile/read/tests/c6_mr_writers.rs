//! Goal: placeholder.
use super::*;

#[test]
fn c6_mr_dump() {
    let Ok(path) = std::env::var("R2DD_DUMP") else {
        return;
    };
    let text = std::fs::read_to_string(path).unwrap();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        println!("== {}", serde_json::to_string(&ability(line)).unwrap());
        for wording in written_variants(&ability(line)) {
            println!("   {wording}");
        }
    }
}
