//! Goal: "the sacrificed creature's power", "the discarded card's mana value" and the like read as a characteristic of what the cost took (`Ref::Paid`) in an activated ability whose cost takes objects and in a spell with a required additional cost that does, after "equal to" and in a where clause, write back as the same text, and stay with the table where the cost took nothing.
use super::*;
use crate::compile::read::Own;

/// A spell on a card whose required additional cost takes an object out of its zone.
fn paying(text: &str) -> Option<Ability> {
    let own = Own {
        pays: true,
        ..Own::default()
    };
    clause_with(text, ClauseKind::Spell, false, &own)
}

/// Reads and writes back the text of a spell on such a card.
fn paying_golden(text: &str) -> Ability {
    let read = paying(text).unwrap_or_else(|| panic!("not read: {text}"));
    assert!(
        render_variants(&read)
            .iter()
            .any(|written| same_text(written, text)),
        "{text}: {:?}",
        render_variants(&read)
    );
    read
}

fn stat(stat: &str) -> String {
    format!(r#"{{"Stat":{{"of":"Paid","stat":"{stat}"}}}}"#)
}

#[rstest]
#[case::pump(
    "{1}, Sacrifice another creature: ~ gets +X/+0 until end of turn, where X is the sacrificed creature's power.",
    "Power"
)]
#[case::damage_before_the_target(
    "{3}{R}, Sacrifice an artifact: ~ deals damage equal to the sacrificed artifact's mana value to any target.",
    "ManaValue"
)]
#[case::damage_after_the_target(
    "{R}, {T}, Discard a card: ~ deals damage to target creature equal to the discarded card's mana value.",
    "ManaValue"
)]
#[case::life_gained(
    "{W}, {T}, Sacrifice a creature: You gain life equal to the sacrificed creature's toughness.",
    "Toughness"
)]
#[case::life_lost(
    "{B}, {T}, Sacrifice an artifact: Target player loses life equal to the sacrificed artifact's mana value.",
    "ManaValue"
)]
#[case::each_opponent_loses(
    "{1}{B}{G}, Sacrifice another creature: Each opponent loses life equal to the sacrificed creature's power.",
    "Power"
)]
#[case::counters(
    "Sacrifice a creature: Put X +1/+1 counters on ~, where X is the sacrificed creature's power.",
    "Power"
)]
#[case::life_with_a_tap(
    "{2}, {T}, Sacrifice another creature: You gain X life, where X is the sacrificed creature's power.",
    "Power"
)]
#[case::draw(
    "Sacrifice a creature: Draw cards equal to the sacrificed creature's power.",
    "Power"
)]
fn an_ability_names_what_its_cost_took(#[case] text: &str, #[case] kind: &str) {
    let read = golden(text, ClauseKind::Activated, false);
    let json = serde_json::to_string(&read).unwrap();
    assert!(json.contains(&stat(kind)), "{json}");
}

#[rstest]
#[case::draw("Draw cards equal to the sacrificed creature's power.", "Power")]
#[case::life(
    "You gain life equal to the sacrificed creature's toughness.",
    "Toughness"
)]
#[case::loss(
    "Target player loses life equal to the sacrificed creature's power.",
    "Power"
)]
#[case::damage(
    "~ deals damage equal to the sacrificed creature's power to any target.",
    "Power"
)]
fn a_spell_names_what_its_additional_cost_took(#[case] text: &str, #[case] kind: &str) {
    let read = paying_golden(text);
    let json = serde_json::to_string(&read).unwrap();
    assert!(json.contains(&stat(kind)), "{json}");
}

#[rstest]
#[case::the_cost_took_nothing(
    "{T}: You gain life equal to the sacrificed creature's power.",
    ClauseKind::Activated
)]
#[case::a_tap_only_cost(
    "{2}, {T}: ~ gets +X/+0 until end of turn, where X is the sacrificed creature's power.",
    ClauseKind::Activated
)]
#[case::an_effect_sacrificed_it(
    "Draw cards equal to the sacrificed creature's power.",
    ClauseKind::Spell
)]
fn a_cost_that_took_nothing_leaves_the_text_to_the_table(
    #[case] text: &str,
    #[case] kind: ClauseKind,
) {
    assert_eq!(clause(text, kind, false), None);
}
