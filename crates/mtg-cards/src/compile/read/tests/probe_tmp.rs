//! Goal: temporary probe, not committed.
use super::*;

#[test]
fn probe() {
    let Ok(path) = std::env::var("PROBE_FILE") else {
        return;
    };
    let text = std::fs::read_to_string(path).unwrap();
    for line in text.lines() {
        let Some((name, rest)) = line.split_once('\t') else {
            continue;
        };
        for part in rest.split(" || ") {
            let first = name.split([',', ' ']).next().unwrap_or(name);
            let t = part.replace(name, "~").replace(first, "~");
            let ok = clause(&t, ClauseKind::Static, false).is_some();
            println!("PROBE {} | {name} | {t}", if ok { "OK  " } else { "FAIL" });
        }
    }
}
