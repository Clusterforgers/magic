//! Goal: each node of the vocabulary pass reads and writes as the same JSON, validates where it is right and is refused where it is wrong, and the support walker names it, so a card that uses one stays Partial until the engine runs it.
#![allow(clippy::unwrap_used)]

use std::collections::BTreeSet;

use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, Source};
use mtg_cards::validate::used_primitives;
use rstest::rstest;

/// Loads a card whose one face has this ability, or says why the library refused it.
fn load(ability: &str) -> Result<(), String> {
    let text = format!(
        r#"{{"oracle_id":"4457ed35-7c10-48c8-9776-456485fdf070","name":"Test","impl_status":"Full","faces":[{{"name":"Test","oracle_fingerprint":"e3b0c44298fc1c149afbf4c8996fb924","abilities":[{ability}]}}]}}"#
    );
    Library::from_sources(&[Source {
        path: "cards/test.json".into(),
        text,
    }])
    .map(|_| ())
    .map_err(|errors| errors.to_string())
}

fn names(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|name| (*name).to_owned()).collect()
}

#[rstest]
#[case::surveil(r#"{"Spell":{"effects":[{"Surveil":{"count":2}}]}}"#, &["Ability.Spell", "Effect.Surveil"])]
#[case::proliferate(r#"{"Spell":{"effects":[{"Proliferate":{}}]}}"#, &["Ability.Spell", "Effect.Proliferate"])]
#[case::goad(
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Goad":{"what":{"Target":0}}}]}}"#,
    &["Ability.Spell", "Effect.Goad"]
)]
#[case::fight(
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}},{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Fight":{"what":{"Target":0},"with":{"Target":1}}}]}}"#,
    &["Ability.Spell", "Effect.Fight"]
)]
#[case::monarch(r#"{"Spell":{"effects":[{"BecomeMonarch":{}}]}}"#, &["Ability.Spell", "Effect.BecomeMonarch"])]
#[case::amass(r#"{"Spell":{"effects":[{"Amass":{"subtype":"Orc","count":2}}]}}"#, &["Ability.Spell", "Effect.Amass"])]
#[case::monstrosity(r#"{"Spell":{"effects":[{"Monstrosity":{"count":3}}]}}"#, &["Ability.Spell", "Effect.Monstrosity"])]
#[case::adapt(r#"{"Spell":{"effects":[{"Adapt":{"count":2}}]}}"#, &["Ability.Spell", "Effect.Adapt"])]
#[case::bolster(r#"{"Spell":{"effects":[{"Bolster":{"count":2}}]}}"#, &["Ability.Spell", "Effect.Bolster"])]
#[case::explore(r#"{"Spell":{"effects":[{"Explore":{"what":"This"}}]}}"#, &["Ability.Spell", "Effect.Explore"])]
#[case::connive(r#"{"Spell":{"effects":[{"Connive":{"what":"This"}}]}}"#, &["Ability.Spell", "Effect.Connive"])]
#[case::exile_until(r#"{"Spell":{"effects":[{"ExileUntil":{"filter":[],"as":"found","exiled_as":"exiled"}}]}}"#, &["Ability.Spell", "Effect.ExileUntil"])]
#[case::cast_from_zone(r#"{"Spell":{"effects":[{"ExileUntil":{"filter":[],"as":"found","exiled_as":"exiled"}},{"CastFromZone":{"what":{"Bound":"found"},"free":true,"otherwise":[{"Move":{"what":{"Bound":"found"},"to":"Hand"}}]}}]}}"#, &["Ability.Spell", "Effect.ExileUntil", "Effect.CastFromZone", "Effect.Move"])]
#[case::time_travel(r#"{"Spell":{"effects":[{"TimeTravel":{}}]}}"#, &["Ability.Spell", "Effect.TimeTravel"])]
#[case::extra_turn(r#"{"Spell":{"effects":[{"ExtraTurn":{}}]}}"#, &["Ability.Spell", "Effect.ExtraTurn"])]
#[case::a_choice_of_mana(
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"OneOf":["White","Blue"]}}}]}}"#,
    &["Ability.Activated", "Ability.ManaAbility.Complex", "Cost.Tap", "Effect.AddMana", "Effect.AddMana.OneOf"]
)]
#[case::mana_of_the_chosen_color(
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"ChosenColor":1}}}]}}"#,
    &["Ability.Activated", "Ability.ManaAbility.Complex", "Cost.Tap", "Effect.AddMana", "Effect.AddMana.ChosenColor"]
)]
#[case::mana_of_the_commanders_identity(
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"CommanderIdentity":1}}}]}}"#,
    &["Ability.Activated", "Ability.ManaAbility.Other", "Cost.Tap", "Effect.AddMana", "Effect.AddMana.CommanderIdentity"]
)]
#[case::mana_a_land_could_produce(
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"ProducedBy":{"among":[{"Type":"Land"},{"ControlledBy":"Opponents"}],"count":1}}}}]}}"#,
    &["Ability.Activated", "Ability.ManaAbility.Other", "Cost.Tap", "Effect.AddMana", "Effect.AddMana.ProducedBy"]
)]
#[case::devotion(
    r#"{"Spell":{"effects":[{"GainLife":{"amount":{"Devotion":["Black"]}}}]}}"#,
    &["Ability.Spell", "Amount.Devotion", "Effect.GainLife"]
)]
#[case::the_greatest_power(
    r#"{"Spell":{"effects":[{"Draw":{"count":{"Greatest":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Power"}}}}]}}"#,
    &["Ability.Spell", "Amount.Greatest", "Effect.Draw"]
)]
#[case::the_total_power(
    r#"{"Spell":{"effects":[{"GainLife":{"amount":{"Total":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Power"}}}}]}}"#,
    &["Ability.Spell", "Amount.Total", "Effect.GainLife"]
)]
#[case::card_types_among_cards(
    r#"{"Spell":{"effects":[{"Draw":{"count":{"CardTypes":[{"In":"Graveyard"},{"OwnedBy":"You"}]}}}]}}"#,
    &["Ability.Spell", "Amount.CardTypes", "Effect.Draw"]
)]
#[case::if_you_do(
    r#"{"Spell":{"effects":[{"May":{"effects":[{"Discard":{}}]}},{"If":{"condition":"Performed","then":[{"Draw":{}}]}}]}}"#,
    &["Ability.Spell", "Condition.Performed", "Effect.Discard", "Effect.Draw", "Effect.If", "Effect.May"]
)]
#[case::if_you_dont(
    r#"{"Spell":{"effects":[{"Discard":{}},{"If":{"condition":{"Not":"Performed"},"then":[{"Draw":{}}]}}]}}"#,
    &["Ability.Spell", "Condition.Performed", "Effect.Discard", "Effect.Draw", "Effect.If"]
)]
#[case::the_monarch(
    r#"{"Spell":{"effects":[{"If":{"condition":{"Monarch":"You"},"then":[{"Draw":{}}]}}]}}"#,
    &["Ability.Spell", "Condition.Monarch", "Effect.Draw", "Effect.If"]
)]
#[case::the_citys_blessing(
    r#"{"Spell":{"effects":[{"If":{"condition":{"CitysBlessing":"You"},"then":[{"Draw":{}}]}}]}}"#,
    &["Ability.Spell", "Condition.CitysBlessing", "Effect.Draw", "Effect.If"]
)]
#[case::paying_energy(
    r#"{"Activated":{"cost":["Tap",{"PayEnergy":2}],"effects":[{"Draw":{}}]}}"#,
    &["Ability.Activated", "Cost.PayEnergy", "Cost.Tap", "Effect.Draw"]
)]
#[case::removing_a_counter(
    r#"{"Activated":{"cost":[{"RemoveCounters":{"counter":"charge"}}],"effects":[{"Draw":{}}]}}"#,
    &["Ability.Activated", "Cost.RemoveCounters", "Effect.Draw"]
)]
#[case::tapping_creatures(
    r#"{"Activated":{"cost":[{"TapPermanents":{"filter":[{"Type":"Creature"},{"ControlledBy":"You"},"Untapped"],"count":2}}],"effects":[{"Draw":{}}]}}"#,
    &["Ability.Activated", "Cost.TapPermanents", "Effect.Draw"]
)]
#[case::exiling_from_the_graveyard(
    r#"{"Activated":{"cost":[{"ExileFromGraveyard":{"filter":[{"Type":"Creature"}]}}],"effects":[{"Draw":{}}]}}"#,
    &["Ability.Activated", "Cost.ExileFromGraveyard", "Effect.Draw"]
)]
#[case::returning_a_permanent(
    r#"{"Activated":{"cost":[{"ReturnToHand":{"filter":[{"Type":"Land"},{"ControlledBy":"You"}]}}],"effects":[{"Draw":{}}]}}"#,
    &["Ability.Activated", "Cost.ReturnToHand", "Effect.Draw"]
)]
#[case::paying_energy_in_a_payment(
    r#"{"Spell":{"effects":[{"Pay":{"cost":[{"PayEnergy":1}],"paid":[{"Draw":{}}]}}]}}"#,
    &["Ability.Spell", "Cost.PayEnergy", "Effect.Draw", "Effect.Pay"]
)]
#[case::dealing_damage(
    r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"Draw":{}}]}}"#,
    &["Ability.Triggered", "Effect.Draw", "Trigger.DealsDamage"]
)]
#[case::gaining_life(
    r#"{"Triggered":{"when":{"LifeGained":{"who":"You"}},"effects":[{"Draw":{}}]}}"#,
    &["Ability.Triggered", "Effect.Draw", "Trigger.LifeGained"]
)]
#[case::losing_life(
    r#"{"Triggered":{"when":{"LifeLost":{"who":"Opponents"}},"effects":[{"Draw":{}}]}}"#,
    &["Ability.Triggered", "Effect.Draw", "Trigger.LifeLost"]
)]
#[case::drawing(
    r#"{"Triggered":{"when":{"Drew":{"who":"You"}},"effects":[{"Draw":{}}]}}"#,
    &["Ability.Triggered", "Effect.Draw", "Trigger.Drew"]
)]
#[case::discarding(
    r#"{"Triggered":{"when":{"Discarded":{"what":[{"Type":"Land"}],"by":"You"}},"effects":[{"Draw":{}}]}}"#,
    &["Ability.Triggered", "Effect.Draw", "Trigger.Discarded"]
)]
#[case::becoming_tapped(
    r#"{"Triggered":{"when":{"Tapped":{"what":["This"]}},"effects":[{"Draw":{}}]}}"#,
    &["Ability.Triggered", "Effect.Draw", "Trigger.Tapped"]
)]
#[case::tapping_for_mana(
    r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Land"}],"by":"You"}},"effects":[{"Draw":{}}]}}"#,
    &["Ability.Triggered", "Effect.Draw", "Trigger.TappedForMana"]
)]
#[case::blocking(
    r#"{"Triggered":{"when":{"Blocks":{"what":["This"]}},"effects":[{"Draw":{}}]}}"#,
    &["Ability.Triggered", "Effect.Draw", "Trigger.Blocks"]
)]
#[case::becoming_blocked(
    r#"{"Triggered":{"when":{"BecomesBlocked":{"what":["This"]}},"effects":[{"Draw":{}}]}}"#,
    &["Ability.Triggered", "Effect.Draw", "Trigger.BecomesBlocked"]
)]
#[case::the_first_attack_of_a_turn(
    r#"{"Triggered":{"when":{"Nth":{"event":{"Attacks":{"what":["This"]}},"nth":1}},"effects":[{"Draw":{}}]}}"#,
    &["Ability.Triggered", "Effect.Draw", "Trigger.Attacks", "Trigger.Nth"]
)]
#[case::the_first_target_of_mine_in_a_turn(
    r#"{"Triggered":{"when":{"Nth":{"event":{"BecomesTarget":{"what":["This"],"by":[{"ControlledBy":"You"}]}},"nth":1}},"effects":[{"Draw":{}}]}}"#,
    &["Ability.Triggered", "Effect.Draw", "Trigger.BecomesTarget", "Trigger.Nth"]
)]
#[case::the_first_target_of_anyone_in_a_turn(
    r#"{"Triggered":{"when":{"Nth":{"event":{"BecomesTarget":{"what":["This"]}},"nth":2}},"effects":[{"Draw":{}}]}}"#,
    &["Ability.Triggered", "Effect.Draw", "Trigger.BecomesTarget", "Trigger.Nth"]
)]
#[case::the_first_damage_to_an_object_in_a_turn(
    r#"{"Triggered":{"when":{"Nth":{"event":{"DealsDamage":{"source":[],"to":"This"}},"nth":1}},"effects":[{"Draw":{}}]}}"#,
    &["Ability.Triggered", "Effect.Draw", "Trigger.DealsDamage", "Trigger.Nth"]
)]
#[case::attacking_together(
    r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"ControlledBy":"You"}],"at_least":2}},"effects":[{"Draw":{}}]}}"#,
    &["Ability.Triggered", "Effect.Draw", "Trigger.GroupAttacks"]
)]
#[case::counters_put(
    r#"{"Triggered":{"when":{"CountersPut":{"what":[{"Type":"Creature"}],"counter":"+1/+1"}},"effects":[{"Draw":{}}]}}"#,
    &["Ability.Triggered", "Effect.Draw", "Trigger.CountersPut"]
)]
#[case::becoming_untapped(
    r#"{"Triggered":{"when":{"Untapped":{"what":[{"Type":"Creature"}]}},"effects":[{"Draw":{}}]}}"#,
    &["Ability.Triggered", "Effect.Draw", "Trigger.Untapped"]
)]
#[case::counters_removed(
    r#"{"Triggered":{"when":{"CountersRemoved":{"what":["This"],"counter":"+1/+1"}},"effects":[{"Draw":{}}]}}"#,
    &["Ability.Triggered", "Effect.Draw", "Trigger.CountersRemoved"]
)]
#[case::becoming_attached(
    r#"{"Triggered":{"when":{"Attached":{"what":[{"Subtype":"Aura"}],"to":["This"]}},"effects":[{"Draw":{}}]}}"#,
    &["Ability.Triggered", "Effect.Draw", "Trigger.Attached"]
)]
#[case::cycling_a_card(
    r#"{"Triggered":{"when":{"Cycled":{"what":["This"],"by":"You"}},"from":"Graveyard","effects":[{"Draw":{}}]}}"#,
    &["Ability.Triggered", "Ability.From", "Effect.Draw", "Trigger.Cycled"]
)]
#[case::one_of_several_events(
    r#"{"Triggered":{"when":{"AnyOf":[{"Drew":{"who":"You"}},{"LifeGained":{"who":"You"}}]},"effects":[{"Draw":{}}]}}"#,
    &["Ability.Triggered", "Effect.Draw", "Trigger.AnyOf", "Trigger.Drew", "Trigger.LifeGained"]
)]
fn a_node_validates_round_trips_and_names_its_primitive(
    #[case] json: &str,
    #[case] expected: &[&str],
) {
    assert_eq!(load(json), Ok(()));
    let ability: Ability = serde_json::from_str(json).unwrap();
    assert_eq!(serde_json::to_string(&ability).unwrap(), json);
    assert_eq!(used_primitives(&ability), names(expected));
}

