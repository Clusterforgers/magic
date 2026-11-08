//! Goal: the d14 wordings ("until end of turn, whenever", "any number of target", "add an additional") read as the text they mean, and a text of another meaning (other duration, other count) never reads as the clause.
use super::*;

#[rstest]
#[case::before(
    "Until end of turn, whenever you cast an instant or sorcery spell, copy it.",
    "Whenever you cast an instant or sorcery spell this turn, copy it.",
    true
)]
#[case::same_form(
    "Whenever you cast an instant or sorcery spell this turn, copy it.",
    "Whenever you cast an instant or sorcery spell this turn, copy it.",
    true
)]
#[case::no_duration(
    "Until end of turn, whenever you cast an instant or sorcery spell, copy it.",
    "Whenever you cast an instant or sorcery spell, copy it.",
    false
)]
#[case::other_spells(
    "Until end of turn, whenever you cast an instant or sorcery spell, copy it.",
    "Whenever you cast a creature spell this turn, copy it.",
    false
)]
#[case::other_effect(
    "Until end of turn, whenever you cast an instant or sorcery spell, copy it.",
    "Whenever you cast an instant or sorcery spell this turn, draw a card.",
    false
)]
fn d14_until_end_of_turn_whenever(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_text(left, right), same, "{right}");
}

#[rstest]
#[case::yours(
    "Up to X target creatures you control phase out, where X is the number of creatures you control.",
    "Any number of target creatures you control phase out.",
    true
)]
#[case::graveyard(
    "Put up to X target creature cards from your graveyard on top of your library, where X is the number of creature cards in your graveyard.",
    "Put any number of target creature cards from your graveyard on top of your library.",
    true
)]
#[case::battlefield(
    "~ deals X damage divided as you choose among up to X target creatures, where X is the number of creatures on the battlefield.",
    "~ deals X damage divided as you choose among any number of target creatures.",
    true
)]
#[case::other_pool(
    "Up to X target creatures you control phase out, where X is the number of lands you control.",
    "Any number of target creatures you control phase out.",
    false
)]
#[case::other_owner(
    "Up to X target creatures you control phase out, where X is the number of creatures your opponents control.",
    "Any number of target creatures you control phase out.",
    false
)]
#[case::wider_pool(
    "Up to X target creatures you control phase out, where X is the number of creatures.",
    "Any number of target creatures you control phase out.",
    false
)]
#[case::fixed_limit(
    "Up to two target creatures you control phase out.",
    "Any number of target creatures you control phase out.",
    false
)]
fn d14_any_number_of_targets(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_text(left, right), same, "{right}");
}

#[rstest]
#[case::plain("Whenever you tap a land for mana, add {G}.", true)]
#[case::additional("Whenever you tap a land for mana, add an additional {G}.", true)]
#[case::other_color("Whenever you tap a land for mana, add {R}.", false)]
#[case::two("Whenever you tap a land for mana, add {G}{G}.", false)]
fn d14_additional_mana(#[case] clause: &str, #[case] same: bool) {
    assert_eq!(
        same_text("Whenever you tap a land for mana, add {G}.", clause),
        same,
        "{clause}"
    );
}
