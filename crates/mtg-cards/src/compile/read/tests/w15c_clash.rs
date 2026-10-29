//! Goal: the clash wordings of `w15c_clash` (a clash with the defending player, a won clash that adds colorless mana at the next main phase) and "whenever you win a clash" read into the DSL and write back as the card's own text; near misses stay with the table.
use super::*;

#[rstest]
#[case::marvo_attacks(
    "Whenever ~ attacks, clash with defending player.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Clash":{"with":"Defending"}}]}}"#
)]
#[case::marvo_wins(
    "Whenever you win a clash, draw a card. Then you may cast a spell from your hand with mana value 8 or less without paying its mana cost.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"W15cClashWon":{"who":"You"}},"effects":[{"Draw":{}},{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"},{"Not":{"Type":"Land"}},{"Compare":{"stat":"ManaValue","op":"Le","to":8}}]},"count":{"min":0,"max":1},"as":"spell"}},{"CastFromZone":{"what":{"Bound":"spell"},"free":true}}]}}"#
)]
#[case::scattering_stroke(
    "Counter target spell. Clash with an opponent. If you win, at the beginning of your next main phase, you may add an amount of {C} equal to that spell's mana value.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"Counter":{"what":{"Target":0}}},{"Clash":{"win":[{"DelayedTrigger":{"when":{"AnyOf":[{"StepBegins":{"step":"Main1","whose":"You"}},{"StepBegins":{"step":"Main2","whose":"You"}}]},"effects":[{"May":{"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]},"times":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}}}}]}}]}}]}}]}}"#
)]
fn w15c_clash_wordings_read_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::defending_with_a_branch("Clash with defending player. If you win, draw a card.")]
#[case::no_target(
    "Clash with an opponent. If you win, at the beginning of your next main phase, you may add an amount of {C} equal to that spell's mana value."
)]
#[case::colored(
    "Counter target spell. Clash with an opponent. If you win, at the beginning of your next main phase, you may add an amount of {R} equal to that spell's mana value."
)]
fn w15c_clash_near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::anyone("Whenever a player wins a clash, draw a card.", r#"{}"#)]
#[case::an_opponent("Whenever an opponent wins a clash, draw a card.", r#"{"who":"Opponents"}"#)]
fn w15c_clash_won_reads_its_player(#[case] text: &str, #[case] who: &str) {
    let json = format!(r#"{{"Triggered":{{"when":{{"W15cClashWon":{who}}},"effects":[{{"Draw":{{}}}}]}}}}"#);
    reads_as(text, ClauseKind::Triggered, false, &json);
}
