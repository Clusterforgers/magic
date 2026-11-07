//! Goal: r7d writers read back on-trust statics (continuous effects, rules, shields, keyword, size and ability grants), and a rule of another meaning reads `Differs`.
use super::*;

/// Prints `id<TAB>state<TAB>closest wording` for each `id<TAB>clause<TAB>ability json` line of the file in `ZZ_PROBE`.
#[test]
#[ignore = "probe"]
fn zz_probe() {
    let Ok(path) = std::env::var("ZZ_PROBE") else {
        return;
    };
    let all = std::env::var("ZZ_ALL").is_ok();
    let text = std::fs::read_to_string(path).unwrap_or_default();
    for line in text.lines() {
        let mut parts = line.splitn(3, '\t');
        let (Some(id), Some(clause), Some(json)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let Ok(read) = serde_json::from_str::<Ability>(json) else {
            println!("{id}\tbad\t");
            continue;
        };
        let wordings = written_variants(&read);
        let state = if wordings.iter().any(|w| same_text(w, clause)) {
            "written"
        } else if wordings.is_empty() {
            "none"
        } else {
            "differs"
        };
        if all {
            for w in &wordings {
                println!("{id}\t{state}\t{w}");
            }
        } else {
            println!(
                "{id}\t{state}\t{}",
                wordings.first().cloned().unwrap_or_default()
            );
        }
    }
}
