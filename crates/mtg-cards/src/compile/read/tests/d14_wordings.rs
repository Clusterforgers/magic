//! Goal: the d14 wordings (the name of a mode, the possessive of "owner", "until end of turn, whenever", "any number of target", "add an additional") read as the text they mean, and a text of another meaning (other mode, other zone or player, other duration, other count) never reads as the clause.
use super::*;

/// Whether some wording the checker accepts for the rule is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const MODES: &str = "Choose one \u{2014}\n\u{2022} Creatures you control get +2/+0 until end of turn.\n\u{2022} Create two 1/1 red Goblin creature tokens.";

#[rstest]
#[case::named_modes(
    "Choose one \u{2014}\n\u{2022} Charge Them \u{2014} Creatures you control get +2/+0 until end of turn.\n\u{2022} Befriend Them \u{2014} Create two 1/1 red Goblin creature tokens.",
    true
)]
#[case::one_name(
    "Choose one \u{2014}\n\u{2022} Charge Them \u{2014} Creatures you control get +2/+0 until end of turn.\n\u{2022} Create two 1/1 red Goblin creature tokens.",
    true
)]
#[case::other_body(
    "Choose one \u{2014}\n\u{2022} Charge Them \u{2014} Creatures you control get +3/+0 until end of turn.\n\u{2022} Befriend Them \u{2014} Create two 1/1 red Goblin creature tokens.",
    false
)]
#[case::other_count(
    "Choose one \u{2014}\n\u{2022} Charge Them \u{2014} Creatures you control get +2/+0 until end of turn.\n\u{2022} Befriend Them \u{2014} Create three 1/1 red Goblin creature tokens.",
    false
)]
#[case::an_action_is_no_name(
    "Choose one \u{2014}\n\u{2022} Creatures you control get +2/+0 until end of turn.\n\u{2022} Sacrifice a creature \u{2014} Create two 1/1 red Goblin creature tokens.",
    false
)]
fn d14_names_of_modes(#[case] clause: &str, #[case] same: bool) {
    assert_eq!(same_text(MODES, clause), same, "{clause}");
}

const BOUNCE: &str =
    r#"{"Spell":{"effects":[{"Move":{"to":"Hand","what":{"Each":[{"Type":"Creature"}]}}}]}}"#;
const TO_LIBRARY: &str = r#"{"Spell":{"effects":[{"Move":{"to":{"Library":{"place":"Top"}},"what":{"Each":[{"Type":"Creature"}]}}}]}}"#;

#[rstest]
#[case::their_owners(BOUNCE, "Return all creatures to their owners' hands.", true)]
#[case::their_owner(BOUNCE, "Return all creatures to their owner's hand.", true)]
#[case::your_hand(BOUNCE, "Return all creatures to your hand.", false)]
#[case::controllers_hands(BOUNCE, "Return all creatures to their controllers' hands.", false)]
#[case::library(TO_LIBRARY, "Return all creatures to their owners' hands.", false)]
fn d14_owners(#[case] json: &str, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(json, clause), reads, "{clause}");
}

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
