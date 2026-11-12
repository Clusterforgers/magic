//! Goal: the r30-7c writers word exactly the shapes they stand for (an attack on a player that isn't blocked, a defending
//! player's life total set to a number, no combat damage for the combat, the dealers of a batch of damage named together as
//! "each of those creatures", a spell cast from exile and a permanent entering from exile) and nothing near them: the
//! controller's life total, a rule for the turn, a single damage event, a spell cast from a graveyard, a permanent of any
//! player and a move to a hand keep their own words.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_cards::compile::written_variants;
use rstest::rstest;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

fn says(json: &str, text: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|written| written == text)
}

const MASTER: &str = r#"{"Triggered":{"when":{"Unblocked":{"what":["This",{"AttackingPlayer":"EachPlayer"}]}},"effects":[{"SetLife":{"who":"Defending","amount":1}},{"Rule":{"rule":{"Combat":{"who":"This","quirk":"AssignsNoCombatDamage"}},"duration":"UntilEndOfCombat"}}]}}"#;

const ZUKO: &str = r#"{"Triggered":{"when":{"AnyOf":[{"SpellCast":{"what":[{"CastFrom":"Exile"}],"by":"You"}},{"ZoneChange":{"what":[{"ControlledBy":"You"}],"from":"Exile","to":"Battlefield"}}]},"effects":[{"Draw":{}}]}}"#;

const ZUKO_TEXT: &str = "Whenever you cast a spell from exile and whenever a permanent you control enters from exile, draw a card.";

const SINGLE: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":[{"Subtype":"Ninja"},{"ControlledBy":"You"}],"to":"EachPlayer","combat":true}},"effects":[{"PutCounters":{"on":"Dealer","counter":"+1/+1"}},{"Draw":{}}]}}"#;

const HEROES: &str = r#"{"Triggered":{"when":{"Batch":{"DealsDamage":{"source":[{"Subtype":"Ninja"},{"ControlledBy":"You"}],"to":"EachPlayer","combat":true}}},"effects":[{"PutCounters":{"on":"Dealer","counter":"+1/+1"}},{"Draw":{}}]}}"#;

#[rstest]
#[case::master(
    MASTER,
    "Whenever ~ attacks a player and isn't blocked, that player's life total becomes 1. ~ assigns no combat damage this combat."
)]
#[case::batch_dealers(
    HEROES,
    "Whenever one or more Ninjas you control deal combat damage to a player, put a +1/+1 counter on each of those creatures and draw a card."
)]
#[case::single_dealer_stays_singular(
    SINGLE,
    "Whenever a Ninja you control deals combat damage to a player, put a +1/+1 counter on that creature, then draw a card."
)]
#[case::from_exile(ZUKO, ZUKO_TEXT)]
fn r307c_shapes_read_as_the_card(#[case] json: &str, #[case] text: &str) {
    assert!(
        says(json, text),
        "{text}\n{:#?}",
        written_variants(&ability(json))
    );
}

#[rstest]
#[case::your_life(
    &MASTER.replace(r#""who":"Defending","#, ""),
    "Whenever ~ attacks a player and isn't blocked, that player's life total becomes 1. ~ assigns no combat damage this combat."
)]
#[case::for_the_turn(
    &MASTER.replace("UntilEndOfCombat", "UntilEndOfTurn"),
    "Whenever ~ attacks a player and isn't blocked, that player's life total becomes 1. ~ assigns no combat damage this combat."
)]
#[case::batch_never_singular(
    HEROES,
    "Whenever one or more Ninjas you control deal combat damage to a player, put a +1/+1 counter on that creature, then draw a card."
)]
#[case::single_never_plural(
    SINGLE,
    "Whenever a Ninja you control deals combat damage to a player, put a +1/+1 counter on each of those creatures and draw a card."
)]
#[case::from_a_graveyard(&ZUKO.replace(r#"{"CastFrom":"Exile"}"#, r#"{"CastFrom":"Graveyard"}"#), ZUKO_TEXT)]
#[case::any_permanent(&ZUKO.replace(r#""what":[{"ControlledBy":"You"}]"#, r#""what":[]"#), ZUKO_TEXT)]
#[case::to_a_hand(&ZUKO.replace(r#""to":"Battlefield""#, r#""to":"Hand""#), ZUKO_TEXT)]
fn r307c_near_misses_keep_their_words(#[case] json: &str, #[case] text: &str) {
    assert!(!says(json, text), "{json} says {text}");
}
