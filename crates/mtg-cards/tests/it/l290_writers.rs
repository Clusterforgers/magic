//! Goal: the l29-0 writers word exactly the shapes they stand for (an exhaust or boast activation, a player becoming a target, a payment of mana and a discard, a keyword gained with enough power, an attack while you have the most life, the Everywhere token, a land that becomes a basic type while the source stays, an attack with the card and/or your commander) and nothing near them; an activation is an exhaust or boast activation only by the limits of those keywords.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_cards::compile::written_variants;
use mtg_cards::did::Action;
use rstest::rstest;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

fn says(json: &str, text: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|written| written == text)
}

#[rstest]
#[case::exhaust(
    r#"{"Triggered":{"when":{"Did":{"action":"ActivatedExhaust","who":"You"}},"effects":[{"Draw":{}}]}}"#,
    "Whenever you activate an exhaust ability, draw a card."
)]
#[case::boast(
    r#"{"Triggered":{"when":{"Did":{"action":"ActivatedBoast","who":"You"}},"effects":[{"Draw":{}}]}}"#,
    "Whenever you activate a boast ability, draw a card."
)]
#[case::you_targeted_by_an_opponent(
    r#"{"Triggered":{"when":{"PlayerBecomesTarget":{"who":"You","by":[{"ControlledBy":"Opponents"}]}},"effects":[{"Draw":{}}]}}"#,
    "Whenever you become the target of a spell or ability an opponent controls, draw a card."
)]
#[case::you_targeted_by_a_spell(
    r#"{"Triggered":{"when":{"PlayerBecomesTarget":{"who":"You","by":["Spell"]}},"effects":[{"Draw":{}}]}}"#,
    "Whenever you become the target of a spell, draw a card."
)]
#[case::pay_and_discard(
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Pay":{"cost":[{"Mana":"{1}"},{"Discard":{}}],"paid":[{"Draw":{}}]}}]}}"#,
    "Whenever ~ attacks, you may pay {1} and discard a card. If you do, draw a card."
)]
#[case::trample_with_power(
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"If":{"condition":{"Matches":{"subject":"This","selector":[{"Compare":{"stat":"Power","op":"Ge","to":10}}]}},"then":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"Trample"}],"duration":"UntilEndOfTurn"}}]}}]}}"#,
    "Whenever ~ attacks, it gains trample until end of turn if its power is 10 or greater."
)]
#[case::most_life(
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"if":{"AtTrigger":{"Among":"MostLife"}},"effects":[{"Draw":{}},{"LoseLife":{"amount":1}}]}}"#,
    "Whenever ~ attacks while you have the most life or are tied for most life, you draw a card and you lose 1 life."
)]
#[case::everywhere(
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Everywhere","types":["Land"],"subtypes":["Plains","Island","Swamp","Mountain","Forest"]}},"tapped":true}}]}}"#,
    "Whenever ~ attacks, create a tapped colorless land token named Everywhere that is every basic land type."
)]
#[case::becomes_a_plains(
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Land"}]}}],"effects":[{"May":{"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"SetLandTypes":["Plains"]}],"duration":"WhileSourceOnBattlefield"}}]}}]}}"#,
    "Whenever ~ attacks, you may have target land become a Plains until ~ leaves the battlefield."
)]
#[case::with_your_commander(
    r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"ControlledBy":"You"},{"AnyOf":["This",{"Is":{"Each":["Commander",{"OwnedBy":"You"}]}}]}]}},"effects":[{"Draw":{}}]}}"#,
    "Whenever you attack with ~ and/or your commander, draw a card."
)]
fn a_shape_reads_as_its_card(#[case] json: &str, #[case] text: &str) {
    assert!(
        says(json, text),
        "{text}\n{:?}",
        written_variants(&ability(json))
    );
}

#[rstest]
#[case::exhaust_of_an_opponent(
    r#"{"Triggered":{"when":{"Did":{"action":"ActivatedExhaust","who":"Opponents"}},"effects":[{"Draw":{}}]}}"#,
    "Whenever you activate an exhaust ability, draw a card."
)]
#[case::opponent_targeted(
    r#"{"Triggered":{"when":{"PlayerBecomesTarget":{"who":"Opponents","by":[]}},"effects":[{"Draw":{}}]}}"#,
    "Whenever you become the target of a spell or ability, draw a card."
)]
#[case::pay_and_discard_two(
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Pay":{"cost":[{"Mana":"{1}"},{"Discard":{"count":2}}],"paid":[{"Draw":{}}]}}]}}"#,
    "Whenever ~ attacks, you may pay {1} and discard a card. If you do, draw a card."
)]
#[case::trample_with_toughness(
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"If":{"condition":{"Matches":{"subject":"This","selector":[{"Compare":{"stat":"Toughness","op":"Ge","to":10}}]}},"then":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"Trample"}],"duration":"UntilEndOfTurn"}}]}}]}}"#,
    "Whenever ~ attacks, it gains trample until end of turn if its power is 10 or greater."
)]
#[case::checked_again_on_resolution(
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"if":{"Among":"MostLife"},"effects":[{"Draw":{}},{"LoseLife":{"amount":1}}]}}"#,
    "Whenever ~ attacks while you have the most life or are tied for most life, you draw a card and you lose 1 life."
)]
#[case::untapped_everywhere(
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Everywhere","types":["Land"],"subtypes":["Plains","Island","Swamp","Mountain","Forest"]}}}}]}}"#,
    "Whenever ~ attacks, create a tapped colorless land token named Everywhere that is every basic land type."
)]
#[case::plains_until_end_of_turn(
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Land"}]}}],"effects":[{"May":{"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"SetLandTypes":["Plains"]}],"duration":"UntilEndOfTurn"}}]}}]}}"#,
    "Whenever ~ attacks, you may have target land become a Plains until ~ leaves the battlefield."
)]
#[case::anyones_commander(
    r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"ControlledBy":"You"},{"AnyOf":["This","Commander"]}]}},"effects":[{"Draw":{}}]}}"#,
    "Whenever you attack with ~ and/or your commander, draw a card."
)]
fn a_near_miss_is_not_worded_as_the_card(#[case] json: &str, #[case] text: &str) {
    assert!(!says(json, text));
}

#[rstest]
#[case::exhaust(r#"{"Activated":{"cost":"{1}","limit":"OncePerGame","effects":[{"Draw":{}}]}}"#, vec![Action::ActivatedExhaust])]
#[case::boast(
    r#"{"Activated":{"cost":"{1}","condition":{"Matches":{"subject":"This","selector":["AttackedThisTurn"]}},"limit":"OncePerTurn","effects":[{"Draw":{}}]}}"#,
    vec![Action::ActivatedBoast]
)]
#[case::once_each_turn(r#"{"Activated":{"cost":"{1}","limit":"OncePerTurn","effects":[{"Draw":{}}]}}"#, vec![])]
#[case::plain(r#"{"Activated":{"cost":"{1}","effects":[{"Draw":{}}]}}"#, vec![])]
#[case::loyalty(r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"Draw":{}}]}}"#, vec![])]
fn an_activation_is_exhaust_or_boast_by_its_limits(#[case] json: &str, #[case] kinds: Vec<Action>) {
    assert_eq!(Action::activation_kinds(&ability(json)), kinds);
}