#[rstest]
#[case::if_you_do_with_nothing_before(
    r#"{"Spell":{"effects":[{"If":{"condition":"Performed","then":[{"Draw":{}}]}}]}}"#,
    "`Performed` is the condition of an `If` that follows"
)]
#[case::performed_outside_an_if(
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep"}},"if":"Performed","effects":[{"Draw":{}}]}}"#,
    "`Performed` is the condition of an `If` that follows"
)]
#[case::performed_in_the_first_branch_of_a_nested_list(
    r#"{"Spell":{"effects":[{"May":{"effects":[{"If":{"condition":"Performed","then":[{"Draw":{}}]}}]}}]}}"#,
    "`Performed` is the condition of an `If` that follows"
)]
#[case::devotion_to_nothing(
    r#"{"Spell":{"effects":[{"GainLife":{"amount":{"Devotion":[]}}}]}}"#,
    "devotion is to at least one color"
)]
#[case::a_choice_of_one_mana(
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"OneOf":["White"]}}}]}}"#,
    "a choice of mana needs at least two types"
)]
#[case::no_chosen_mana(
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"ChosenColor":0}}}]}}"#,
    "adds no mana"
)]
#[case::no_mana_a_land_could_produce(
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"ProducedBy":{"among":[],"count":0}}}}]}}"#,
    "adds no mana"
)]
#[case::the_monarch_is_a_player(
    r#"{"Spell":{"effects":[{"If":{"condition":{"Monarch":"This"},"then":[{"Draw":{}}]}}]}}"#,
    "expected a player"
)]
#[case::a_player_does_not_fight(
    r#"{"Spell":{"effects":[{"Fight":{"what":"You","with":"This"}}]}}"#,
    "expected an object"
)]
#[case::an_object_does_not_surveil(
    r#"{"Spell":{"effects":[{"Surveil":{"who":"This"}}]}}"#,
    "expected a player"
)]
#[case::removing_counters_from_a_source_that_is_not_on_the_battlefield(
    r#"{"Activated":{"cost":[{"RemoveCounters":{"counter":"charge"}}],"from":"Graveyard","effects":[{"Draw":{}}]}}"#,
    "`RemoveCounters` is paid from the battlefield"
)]
#[case::an_amount_in_a_cost_is_checked(
    r#"{"Activated":{"cost":[{"PayEnergy":{"Life":"This"}}],"effects":[{"Draw":{}}]}}"#,
    "expected a player"
)]
#[case::a_player_who_gains_life_is_a_player(
    r#"{"Triggered":{"when":{"LifeGained":{"who":"This"}},"effects":[{"Draw":{}}]}}"#,
    "expected a player"
)]
#[case::a_player_who_cycles_is_a_player(
    r#"{"Triggered":{"when":{"Cycled":{"by":"This"}},"effects":[{"Draw":{}}]}}"#,
    "expected a player"
)]
#[case::the_first_time_is_one(
    r#"{"Triggered":{"when":{"Nth":{"event":{"Attacks":{"what":["This"]}},"nth":0}},"effects":[{"Draw":{}}]}}"#,
    "the first time is 1"
)]
#[case::a_count_of_targets_by_an_opponent(
    r#"{"Triggered":{"when":{"Nth":{"event":{"BecomesTarget":{"what":["This"],"by":[{"ControlledBy":"Opponents"}]}},"nth":1}},"effects":[{"Draw":{}}]}}"#,
    "are counted in a turn"
)]
#[case::a_count_of_targets_by_a_kind_of_spell(
    r#"{"Triggered":{"when":{"Nth":{"event":{"BecomesTarget":{"what":["This"],"by":["Spell"]}},"nth":1}},"effects":[{"Draw":{}}]}}"#,
    "are counted in a turn"
)]
#[case::a_count_of_damage_to_a_player(
    r#"{"Triggered":{"when":{"Nth":{"event":{"DealsDamage":{"source":[],"to":"Opponents"}},"nth":1}},"effects":[{"Draw":{}}]}}"#,
    "are counted in a turn"
)]
#[case::a_count_of_damage_from_one_source(
    r#"{"Triggered":{"when":{"Nth":{"event":{"DealsDamage":{"source":["This"],"to":"This"}},"nth":1}},"effects":[{"Draw":{}}]}}"#,
    "are counted in a turn"
)]
#[case::a_count_of_combat_damage_only(
    r#"{"Triggered":{"when":{"Nth":{"event":{"DealsDamage":{"source":[],"to":"This","combat":true}},"nth":1}},"effects":[{"Draw":{}}]}}"#,
    "are counted in a turn"
)]
#[case::a_count_of_something_else(
    r#"{"Triggered":{"when":{"Nth":{"event":{"Blocks":{"what":["This"]}},"nth":1}},"effects":[{"Draw":{}}]}}"#,
    "are counted in a turn"
)]
#[case::no_attackers_at_all(
    r#"{"Triggered":{"when":{"GroupAttacks":{"what":[],"at_least":0}},"effects":[{"Draw":{}}]}}"#,
    "at least one creature attacks"
)]
#[case::nothing_triggering_in_a_state(
    r#"{"Triggered":{"when":{"AnyOf":[{"State":{"condition":"YourTurn"}},{"Drew":{}}]},"effects":[{"Draw":{}}]}}"#,
    "a state is the whole pattern of a triggered ability"
)]
#[case::a_target_that_is_not_there(
    r#"{"Spell":{"effects":[{"Goad":{"what":{"Target":0}}}]}}"#,
    "target 0 does not exist"
)]
fn a_node_used_wrongly_is_refused_where_it_stands(#[case] json: &str, #[case] message: &str) {
    let error = load(json).unwrap_err();
    assert!(error.contains(message), "{message:?} is not in {error}");
}
