//! Goal: probe.
use super::*;

#[test]
fn w13b_probe2() {
    let a = ability(r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Land"},{"ControlledBy":"You"}],"to":"Battlefield"}},"effects":[{"May":{"effects":[{"PutCounters":{"on":"You","counter":{"Other":"rad"},"count":2}}]}}]}}"#);
    eprintln!("{:?}", render_variants(&a));
}

#[test]
fn w13b_probe() {
    for (text, kind) in [
        ("Whenever one or more nonland cards are milled, put a +1/+1 counter on each of up to X target creatures, where X is the number of nonland cards milled this way.", ClauseKind::Triggered),
        ("Whenever one or more nonland cards are milled, you may create that many 2/2 black Zombie Mutant creature tokens. Do this only once each turn.", ClauseKind::Triggered),
        ("Whenever one or more nonland cards are milled, you may create that many 2/2 black Zombie Mutant creature tokens.", ClauseKind::Triggered),
        ("Hive Mind — Whenever a player mills one or more creature cards, you create a 1/1 black Horror creature token.", ClauseKind::Triggered),
        ("Whenever a player mills one or more creature cards, you create a 1/1 black Horror creature token.", ClauseKind::Triggered),
        ("{1}, {T}: Target player mills three cards.", ClauseKind::Activated),
        ("Landfall — Whenever a land you control enters, you may get two rad counters.", ClauseKind::Triggered),
        ("{T}: Put target land card in a graveyard that was milled this turn onto the battlefield tapped under your control.", ClauseKind::Activated),
        ("You may have this land enter tapped. If you do, you get two rad counters.", ClauseKind::Static),
        ("{5}, {T}: Draw a card. This ability costs {1} less to activate for each rad counter you have.", ClauseKind::Activated),
        ("{5}, {T}: Draw a card.", ClauseKind::Activated),
        ("Target player loses all rad counters. Sacrifice this artifact.", ClauseKind::Spell),
        ("Target player loses all rad counters.", ClauseKind::Spell),
    ] {
        let read = clause(text, kind, false);
        eprintln!("{text}\n  => {}\n", read.map(|a| serde_json::to_string(&a).unwrap_or_default()).unwrap_or_else(|| "NONE".into()));
    }
}
