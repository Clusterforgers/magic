//! Goal: the properties of Forge's valid-strings map to selector predicates, and a count over a valid-string reduces to the
//! right characteristic; a property or negation the DSL cannot express leaves the line a gap and never a wider selector.
use mtg_cards::refs::Selector;
use mtg_cards::value::Amount;
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

#[rstest]
#[case::kicked("Card.kicked", json!(["Kicked"]))]
#[case::outlaw(
    "Creature.Outlaw",
    json!([{"Type": "Creature"}, {"AnyOf": [{"Subtype": "Assassin"}, {"Subtype": "Mercenary"}, {"Subtype": "Pirate"}, {"Subtype": "Rogue"}, {"Subtype": "Warlock"}]}])
)]
#[case::attacked("Creature.attackedThisTurn", json!([{"Type": "Creature"}, "AttackedThisTurn"]))]
#[case::defender(
    "Creature.DefenderCtrl",
    json!([{"Type": "Creature"}, {"ControlledBy": "Defending"}])
)]
#[case::active(
    "Creature.ActivePlayerCtrl",
    json!([{"Type": "Creature"}, {"ControlledBy": "ActivePlayer"}])
)]
#[case::team("Creature.YourTeamCtrl", json!([{"Type": "Creature"}, {"ControlledBy": "You"}]))]
#[case::in_zone("Creature.inZoneBattlefield", json!([{"Type": "Creature"}, {"In": "Battlefield"}]))]
#[case::in_graveyard("Creature.inZoneGraveyard", json!([{"Type": "Creature"}, {"In": "Graveyard"}]))]
#[case::controlled_by_the_defender(
    "Creature.ControlledBy TriggeredDefendingPlayer",
    json!([{"Type": "Creature"}, {"ControlledBy": "Defending"}])
)]
#[case::shares_a_type(
    "Creature.Other+sharesCreatureTypeWith",
    json!([{"Type": "Creature"}, "Other", {"SharesSubtype": "This"}])
)]
#[case::negated_pair(
    "Creature.!wasCastFromYourHand",
    json!([{"Type": "Creature"}, {"AnyOf": [{"Not": {"WasCast": {"from": "Hand"}}}, {"Not": {"OwnedBy": "You"}}]}])
)]
#[case::bound_by_a_part(
    "Creature.cmcLEX",
    json!([{"Type": "Creature"}, {"Compare": {"stat": "ManaValue", "op": "Le", "to": "X"}}])
)]
fn properties_map(#[case] spec: &str, #[case] expected: Value) {
    let found = {
        let script = format!(
            "{}S:Mode$ Continuous | Affected$ {spec} | AddPower$ 1\nSVar:X:Count$xPaid\n",
            "Name:T\nTypes:Creature Elf\n"
        );
        let found = abilities(&script);
        assert_eq!(found.len(), 1, "{:?}", gaps(&script));
        found[0]["Static"]["affected"]["Each"].clone()
    };
    assert_eq!(found, expected);
    assert!(serde_json::from_value::<Selector>(found).is_ok());
}

#[rstest]
#[case::saddled("Card.Self+IsSaddled", "valid:IsSaddled")]
#[case::controlled_by_a_card(
    "Creature.ControlledBy TriggeredTarget",
    "valid:ControlledBy TriggeredTarget"
)]
#[case::targeted_player_without_a_target(
    "Creature.TargetedPlayerCtrl",
    "defined:TargetedPlayer-without-a-player-target"
)]
#[case::unknown_inside_a_negation("Creature.!bogusProperty", "valid:bogusProperty")]
fn what_the_dsl_cannot_select_is_a_gap(#[case] spec: &str, #[case] gap: &str) {
    let script = format!(
        "Name:T\nTypes:Creature Elf\nS:Mode$ Continuous | Affected$ {spec} | AddPower$ 1\n"
    );
    assert_eq!(gaps(&script), vec![gap.to_owned()]);
}

#[rstest]
#[case::strongest(
    "Valid Creature.YouCtrl$GreatestCardPower",
    json!({"Greatest": {"of": [{"Type": "Creature"}, {"ControlledBy": "You"}], "stat": "Power"}})
)]
#[case::total_power(
    "Valid Creature.attacking$CardPower",
    json!({"Total": {"of": [{"Type": "Creature"}, "Attacking"], "stat": "Power"}})
)]
#[case::different_costs(
    "ValidGraveyard Card.YouOwn$DifferentCardManaCost",
    json!({"Distinct": {"of": [{"OwnedBy": "You"}, {"In": "Graveyard"}], "stat": "ManaValue"}})
)]
#[case::names(
    "Valid Land.YouCtrl$DifferentCardNames",
    json!({"Distinct": {"of": [{"Type": "Land"}, {"ControlledBy": "You"}], "stat": "Name"}})
)]
#[case::colors(
    "Valid Permanent.YouCtrl$Colors",
    json!({"Distinct": {"of": [{"ControlledBy": "You"}], "stat": "Colors"}})
)]
fn a_count_over_a_valid_string_reduces_to_a_characteristic(
    #[case] count: &str,
    #[case] expected: Value,
) {
    let script = format!("Name:T\nTypes:Sorcery\nA:SP$ Draw | NumCards$ X\nSVar:X:Count${count}\n");
    let found = abilities(&script);
    let amount = found[0]["Spell"]["effects"][0]["Draw"]["count"].clone();
    assert_eq!(amount, expected);
    assert!(serde_json::from_value::<Amount>(amount).is_ok());
}

#[rstest]
#[case::other_players("LoseLife | Defined$ Player.Other | LifeAmount$ 2", r#""Opponents""#)]
#[case::every_creature_you_control(
    "PutCounter | Defined$ Valid Creature.YouCtrl | CounterType$ P1P1 | CounterNum$ 1",
    r#"{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#
)]
fn defined_words_name_players_and_sets(#[case] effect: &str, #[case] expected: &str) {
    let script = format!("Name:T\nTypes:Sorcery\nA:SP$ {effect}\n");
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    assert!(found[0].to_string().contains(expected), "{}", found[0]);
}

#[rstest]
#[case::same_name("sharesNameWith Targeted", r#"{"SharesName":{"Target":0}}"#)]
#[case::same_type("sharesCardTypeWith Targeted", r#"{"SharesType":{"Target":0}}"#)]
#[case::same_creature_type("sharesCreatureTypeWith Targeted", r#"{"SharesSubtype":{"Target":0}}"#)]
#[case::not_the_target("NotDefinedTargeted", r#"{"Not":{"Is":{"Target":0}}}"#)]
fn a_property_that_reads_the_target_names_it(#[case] prop: &str, #[case] expected: &str) {
    let script = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ Destroy | ValidTgts$ Creature | SubAbility$ DBAll\nSVar:DBAll:DB$ DestroyAll | ValidCards$ Creature.{prop}\n"
    );
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    assert!(found[0].to_string().contains(expected), "{}", found[0]);
}
