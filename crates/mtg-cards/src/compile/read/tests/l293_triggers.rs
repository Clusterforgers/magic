//! Goal: the l29-3 trigger wordings (a clash and its result, keyword actions of a player, a target an opponent chooses,
//! the first discard of a turn) read into the DSL and write back as the card's own text; near misses stay with the table.
use super::*;

/// Prints what each clause of `L293_PROBE` (separated by `|`) reads as and how it is written back; a tool.
#[test]
fn l293_probe() {
    let list = std::env::var("L293_PROBE").unwrap_or_default();
    for text in list.split('|').filter(|text| !text.is_empty()) {
        let read = clause(text, ClauseKind::Triggered, false);
        println!(
            "== {text}\n  read: {}",
            read.as_ref()
                .map(|a| serde_json::to_string(a).unwrap_or_default())
                .unwrap_or_default()
        );
        if let Some(read) = read {
            for written in written_variants(&read) {
                println!("  wrote: {written}");
            }
        }
    }
    if let Ok(json) = std::env::var("L293_JSON") {
        for one in json.split('|') {
            println!("== {one}");
            for written in written_variants(&ability(one)) {
                println!("  wrote: {written}");
            }
        }
    }
}

const L293_WON: &str = r#"{"Compare":{"left":"Triggering","op":"Ge","right":1}}"#;

#[rstest]
#[case::any_clash(
    "Whenever you clash, draw a card.",
    r#"{"Triggered":{"when":{"Did":{"action":"Clashed","who":"You"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::an_opponent_clashes(
    "Whenever an opponent clashes, draw a card.",
    r#"{"Triggered":{"when":{"Did":{"action":"Clashed","who":"Opponents"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::won_draws(
    "Whenever you clash, tap target creature an opponent controls. If you won, draw a card.",
    &format!(r#"{{"Triggered":{{"when":{{"Did":{{"action":"Clashed","who":"You"}}}},"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},{{"ControlledBy":"Opponents"}}]}}}}],"effects":[{{"Tap":{{"what":{{"Target":0}}}}}},{{"If":{{"condition":{L293_WON},"then":[{{"Draw":{{}}}}]}}}}]}}}}"#)
)]
#[case::first_discards(
    "Whenever you discard one or more cards for the first time each turn, draw that many cards.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"Batch":{"Discarded":{"by":"You"}}},"nth":1}},"effects":[{"Draw":{"count":"Triggering"}}]}}"#
)]
#[case::first_nonland_discards(
    "Whenever you discard one or more nonland cards for the first time each turn, create a Junk token.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"Batch":{"Discarded":{"what":[{"Not":{"Type":"Land"}}],"by":"You"}}},"nth":1}},"effects":[{"CreateToken":{"token":{"Named":"junk"}}}]}}"#
)]
#[case::nonland_discards(
    "Whenever you discard one or more nonland cards, draw a card.",
    r#"{"Triggered":{"when":{"Batch":{"Discarded":{"what":[{"Not":{"Type":"Land"}}],"by":"You"}}},"effects":[{"Draw":{}}]}}"#
)]
fn l293_wordings_read_and_write_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::second_time(
    "Whenever you discard one or more cards for the second time each turn, draw a card."
)]
#[case::win_not_won("Whenever you clash, draw a card. If you win, draw a card.")]
#[case::noncreature("Whenever you discard one or more noncreature cards, draw a card.")]
fn l293_near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}

#[rstest]
#[case::won_in_a_clash(
    L293_WON,
    "Whenever you clash, draw a card. If you won, draw a card.",
    true
)]
#[case::won_needs_a_clash_trigger(
    L293_WON,
    "Whenever you scry, draw a card. If you won, draw a card.",
    false
)]
fn l293_won_is_written_only_after_a_clash(
    #[case] won: &str,
    #[case] text: &str,
    #[case] same: bool,
) {
    let when = if text.contains("clash") {
        r#"{"Did":{"action":"Clashed","who":"You"}}"#
    } else {
        r#"{"Did":{"action":"Scried","who":"You"}}"#
    };
    let json = format!(
        r#"{{"Triggered":{{"when":{when},"effects":[{{"Draw":{{}}}},{{"If":{{"condition":{won},"then":[{{"Draw":{{}}}}]}}}}]}}}}"#
    );
    let written = written_variants(&ability(&json));
    assert_eq!(
        written.iter().any(|w| same_text(w, text)),
        same,
        "{written:?}"
    );
}

#[rstest]
#[case::entangling_trap(
    "Whenever you clash, tap target creature an opponent controls. If you won, that creature doesn't untap during its controller's next untap step.",
    &format!(r#"{{"Triggered":{{"when":{{"Did":{{"action":"Clashed","who":"You"}}}},"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},{{"ControlledBy":"Opponents"}}]}}}}],"effects":[{{"Tap":{{"what":{{"Target":0}}}}}},{{"If":{{"condition":{L293_WON},"then":[{{"Continuous":{{"affected":{{"Target":0}},"mods":["DoesntUntap"],"duration":"NextUntapStep"}}}}]}}}}]}}}}"#)
)]
#[case::rebellion(
    "Whenever you clash, you may pay {1}. If you do, create a 3/1 red Elemental Shaman creature token. If you won, that token gains haste until end of turn.",
    &format!(r#"{{"Triggered":{{"when":{{"Did":{{"action":"Clashed","who":"You"}}}},"effects":[{{"Pay":{{"cost":"{{1}}","paid":[{{"CreateToken":{{"token":{{"Custom":{{"name":"Elemental Shaman","colors":["Red"],"types":["Creature"],"subtypes":["Elemental","Shaman"],"power":3,"toughness":1}}}},"as":"tokens"}}}},{{"If":{{"condition":{L293_WON},"then":[{{"Continuous":{{"affected":{{"Bound":"tokens"}},"mods":[{{"AddKeyword":"Haste"}}],"duration":"UntilEndOfTurn"}}}}]}}}}]}}}}]}}}}"#)
)]
fn l293_won_sentences_write_back_from_their_twins(#[case] text: &str, #[case] json: &str) {
    let written = written_variants(&ability(json));
    assert!(written.iter().any(|w| same_text(w, text)), "{written:?}");
    let other = json.replace(r#""right":1"#, r#""right":2"#);
    let written = written_variants(&ability(&other));
    assert!(!written.iter().any(|w| same_text(w, text)), "{written:?}");
}

/// "If you won" read in a trigger on no clash is never written back, so the compiler leaves it to the table.
#[test]
fn l293_won_without_a_clash_is_not_written_back() {
    let text = "Whenever you win a coin flip, draw a card. If you won, draw a card.";
    if let Some(read) = clause(text, ClauseKind::Triggered, false) {
        assert!(!written_variants(&read).iter().any(|w| same_text(w, text)));
    }
}
