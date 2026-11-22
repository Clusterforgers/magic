//! Goal: the ordinary wordings around the Star Trek counts read and write back as the card's text: "the number of opponents you attacked this turn" in a where clause, "{X} less to cast" and "to activate" of any count, a where clause that sets only how many targets, "whenever you attack", two subtypes of a group, mana of one chosen color counted from the game, Spacecraft in the plural, a permanent that isn't of a card type, and the new creature types; a near miss stays with the table.
use super::*;

#[rstest]
#[case::draw_and_lose(
    ClauseKind::Triggered,
    "Whenever ~ attacks, you draw X cards and lose X life, where X is the number of opponents you attacked this turn."
)]
#[case::pump_the_team(
    ClauseKind::Triggered,
    "Whenever ~ attacks, creatures you control get +X/+X until end of turn, where X is the number of opponents you attacked this turn."
)]
#[case::spell_costs_less(
    ClauseKind::Static,
    "This spell costs {X} less to cast, where X is the number of differently named lands you control."
)]
#[case::ability_costs_less(
    ClauseKind::Activated,
    "{4}{U}, {T}: Tap target creature. This ability costs {X} less to activate, where X is the number of differently named lands you control."
)]
#[case::goad_up_to_x(
    ClauseKind::Triggered,
    "Whenever ~ attacks, goad up to X target creatures, where X is the number of opponents you attacked this turn."
)]
#[case::whenever_you_attack(
    ClauseKind::Triggered,
    "Whenever you attack, put a +1/+1 counter on each of up to X target creatures you control, where X is the number of opponents you attacked this turn."
)]
#[case::two_subtypes(
    ClauseKind::Static,
    "Klingons and Warriors you control have vigilance."
)]
#[case::spacecraft(ClauseKind::Static, "Spacecraft you control have hexproof.")]
#[case::isnt_a_creature_trigger(
    ClauseKind::Triggered,
    "Whenever a creature dies, if ~ isn't a creature, draw a card."
)]
#[case::isnt_a_creature_static(
    ClauseKind::Static,
    "As long as ~ isn't a creature, it has hexproof."
)]
#[case::mana_equal_to(
    ClauseKind::Activated,
    "{T}: Choose a color. Add an amount of mana of that color equal to the number of opponents you attacked this turn."
)]
#[case::mana_of_creature_types(
    ClauseKind::Activated,
    "{5}, {T}: Choose a color. Add an amount of mana of that color equal to the number of creature types among non-Borg creatures you control."
)]
#[case::romulans(ClauseKind::Spell, "Create two 2/2 black Romulan creature tokens.")]
fn r116_05_wordings_read_and_write_back(#[case] kind: ClauseKind, #[case] text: &str) {
    let read = clause(text, kind, false).unwrap_or_else(|| panic!("not read: {text}"));
    let written = written_variants(&read);
    assert!(
        written.iter().any(|one| same_text(one, text)),
        "{text}: {written:?}"
    );
}

#[rstest]
#[case::subtype_and_nothing(ClauseKind::Static, "Klingons and you control have vigilance.")]
#[case::not_a_card_type(ClauseKind::Static, "As long as ~ isn't a Dragon, it has hexproof.")]
#[case::mana_of_a_number(
    ClauseKind::Activated,
    "{T}: Choose a color. Add an amount of mana of that color equal to 3."
)]
#[case::isnt_alone(ClauseKind::Static, "As long as ~ isn't, it has hexproof.")]
fn r116_05_near_misses_stay_with_the_table(#[case] kind: ClauseKind, #[case] text: &str) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}
