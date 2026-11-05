//! Goal: round 7 piece 5 reads the expression words of many cards at once: the cards a permanent exiled, the top card of the library, the player a damage trigger names, the number of times an ability resolved, and the valid properties of a basic land type or a cast from a hand; a word it does not know stays a gap by name.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn spell(body: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {body}\n")
}

fn creature(lines: &str) -> String {
    format!("Name:T\nTypes:Creature Rogue\nPT:1/1\n{lines}\n")
}

#[rstest]
#[case::exiled_with(
    "ChangeZone | Defined$ ExiledWith | Origin$ Exile | Destination$ Hand",
    json!({"Move": {"to": "Hand", "what": {"Each": [{"ExiledWith": "This"}, {"In": "Exile"}]}}})
)]
#[case::top_of_library(
    "ChangeZone | Defined$ TopOfLibrary | Origin$ Library | Destination$ Graveyard",
    json!({"Move": {"to": "Graveyard", "what": {"Each": [{"In": "Library"}, {"OwnedBy": "You"}, "TopOfLibrary"]}}})
)]
fn a_defined_word_names_the_cards_it_stands_for(#[case] line: &str, #[case] effect: Value) {
    let text = spell(line);
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(abilities(&text)[0]["Spell"]["effects"][0], effect);
}

#[rstest]
#[case::life(
    "LifeTotal/HalfUp",
    json!({"Half": {"of": {"Life": "Triggering"}, "up": true}})
)]
#[case::hand("CardsInHand", json!({"Count": [{"In": "Hand"}, {"OwnedBy": "Triggering"}]}))]
#[case::poison(
    "Counters.Poison",
    json!({"Counters": {"on": "Triggering", "counter": "poison"}})
)]
fn a_stat_of_the_player_a_damage_trigger_names(#[case] what: &str, #[case] amount: Value) {
    let text = creature(&format!(
        "T:Mode$ DamageDone | ValidSource$ Card.Self | ValidTarget$ Player | CombatDamage$ True | Execute$ L\nSVar:L:DB$ LoseLife | Defined$ TriggeredTarget | LifeAmount$ X\nSVar:X:TriggeredTarget${what}"
    ));
    assert_eq!(gaps(&text), Vec::<String>::new());
    let effects = &abilities(&text)[0]["Triggered"]["effects"][0]["LoseLife"];
    assert_eq!(effects["amount"], amount);
}

#[rstest]
#[case::a_creature_may_be_the_target(
    "ValidTarget$ Creature",
    "LifeTotal",
    "amount:TriggeredTarget"
)]
#[case::an_unknown_stat("ValidTarget$ Player", "Bogus", "amount:TriggeredTarget$Bogus")]
#[case::an_unknown_counter(
    "ValidTarget$ Player",
    "Counters.ALL",
    "amount:TriggeredTarget$Counters.ALL"
)]
fn a_triggered_target_that_is_no_player_or_stat_unknown_is_a_gap(
    #[case] target: &str,
    #[case] what: &str,
    #[case] gap: &str,
) {
    let text = creature(&format!(
        "T:Mode$ DamageDone | ValidSource$ Card.Self | {target} | Execute$ L\nSVar:L:DB$ LoseLife | LifeAmount$ X\nSVar:X:TriggeredTarget${what}"
    ));
    assert_eq!(gaps(&text), [gap]);
}

#[test]
fn the_times_an_ability_resolved_is_a_number_the_ability_reads() {
    let text = creature(
        "A:AB$ Pump | Cost$ 2 | Defined$ Self | NumAtt$ +X\nSVar:X:Count$ResolvedThisTurn",
    );
    assert_eq!(gaps(&text), Vec::<String>::new());
    let effect = &abilities(&text)[0]["Activated"]["effects"][0];
    assert_eq!(
        effect["Continuous"]["mods"][0]["ModifyPT"]["power"],
        json!("ResolvedThisTurn")
    );
}

#[rstest]
#[case::basic_land_type(
    "Land.hasABasicLandType",
    json!([{"Type": "Land"}, {"AnyOf": [{"Subtype": "Plains"}, {"Subtype": "Island"}, {"Subtype": "Swamp"}, {"Subtype": "Mountain"}, {"Subtype": "Forest"}]}])
)]
#[case::cast_from_their_hand(
    "Card.wasCastFromTheirHand",
    json!([{"WasCast": {"from": "Hand"}}])
)]
#[case::not_cast_from_their_hand(
    "Card.!wasCastFromTheirHand",
    json!([{"Not": {"WasCast": {"from": "Hand"}}}])
)]
fn a_valid_property_is_read(#[case] valid: &str, #[case] filter: Value) {
    let text = spell(&format!(
        "ChangeZone | Origin$ Library | Destination$ Hand | ChangeType$ {valid}"
    ));
    let found = gaps(&text);
    assert_eq!(found, Vec::<String>::new());
    let shown = abilities(&text)[0].to_string();
    for pred in filter.as_array().unwrap() {
        assert!(shown.contains(&pred.to_string()), "{pred} in {shown}");
    }
}

#[rstest]
#[case::unknown_basic_word("Land.hasAnImaginaryType", "valid:hasAnImaginaryType")]
#[case::unknown_cast_zone("Card.wasCastFromTheirMind", "valid:wasCastFromTheirMind")]
fn a_valid_property_it_does_not_know_stays_a_gap(#[case] valid: &str, #[case] gap: &str) {
    let text = spell(&format!(
        "ChangeZone | Origin$ Library | Destination$ Hand | ChangeType$ {valid}"
    ));
    assert_eq!(gaps(&text), [gap]);
}

#[test]
fn the_enchanted_player_is_the_player_the_aura_is_attached_to() {
    let text = "Name:T\nTypes:Enchantment Aura Curse\nK:Enchant:Player\nT:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ Player.EnchantedBy | TriggerZones$ Battlefield | Execute$ M\nSVar:M:DB$ Mill | Defined$ Player.EnchantedBy | NumCards$ 2\n";
    assert_eq!(gaps(text), Vec::<String>::new());
    assert!(
        abilities(text)[1]
            .to_string()
            .contains(r#""Mill":{"count":2,"who":"AttachedPlayer"}"#)
    );
}
