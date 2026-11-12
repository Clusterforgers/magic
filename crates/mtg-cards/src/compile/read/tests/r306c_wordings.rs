//! Goal: the r30-6c wordings ("at end of combat on your turn", "for each opponent you attacked", "This ability triggers
//! only once.") read into the DSL and write back as the card's own text; near misses stay with the table, and the whole
//! abilities of the pack's table are written only as they are.
use super::*;

#[rstest]
#[case::end_of_combat_draw(
    "At end of combat on your turn, draw a card.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"EndCombat","whose":"You"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::once_each_turn_still(
    "Whenever you gain life, draw a card. This ability triggers only once each turn.",
    r#"{"Triggered":{"when":{"LifeGained":{"who":"You"}},"limit":"OncePerTurn","effects":[{"Draw":{}}]}}"#
)]
#[case::once_ever_draw(
    "Whenever you gain life, draw a card. This ability triggers only once.",
    r#"{"Triggered":{"when":{"LifeGained":{"who":"You"}},"limit":"OncePerGame","effects":[{"Draw":{}}]}}"#
)]
fn r306c_wordings_read_and_write_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

/// The pack's cards' clauses are written back from their rules.
#[rstest]
#[case::rose(
    "At end of combat on your turn, if you attacked this turn, create a Junk token for each opponent you attacked.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"EndCombat","whose":"You"}},"if":{"Compare":{"left":{"Tallied":{"who":"You","what":"Attackers"}},"op":"Ge","right":1}},"effects":[{"CreateToken":{"token":{"Named":"junk"},"count":{"Players":"Attacked"}}}]}}"#
)]
#[case::once_ever(
    "At the beginning of your second main phase, if this creature is tapped, put a flying counter on it. This ability triggers only once.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Main2","whose":"You"}},"if":{"CountOf":{"subject":"This","selector":["Tapped"],"op":"Ge","right":1}},"limit":"OncePerGame","effects":[{"PutCounters":{"on":"This","counter":{"Other":"flying"}}}]}}"#
)]
fn r306c_card_clauses_write_back(#[case] text: &str, #[case] json: &str) {
    let written = written_variants(&ability(json));
    assert!(written.iter().any(|w| same_text(w, text)), "{written:?}");
}

#[rstest]
#[case::each_opponent_end_of_combat("At end of combat on each opponent's turn, draw a card.")]
#[case::once_twice("Whenever you gain life, draw a card. This ability triggers only twice.")]
fn r306c_near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}

/// "for each opponent" and "for each opponent you attacked" are different counts, each written as itself.
#[rstest]
#[case::attacked(r#""Attacked""#, "for each opponent you attacked", true)]
#[case::opponents(r#""Opponents""#, "for each opponent you attacked", false)]
#[case::plain(r#""Opponents""#, "for each opponent", true)]
fn r306c_attacked_opponents_are_their_own_count(
    #[case] players: &str,
    #[case] words: &str,
    #[case] same: bool,
) {
    let json = format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"EndCombat","whose":"You"}}}},"effects":[{{"CreateToken":{{"token":{{"Named":"junk"}},"count":{{"Players":{players}}}}}}}]}}}}"#
    );
    let text = format!("At end of combat on your turn, create a Junk token {words}.");
    let written = written_variants(&ability(&json));
    assert_eq!(
        written.iter().any(|w| same_text(w, &text)),
        same,
        "{written:?}"
    );
}

/// A whole ability of the table is written only when it matches exactly: one more quest counter needed reads otherwise.
#[test]
fn r306c_a_changed_whole_is_not_written_as_the_card() {
    let text = "Whenever a land you control enters, put a quest counter on this enchantment. When you do, if it has four or more quest counters on it, put a +1/+1 counter on target creature you control. It gains trample until end of turn.";
    let cond = |n: u32| {
        format!(
            r#"{{"Compare":{{"left":{{"Counters":{{"on":"This","counter":{{"Other":"quest"}}}}}},"op":"Ge","right":{n}}}}}"#
        )
    };
    let json = |n: u32| {
        format!(
            r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":[{{"Type":"Land"}},{{"ControlledBy":"You"}}],"to":"Battlefield"}}}},"effects":[{{"PutCounters":{{"on":"This","counter":{{"Other":"quest"}}}}}},{{"If":{{"condition":{c},"then":[{{"Reflexive":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}]}}}}],"effects":[{{"If":{{"condition":{c},"then":[{{"PutCounters":{{"on":{{"Target":0}},"counter":"+1/+1"}}}},{{"Continuous":{{"affected":{{"Target":0}},"mods":[{{"AddKeyword":"Trample"}}],"duration":"UntilEndOfTurn"}}}}]}}}}]}}}}]}}}}]}}}}"#,
            c = cond(n)
        )
    };
    assert!(
        written_variants(&ability(&json(4)))
            .iter()
            .any(|w| same_text(w, text))
    );
    assert!(
        !written_variants(&ability(&json(5)))
            .iter()
            .any(|w| same_text(w, text))
    );
}
