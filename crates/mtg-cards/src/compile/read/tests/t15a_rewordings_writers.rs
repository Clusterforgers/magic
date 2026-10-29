//! Goal: the rewords of the popular-card sweep give the oracle wording of a delayed next main phase, a card put on top after a search, a payment an opponent declines, a tapped permanent that deals damage and a chosen color for an amount of mana, and a wrong player, cost, amount, kind or step never reads back.
use rstest::rstest;

use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn t15a_written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const T15A_DRAIN: &str = r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"Counter":{"what":{"Target":0}}},{"DelayedTrigger":{"when":{"AnyOf":[{"StepBegins":{"step":"Main1","whose":"You"}},{"StepBegins":{"step":"Main2","whose":"You"}}]},"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]},"times":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}}}}]}}]}}"#;
const T15A_DRAIN_COMBAT: &str = r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"Counter":{"what":{"Target":0}}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]},"times":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}}}}]}}]}}"#;
const T15A_TUTOR: &str = r#"{"Spell":{"effects":[{"Search":{"filter":[{"Type":"Creature"}],"reveal":true,"as":"found"}},{"Shuffle":{}},{"Move":{"what":{"Bound":"found"},"to":{"Library":{"place":"Top"}}}}]}}"#;
const T15A_TUTOR_BOTTOM: &str = r#"{"Spell":{"effects":[{"Search":{"filter":[{"Type":"Creature"}],"reveal":true,"as":"found"}},{"Shuffle":{}},{"Move":{"what":{"Bound":"found"},"to":{"Library":{"place":"Bottom"}}}}]}}"#;
const T15A_TITHE: &str = r#"{"Triggered":{"when":{"Drew":{"who":"Opponents"}},"effects":[{"Pay":{"who":"Triggering","cost":"{2}","unpaid":[{"CreateToken":{"token":{"Named":"treasure"}}}]}}]}}"#;
const T15A_TITHE_YOU: &str = r#"{"Triggered":{"when":{"Drew":{"who":"You"}},"effects":[{"Pay":{"who":"Triggering","cost":"{2}","unpaid":[{"CreateToken":{"token":{"Named":"treasure"}}}]}}]}}"#;
const T15A_VAULT: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Draw","whose":"You"}},"if":{"Matches":{"subject":"This","selector":["Tapped"]}},"effects":[{"DealDamage":{"source":"This","to":"You","amount":1}}]}}"#;
const T15A_VAULT_UNTAPPED: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Draw","whose":"You"}},"if":{"Matches":{"subject":"This","selector":["Untapped"]}},"effects":[{"DealDamage":{"source":"This","to":"You","amount":1}}]}}"#;
const T15A_CITY: &str = r#"{"Activated":{"cost":[{"Mana":"{2}"},"Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},"ChosenSubtype"]}}}}]}}"#;
const T15A_CITY_LANDS: &str = r#"{"Activated":{"cost":[{"Mana":"{2}"},"Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}}}}]}}"#;

