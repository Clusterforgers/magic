//! Goal: scratch.
use super::*;

#[test]
fn w22_scratch() {
    for json in std::fs::read_to_string("/tmp/claude-1000/w22/scratch.txt")
        .unwrap_or_default()
        .lines()
    {
        let Ok(a) = serde_json::from_str::<Ability>(json) else {
            println!("PARSEFAIL {json}");
            continue;
        };
        println!("{json}\n   => {:?}", written_variants(&a));
    }
}
