//! Goal: a conditional step reads back as the oracle words it ("If X, Y.", "Y if X.", "Otherwise, Z.", "If you can't, Z.", "Then if X, Y."); another condition, number, kind, player or branch never does.
use super::*;

/// Prints the written wordings of the abilities in the file `W4A_DUMP` names (one JSON per line after a tab).
#[test]
#[ignore = "a tool: prints the wordings of abilities"]
fn w4a_dump() {
    let Ok(path) = std::env::var("W4A_DUMP") else {
        return;
    };
    let text = std::fs::read_to_string(path).unwrap_or_default();
    for line in text.lines() {
        let json = line.split('\t').next_back().unwrap_or_default();
        println!("== {}", line.split('\t').next().unwrap_or_default());
        let mut wordings = written_variants(&ability(json));
        wordings.retain(|w| !w.contains("raw 1") && !w.contains("raw 4"));
        for wording in wordings.iter().take(6) {
            println!("   {wording}");
        }
    }
}
