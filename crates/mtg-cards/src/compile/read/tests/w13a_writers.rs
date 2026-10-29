//! Goal: the writers of the shapes the stored local-model proposals hit (r4d-b, r4b-b) give the oracle wording of the ability, and a changed clause does not read back.
use super::*;

#[test]
#[ignore = "exploration dump"]
fn w13a_dump() {
    let list = std::fs::read_to_string(std::env::var("W13A_LIST").unwrap_or_default()).unwrap_or_default();
    for path in list.split_whitespace() {
        let full = if path.starts_with('/') { path.to_owned() } else { format!("../../{path}") };
        let text = std::fs::read_to_string(full).unwrap_or_default();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        println!("== {path}");
        for raw in value["faces"][0]["abilities"].as_array().into_iter().flatten() {
            let Ok(ability) = serde_json::from_value::<Ability>(raw.clone()) else {
                println!("  (no ability) {raw}");
                continue;
            };
            println!("  {raw}");
            for text in written_variants(&ability).iter().take(6) {
                println!("    > {text}");
            }
        }
    }
}

#[test]
#[ignore = "exploration dump"]
fn w13a_read_dump() {
    let list = std::fs::read_to_string(std::env::var("W13A_TEXTS").unwrap_or_default()).unwrap_or_default();
    for text in list.lines() {
        println!("== {text}");
        for kind in [ClauseKind::Spell, ClauseKind::Triggered, ClauseKind::Activated, ClauseKind::Static] {
            if let Some(read) = clause(text, kind, false) {
                println!("  {}", serde_json::to_string(&read).unwrap_or_default());
            }
        }
    }
}
