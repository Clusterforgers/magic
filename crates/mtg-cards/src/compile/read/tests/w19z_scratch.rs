//! Goal: scratch.
use super::*;

#[test]
fn scratch() {
    let Ok(file) = std::fs::read_to_string("/home/chris/.cache/mtg-cli/orch/g2m/w19/scratch.txt") else {
        return;
    };
    for line in file.lines() {
        let (kind, text) = line.split_once('\t').unwrap_or(("A", line));
        let kind = match kind {
            "S" => ClauseKind::Static,
            "T" => ClauseKind::Triggered,
            "P" => ClauseKind::Spell,
            _ => ClauseKind::Activated,
        };
        let read = clause(text, kind, false);
        match read {
            None => eprintln!("NOTREAD {text}"),
            Some(read) => {
                let wordings = render_variants(&read);
                let ok = wordings.iter().any(|w| same_text(w, text));
                eprintln!("{} {text} => {:?}", if ok { "OK" } else { "DIFF" }, wordings.first());
            }
        }
    }
}
