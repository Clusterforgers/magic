//! Goal: temporary probe.
use super::*;

#[test]
fn probe() {
    let Ok(path) = std::env::var("PROBE_FILE") else {
        return;
    };
    let text = std::fs::read_to_string(path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    for (name, abilities) in value.as_object().unwrap() {
        for json in abilities.as_array().unwrap() {
            let a: Ability = match serde_json::from_value(json.clone()) {
                Ok(a) => a,
                Err(e) => {
                    println!("## {name}: invalid {e}");
                    continue;
                }
            };
            let texts = written_variants(&a);
            println!("## {name}: {} wordings", texts.len());
            for t in texts.iter().take(6) {
                println!("   {t}");
            }
        }
    }
}
