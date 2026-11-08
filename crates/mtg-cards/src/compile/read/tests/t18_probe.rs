//! Goal: probe.
use super::*;

#[test]
fn probe() {
    let Ok(path) = std::env::var("T18_FILE") else {
        return;
    };
    let lines = std::fs::read_to_string(path).unwrap_or_default();
    for json in lines.lines().filter(|line| !line.is_empty()) {
        println!("J: {json}");
        for text in written_variants(&ability(json)).iter().take(3) {
            println!("W: {text}");
        }
    }
}
