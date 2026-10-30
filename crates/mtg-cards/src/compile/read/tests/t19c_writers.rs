//! Goal: a choice of several cards of the hand put onto the battlefield and a choice of one of two modes without a target give the oracle wording of the data, and another count, zone, player, kind or phrase never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// A trigger on the card entering whose effects are the choice and the move of what was chosen.
fn entering(choose: &str, moved: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[{choose},{moved}]}}}}"#
    )
}

/// The choice of cards of `owner`'s `zone` of the given kind, between `min` and `max` (`null` for any number).
fn pick(zone: &str, owner: &str, kind: &str, min: u32, max: &str) -> String {
    format!(
        r#"{{"Choose":{{"as":"card","from":{{"Each":[{{"In":"{zone}"}},{{"OwnedBy":"{owner}"}},{kind}]}},"count":{{"min":{min},"max":{max}}}}}}}"#
    )
}

const CREATURE: &str = r#"{"Type":"Creature"}"#;
const LAND: &str = r#"{"Type":"Land"}"#;
const AURA: &str = r#"{"Subtype":"Aura"}"#;
const BATTLEFIELD: &str = r#"{"Move":{"what":{"Bound":"card"},"to":{"Battlefield":{}}}}"#;
const TAPPED: &str = r#"{"Move":{"what":{"Bound":"card"},"to":{"Battlefield":{"tapped":true}}}}"#;
const HAND: &str = r#"{"Move":{"what":{"Bound":"card"},"to":"Hand"}}"#;

#[rstest]
#[case::any_number(
    CREATURE,
    "null",
    BATTLEFIELD,
    "When ~ enters, put any number of creature cards from your hand onto the battlefield."
)]
#[case::any_number_tapped(
    LAND,
    "null",
    TAPPED,
    "When ~ enters, put any number of land cards from your hand onto the battlefield tapped."
)]
#[case::up_to_two(
    CREATURE,
    "2",
    BATTLEFIELD,
    "When ~ enters, put up to two creature cards from your hand onto the battlefield."
)]
#[case::up_to_three(
    LAND,
    "3",
    TAPPED,
    "When ~ enters, put up to three land cards from your hand onto the battlefield tapped."
)]
#[case::a_subtype(
    AURA,
    "null",
    BATTLEFIELD,
    "When ~ enters, put any number of Aura cards from your hand onto the battlefield."
)]
fn several_cards_of_the_hand_put_onto_the_battlefield_are_written(
    #[case] kind: &str,
    #[case] most: &str,
    #[case] moved: &str,
    #[case] text: &str,
) {
    let json = entering(&pick("Hand", "You", kind, 0, most), moved);
    assert!(
        written(&json, text),
        "{text}: {:?}",
        written_variants(&ability(&json))
    );
}

#[test]
fn the_same_choice_in_a_spell_is_written_too() {
    let json = format!(
        r#"{{"Spell":{{"effects":[{},{BATTLEFIELD}]}}}}"#,
        pick("Hand", "You", CREATURE, 0, "2")
    );
    assert!(written(
        &json,
        "Put up to two creature cards from your hand onto the battlefield."
    ));
}

#[rstest]
#[case::another_number(
    "Hand",
    "You",
    CREATURE,
    1,
    "null",
    BATTLEFIELD,
    "When ~ enters, put any number of creature cards from your hand onto the battlefield."
)]
#[case::at_least_one_is_not_any(
    "Hand",
    "You",
    CREATURE,
    0,
    "null",
    BATTLEFIELD,
    "When ~ enters, put one or more creature cards from your hand onto the battlefield."
)]
#[case::up_to_two_is_not_up_to_three(
    "Hand",
    "You",
    CREATURE,
    0,
    "2",
    BATTLEFIELD,
    "When ~ enters, put up to three creature cards from your hand onto the battlefield."
)]
#[case::up_to_two_is_not_any(
    "Hand",
    "You",
    CREATURE,
    0,
    "2",
    BATTLEFIELD,
    "When ~ enters, put any number of creature cards from your hand onto the battlefield."
)]
#[case::any_is_not_up_to_two(
    "Hand",
    "You",
    CREATURE,
    0,
    "null",
    BATTLEFIELD,
    "When ~ enters, put up to two creature cards from your hand onto the battlefield."
)]
#[case::another_kind(
    "Hand",
    "You",
    LAND,
    0,
    "null",
    BATTLEFIELD,
    "When ~ enters, put any number of creature cards from your hand onto the battlefield."
)]
#[case::tapped_is_not_untapped(
    "Hand",
    "You",
    LAND,
    0,
    "null",
    TAPPED,
    "When ~ enters, put any number of land cards from your hand onto the battlefield."
)]
#[case::another_zone(
    "Graveyard",
    "You",
    CREATURE,
    0,
    "null",
    BATTLEFIELD,
    "When ~ enters, put any number of creature cards from your hand onto the battlefield."
)]
#[case::an_opponents_hand(
    "Hand",
    "Opponents",
    CREATURE,
    0,
    "null",
    BATTLEFIELD,
    "When ~ enters, put any number of creature cards from your hand onto the battlefield."
)]
#[case::another_place(
    "Hand",
    "You",
    CREATURE,
    0,
    "null",
    HAND,
    "When ~ enters, put any number of creature cards from your hand onto the battlefield."
)]
fn another_count_zone_player_kind_or_place_is_not_the_text(
    #[case] zone: &str,
    #[case] owner: &str,
    #[case] kind: &str,
    #[case] min: u32,
    #[case] most: &str,
    #[case] moved: &str,
    #[case] text: &str,
) {
    let json = entering(&pick(zone, owner, kind, min, most), moved);
    assert!(!written(&json, text), "{text}");
}

