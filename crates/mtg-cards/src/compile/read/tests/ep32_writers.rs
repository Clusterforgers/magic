//! Goal: the writers of ep32 write the card's words and nothing near them. "If a spell or ability an opponent controls causes you to discard ~, put it onto the battlefield [with N counters on it] instead of putting it into your graveyard." reads as the card's own discard caused by an opponent, replaced from the hand by its move to the battlefield; any discard, another player, another zone or another card does not read as it.
use super::*;

const SMITER: &str = "If a spell or ability an opponent controls causes you to discard ~, put it onto the battlefield instead of putting it into your graveyard.";

const DODECAPOD: &str = "If a spell or ability an opponent controls causes you to discard ~, put it onto the battlefield with two +1/+1 counters on it instead of putting it into your graveyard.";

fn surprise(by: &str, what: &str, counters: &str, from: &str) -> Ability {
    ability(&format!(
        r#"{{"Replacement":{{"when":{{"Discarded":{{"what":{what}{by}}}}},"by":{{"Instead":[{{"Move":{{"what":"This","to":{{"Battlefield":{{{counters}}}}}}}}}]}},"from":"{from}"}}}}"#
    ))
}

#[rstest]
#[case::loxodon_smiter(SMITER, "")]
#[case::dodecapod(DODECAPOD, r#""counters":[["+1/+1",2]]"#)]
fn ep32_discard_by_opponent_reads(#[case] text: &str, #[case] counters: &str) {
    let read = golden(text, ClauseKind::Static, false);
    assert_eq!(
        read,
        surprise(r#","by":"Opponents""#, r#"["This"]"#, counters, "Hand")
    );
}

#[rstest]
#[case::smiter(
    surprise(r#","by":"Opponents""#, r#"["This"]"#, "", "Hand"),
    SMITER,
    true
)]
#[case::dodecapod(
    surprise(
        r#","by":"Opponents""#,
        r#"["This"]"#,
        r#""counters":[["+1/+1",2]]"#,
        "Hand"
    ),
    DODECAPOD,
    true
)]
#[case::any_discard(surprise("", r#"["This"]"#, "", "Hand"), SMITER, false)]
#[case::by_you(surprise(r#","by":"You""#, r#"["This"]"#, "", "Hand"), SMITER, false)]
#[case::from_the_battlefield(
    surprise(r#","by":"Opponents""#, r#"["This"]"#, "", "Battlefield"),
    SMITER,
    false
)]
#[case::another_card(
    surprise(r#","by":"Opponents""#, r#"[{"Type":"Creature"}]"#, "", "Hand"),
    SMITER,
    false
)]
#[case::without_the_counters(
    surprise(r#","by":"Opponents""#, r#"["This"]"#, "", "Hand"),
    DODECAPOD,
    false
)]
#[case::tapped(
    surprise(r#","by":"Opponents""#, r#"["This"]"#, r#""tapped":true"#, "Hand"),
    SMITER,
    false
)]
fn ep32_discard_by_opponent_near_misses_differ(
    #[case] ability: Ability,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability);
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}
