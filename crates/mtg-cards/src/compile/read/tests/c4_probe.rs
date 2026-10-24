//! Goal: scratch probe (not committed): prints the wordings of every ability of the rules files named by `PROBE`.
use super::*;

#[test]
fn probe() {
    let Ok(files) = std::env::var("PROBE") else {
        return;
    };
    for file in files.split(',') {
        let text = std::fs::read_to_string(file).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        println!("=== {file}");
        for ab in value["faces"][0]["abilities"].as_array().unwrap() {
            let parsed: Ability = serde_json::from_value(ab.clone()).unwrap();
            println!("--- {ab}");
            for wording in written_variants(&parsed) {
                println!("   > {wording}");
            }
        }
    }
}