/// An activated ability that chooses one of two modes, each a change of the enchanted creature.
fn aura_ability(first: &str, second: &str, choose: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":"{{1}}","effects":[{{"Modal":{{{choose}"modes":[{},{}]}}}}]}}}}"#,
        mode("Attached", first),
        mode("Attached", second)
    )
}

/// A mode that changes `affected` until end of turn with the modification.
fn mode(affected: &str, mods: &str) -> String {
    format!(
        r#"{{"effects":[{{"Continuous":{{"affected":"{affected}","duration":"UntilEndOfTurn","mods":[{mods}]}}}}]}}"#
    )
}

const UP: &str = r#"{"ModifyPT":{"power":1,"toughness":-1}}"#;
const DOWN: &str = r#"{"ModifyPT":{"power":-1,"toughness":1}}"#;
const BIG: &str = r#"{"ModifyPT":{"power":2,"toughness":2}}"#;
const FLYING: &str = r#"{"AddKeyword":"Flying"}"#;
const REACH: &str = r#"{"AddKeyword":"Reach"}"#;

#[rstest]
#[case::power_for_toughness(
    UP,
    DOWN,
    "{1}: Enchanted creature gets +1/-1 or -1/+1 until end of turn."
)]
#[case::two_keywords(
    FLYING,
    REACH,
    "{1}: Enchanted creature gains flying or reach until end of turn."
)]
#[case::a_size(
    BIG,
    UP,
    "{1}: Enchanted creature gets +2/+2 or +1/-1 until end of turn."
)]
fn one_of_two_changes_of_the_enchanted_creature_is_written_with_or(
    #[case] first: &str,
    #[case] second: &str,
    #[case] text: &str,
) {
    let json = aura_ability(first, second, "");
    assert!(
        written(&json, text),
        "{text}: {:?}",
        written_variants(&ability(&json))
    );
}

#[rstest]
#[case::the_other_order(
    UP,
    DOWN,
    "{1}: Enchanted creature gets -1/+1 or +1/-1 until end of turn."
)]
#[case::another_number(
    UP,
    DOWN,
    "{1}: Enchanted creature gets +1/-1 or -1/+2 until end of turn."
)]
#[case::both_are_not_one_of(
    UP,
    DOWN,
    "{1}: Enchanted creature gets +1/-1 and -1/+1 until end of turn."
)]
#[case::no_or(UP, DOWN, "{1}: Enchanted creature gets +1/-1 until end of turn.")]
#[case::another_creature(
    UP,
    DOWN,
    "{1}: Target creature gets +1/-1 or -1/+1 until end of turn."
)]
#[case::another_keyword(
    FLYING,
    REACH,
    "{1}: Enchanted creature gains flying or trample until end of turn."
)]
fn another_phrase_or_order_is_not_the_text(
    #[case] first: &str,
    #[case] second: &str,
    #[case] text: &str,
) {
    let json = aura_ability(first, second, "");
    assert!(!written(&json, text), "{text}");
}

#[rstest]
#[case::two_of_two(r#""choose":{"min":2,"max":2},"#)]
#[case::one_or_both(r#""choose":{"min":1,"max":2},"#)]
#[case::at_random(r#""at_random":true,"#)]
fn a_choice_of_more_than_one_mode_is_not_worded_with_or(#[case] choose: &str) {
    let json = aura_ability(UP, DOWN, choose);
    assert!(!written(
        &json,
        "{1}: Enchanted creature gets +1/-1 or -1/+1 until end of turn."
    ));
}

#[test]
fn modes_of_two_different_creatures_are_not_worded_with_or() {
    let json = format!(
        r#"{{"Activated":{{"cost":"{{1}}","effects":[{{"Modal":{{"modes":[{},{}]}}}}]}}}}"#,
        mode("Attached", UP),
        mode("This", DOWN)
    );
    assert!(!written(
        &json,
        "{1}: Enchanted creature gets +1/-1 or -1/+1 until end of turn."
    ));
}
