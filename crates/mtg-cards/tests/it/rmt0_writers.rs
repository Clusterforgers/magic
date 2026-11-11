//! Goal: the writers the rmt0 pack adds (cleave's bracketed words, waterbend as a cost, a spell's additional cost named
//! by its pronoun, a casting restriction after an opponent's upkeep, a spell not cast from a hand, creature types before
//! the noun, a subtype it isn't after the noun, X named in a sentence of its own, a revealed hand you pick the discard from) accept only the rules the printed text says: one changed value
//! in a corpus card's rules is no longer accepted.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::BTreeSet;

use mtg_cards::load::slug;
use mtg_cards::proposal::{Verdict, check_exact};
use mtg_cards::support::Caps;
use rstest::rstest;
use serde_json::{Value, json};

/// The verdict on the rules file of `name` with the value at `pointer` (in its first face's abilities) set to `value`,
/// or as it is when `pointer` is empty.
fn verdict(name: &str, pointer: &str, value: Value) -> Verdict {
    let printed = common::printed();
    let card = printed.iter().find(|card| card.name == name).unwrap();
    let text =
        std::fs::read_to_string(common::cards_dir().join(format!("{}.json", slug(name)))).unwrap();
    let mut rules: Value = serde_json::from_str(&text).unwrap();
    if !pointer.is_empty() {
        *rules
            .pointer_mut(&format!("/faces/0/abilities{pointer}"))
            .unwrap_or_else(|| panic!("{name} has {pointer}")) = value;
    }
    let names: BTreeSet<String> = printed
        .iter()
        .map(|card| card.name.to_lowercase())
        .collect();
    let library = common::Corpus::repo().library();
    check_exact(
        &rules.to_string(),
        card,
        &library,
        &names,
        &Caps::everything(),
    )
    .verdict()
}

#[rstest]
#[case::cleave("Fierce Retribution")]
#[case::cleave_two_runs("Dig Up")]
#[case::waterbend("Water Whip")]
#[case::waterbend_may("Katara, Seeking Revenge")]
#[case::after_upkeep("Reset")]
#[case::not_from_a_hand("Wash Away")]
#[case::subtypes_before_the_noun("Lunar Rejection")]
#[case::hand_pick("Dread Fugue")]
#[case::x_sentence("Lantern Flare")]
#[case::lacking_subtype("Heliod, the Radiant Dawn // Heliod, the Warped Eclipse")]
fn the_card_as_written_is_accepted(#[case] name: &str) {
    assert_eq!(verdict(name, "", Value::Null), Verdict::Accepted);
}

#[rstest]
#[case::cleave_keeps_the_word("Fierce Retribution", "/0/Keyword/Cleave/spell/Spell/targets/0/what/Object", json!([{"Type": "Creature"}, "Attacking"]))]
#[case::cleave_another_word("Fierce Retribution", "/0/Keyword/Cleave/spell/Spell/targets/0/what/Object", json!([{"Type": "Creature"}, "Blocking"]))]
#[case::cleave_another_cost("Fierce Retribution", "/0/Keyword/Cleave/cost", json!("{4}{W}"))]
#[case::cleave_another_effect("Fierce Retribution", "/0/Keyword/Cleave/spell/Spell/effects/0", json!({"Exile": {"what": {"Target": 0}}}))]
#[case::waterbend_another_amount("Water Whip", "/0/AdditionalCost/cost/0/Waterbend", json!(4))]
#[case::waterbend_optional("Water Whip", "/0/AdditionalCost/required", json!(false))]
#[case::paid_rather_than_unpaid("Katara, Seeking Revenge", "/1/Triggered/effects/1/If/condition", json!("AdditionalCostPaid"))]
#[case::before_upkeep("Reset", "/0/CastOnly/condition/All/2/Not/Step", json!("Draw"))]
#[case::on_your_turn("Reset", "/0/CastOnly/condition/All/0", json!("YourTurn"))]
#[case::from_a_hand("Wash Away", "/1/Spell/targets/0/what/Object/1", json!({"CastFrom": "Hand"}))]
#[case::from_a_graveyard("Wash Away", "/1/Spell/targets/0/what/Object/1", json!({"Not": {"CastFrom": "Graveyard"}}))]
#[case::one_subtype("Lunar Rejection", "/1/Spell/targets/0/what/Object/1", json!({"Subtype": "Wolf"}))]
#[case::another_subtype("Lunar Rejection", "/1/Spell/targets/0/what/Object/1/AnyOf/1", json!({"Subtype": "Elf"}))]
#[case::they_choose("Dread Fugue", "/1/Spell/effects/1/Discard/by", json!({"Target": 0}))]
#[case::another_bound("Dread Fugue", "/1/Spell/effects/1/Discard/filter/1/Compare/to", json!(3))]
#[case::another_count("Lantern Flare", "/1/Spell/effects/1/GainLife/amount/Count/0/Type", json!("Land"))]
#[case::cleaved_count("Lantern Flare", "/0/Keyword/Cleave/spell/Spell/effects/0/DealDamage/amount", json!(2))]
#[case::another_lacking_subtype("Heliod, the Radiant Dawn // Heliod, the Warped Eclipse", "/0/Triggered/targets/0/what/Object/1/Not/Subtype", json!("Elf"))]
#[case::having_the_subtype("Heliod, the Radiant Dawn // Heliod, the Warped Eclipse", "/0/Triggered/targets/0/what/Object/1", json!({"Subtype": "God"}))]
#[case::another_hand("Dread Fugue", "/1/Spell/effects/0/Reveal/what/Each/1/OwnedBy", json!("You"))]
fn one_changed_value_is_not_accepted(
    #[case] name: &str,
    #[case] pointer: &str,
    #[case] value: Value,
) {
    assert_ne!(
        verdict(name, pointer, value),
        Verdict::Accepted,
        "{name} {pointer}"
    );
}