#[rstest]
#[case::next_main_phase(
    T15A_DRAIN,
    "Counter target spell. At the beginning of your next main phase, add an amount of {C} equal to that spell's mana value.",
    true
)]
#[case::another_step(
    T15A_DRAIN_COMBAT,
    "Counter target spell. At the beginning of your next main phase, add an amount of {C} equal to that spell's mana value.",
    false
)]
#[case::next_main_phase_alone_is_no_clause(
    T15A_DRAIN,
    "At the beginning of your next main phase, add an amount of {C} equal to that spell's mana value.",
    false
)]
#[case::the_card_on_top(
    T15A_TUTOR,
    "Search your library for a creature card, reveal it, then shuffle and put the card on top.",
    true
)]
#[case::another_place(
    T15A_TUTOR_BOTTOM,
    "Search your library for a creature card, reveal it, then shuffle and put the card on top.",
    false
)]
#[case::another_kind(
    T15A_TUTOR,
    "Search your library for a land card, reveal it, then shuffle and put the card on top.",
    false
)]
#[case::declined_payment(
    T15A_TITHE,
    "Whenever an opponent draws a card, that player may pay {2}. If the player doesn't, you create a Treasure token.",
    true
)]
#[case::another_cost(
    T15A_TITHE,
    "Whenever an opponent draws a card, that player may pay {3}. If the player doesn't, you create a Treasure token.",
    false
)]
#[case::another_player(
    T15A_TITHE_YOU,
    "Whenever an opponent draws a card, that player may pay {2}. If the player doesn't, you create a Treasure token.",
    false
)]
#[case::another_effect(
    T15A_TITHE,
    "Whenever an opponent draws a card, that player may pay {2}. If the player doesn't, you draw a card.",
    false
)]
#[case::tapped_deals(
    T15A_VAULT,
    "At the beginning of your draw step, if this artifact is tapped, it deals 1 damage to you.",
    true
)]
#[case::untapped_is_another_state(
    T15A_VAULT_UNTAPPED,
    "At the beginning of your draw step, if this artifact is tapped, it deals 1 damage to you.",
    false
)]
#[case::another_damage(
    T15A_VAULT,
    "At the beginning of your draw step, if this artifact is tapped, it deals 2 damage to you.",
    false
)]
#[case::chosen_color_amount(
    T15A_CITY,
    "{2}, {T}: Choose a color. Add an amount of mana of that color equal to the number of creatures you control of the chosen type.",
    true
)]
#[case::another_count(
    T15A_CITY_LANDS,
    "{2}, {T}: Choose a color. Add an amount of mana of that color equal to the number of creatures you control of the chosen type.",
    false
)]
fn t15a_writes_the_oracle_wording(#[case] json: &str, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(t15a_written(json, clause), reads, "{clause}");
}

/// A trigger of three events whose second and third are given.
fn t15a_konrad(second: &str, third: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"AnyOf":[{{"ZoneChange":{{"what":["Other",{{"Type":"Creature"}}],"from":"Battlefield","to":"Graveyard"}}}},{second},{third}]}},"effects":[{{"DealDamage":{{"to":"Opponents","amount":1}}}}]}}}}"#
    )
}

const T15A_PUT: &str = r#"{"ZoneChangeExcept":{"what":[{"Type":"Creature"}],"except":"Battlefield","to":"Graveyard"}}"#;
const T15A_LEAVES: &str =
    r#"{"ZoneChange":{"what":[{"Type":"Creature"},{"OwnedBy":"You"}],"from":"Graveyard"}}"#;
const T15A_LEAVES_LAND: &str =
    r#"{"ZoneChange":{"what":[{"Type":"Land"},{"OwnedBy":"You"}],"from":"Graveyard"}}"#;
const T15A_LEAVES_THEIRS: &str =
    r#"{"ZoneChange":{"what":[{"Type":"Creature"},{"OwnedBy":"Opponents"}],"from":"Graveyard"}}"#;
const T15A_KONRAD: &str = "Whenever another creature dies, or a creature card is put into a graveyard from anywhere other than the battlefield, or a creature card leaves your graveyard, ~ deals 1 damage to each opponent.";

#[rstest]
#[case::three_events(T15A_PUT, T15A_LEAVES, T15A_KONRAD, true)]
#[case::another_card_type_leaves(T15A_PUT, T15A_LEAVES_LAND, T15A_KONRAD, false)]
#[case::another_owner_leaves(T15A_PUT, T15A_LEAVES_THEIRS, T15A_KONRAD, false)]
#[case::a_second_event_twice(T15A_PUT, T15A_PUT, T15A_KONRAD, false)]
#[case::another_damage(
    T15A_PUT,
    T15A_LEAVES,
    "Whenever another creature dies, or a creature card is put into a graveyard from anywhere other than the battlefield, or a creature card leaves your graveyard, ~ deals 2 damage to each opponent.",
    false
)]
fn t15a_three_events_are_named_in_one_clause(
    #[case] second: &str,
    #[case] third: &str,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(
        t15a_written(&t15a_konrad(second, third), clause),
        reads,
        "{clause}"
    );
}
