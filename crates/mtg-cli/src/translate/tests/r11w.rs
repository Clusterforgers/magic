//! Goal: "sacrifice ~" read from a SacrificeAll of the card itself (Self, or EffectSource in an effect's delayed trigger)
//! is sacrificed by the ability's controller (CR 701.21a), while a SacrificeAll of other named objects stays with their
//! controllers.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::abilities;

const GUARD: &str = "Name:Kjeldoran Elite Guard\nTypes:Creature Human Soldier\nPT:2/2\nA:AB$ Pump | Cost$ T | NumAtt$ +2 | NumDef$ +2 | ValidTgts$ Creature | ActivationPhases$ BeginCombat->EndCombat | SubAbility$ E | SpellDescription$ Target creature gets +2/+2 until end of turn. When that creature leaves the battlefield this turn, sacrifice CARDNAME. Activate only during combat.\nSVar:E:DB$ Effect | Triggers$ Lost | RememberObjects$ Targeted | ExileOnMoved$ Battlefield\nSVar:Lost:Mode$ ChangesZone | ValidCard$ Card.IsRemembered | Origin$ Battlefield | Destination$ Any | Execute$ S | TriggerDescription$ When the targeted creature leaves the battlefield this turn, sacrifice EFFECTSOURCE.\nSVar:S:DB$ SacrificeAll | Defined$ EffectSource\nOracle:{T}: Target creature gets +2/+2 until end of turn. When that creature leaves the battlefield this turn, sacrifice Kjeldoran Elite Guard. Activate only during combat.\n";

fn upkeep(defined: &str) -> String {
    format!(
        "Name:Subject\nTypes:Creature Beast\nPT:2/2\nT:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ S | TriggerDescription$ At the beginning of your upkeep, sacrifice CARDNAME.\nSVar:S:DB$ SacrificeAll | Defined$ {defined}\nOracle:At the beginning of your upkeep, sacrifice Subject.\n"
    )
}

#[rstest]
#[case::delayed_effect_source(GUARD.to_owned(), r#"{"Sacrifice":{"filter":["This"]}}"#)]
#[case::self_in_a_trigger(upkeep("Self"), r#"{"Sacrifice":{"filter":["This"]}}"#)]
#[case::enchanted_stays_with_its_controller(
    upkeep("Enchanted"),
    r#"{"Sacrifice":{"filter":[{"Is":"Attached"}],"who":{"ControllerOf":"Attached"}}}"#
)]
fn the_card_itself_is_sacrificed_by_the_ability_controller(
    #[case] text: String,
    #[case] wanted: &str,
) {
    let json = serde_json::to_string(&abilities(&text)).unwrap();
    assert!(json.contains(wanted), "{json}");
}
