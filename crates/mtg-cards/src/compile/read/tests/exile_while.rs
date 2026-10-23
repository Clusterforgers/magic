//! Goal: "exile target ... until ~ leaves the battlefield" reads as an `Exile` that lasts while its source stays, as the effect of an enters trigger or an attack trigger, writes back as the same text, and a wording that differs (another object, another end, no source) stays with the table.
use super::*;

use crate::effect::Effect;
use crate::modify::Duration;

fn exiles_while_the_source_stays(read: &Ability) -> bool {
    matches!(
        read.effects(),
        [Effect::Exile {
            duration: Some(Duration::WhileSourceOnBattlefield),
            bind: None,
            ..
        }]
    )
}

#[rstest]
#[case::banisher_priest(
    "When ~ enters, exile target creature an opponent controls until ~ leaves the battlefield."
)]
#[case::banishing_light(
    "When ~ enters, exile target nonland permanent an opponent controls until ~ leaves the battlefield."
)]
#[case::an_artifact_or_creature(
    "When ~ enters, exile target artifact or creature an opponent controls until ~ leaves the battlefield."
)]
#[case::on_an_attack(
    "Whenever ~ attacks, exile target creature an opponent controls until ~ leaves the battlefield."
)]
fn exiling_until_the_source_leaves_reads_and_writes_back(#[case] text: &str) {
    let read = golden(text, ClauseKind::Triggered, false);
    assert!(exiles_while_the_source_stays(&read), "{read:?}");
}

#[test]
fn the_source_noun_does_not_matter() {
    for noun in [
        "creature",
        "enchantment",
        "Aura",
        "artifact",
        "Vehicle",
        "Saga",
    ] {
        let text = format!(
            "When this {noun} enters, exile target nonland permanent an opponent controls until this {noun} leaves the battlefield."
        );
        let read = clause(&text, ClauseKind::Triggered, false)
            .unwrap_or_else(|| panic!("not read: {text}"));
        assert!(exiles_while_the_source_stays(&read), "{text}");
    }
}

#[rstest]
#[case::another_permanent(
    "When ~ enters, exile target creature an opponent controls until target enchantment you control leaves the battlefield."
)]
#[case::until_end_of_turn(
    "When ~ enters, exile target creature an opponent controls until end of turn."
)]
#[case::no_end("When ~ enters, exile target creature an opponent controls until.")]
#[case::then_a_cast(
    "When ~ enters, exile target creature an opponent controls until ~ leaves the battlefield. You may cast that card."
)]
#[case::dies("When ~ enters, exile target creature an opponent controls until ~ dies.")]
fn a_wording_that_differs_stays_with_the_table(#[case] text: &str) {
    assert!(
        clause(text, ClauseKind::Triggered, false).is_none(),
        "{text}"
    );
}
