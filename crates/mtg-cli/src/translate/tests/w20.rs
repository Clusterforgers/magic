//! Goal: pack w20's Forge lines: a count of the cards an optional step imprinted reads as the step's own choice (Fishing Gear, Unexpected Results), an effect line of the imprinted cards inside the condition that took them reads on (Rotisserie Elemental), a charm mode named by the description of its sub-ability keeps the name (What Must Be Done), and "historic" is written in the order the writers know; the objects an optional step imprinted stay unusable after it.
use serde_json::{Value, json};

use super::{abilities, gaps};

const FISHING_GEAR: &str = "Name:Fishing Gear\nTypes:Artifact Equipment\nK:Equip:2\nT:Mode$ DamageDone | ValidSource$ Creature.EquippedBy | ValidTarget$ Player | CombatDamage$ True | Execute$ TrigExile | TriggerZones$ Battlefield | TriggerDescription$ X.\nSVar:TrigExile:DB$ Dig | Defined$ TriggeredTarget | DigNum$ 1 | ChangeNum$ All | DestinationZone$ Exile | RememberChanged$ True | SubAbility$ DBChange\nSVar:DBChange:DB$ ChangeZone | ConditionDefined$ Remembered | ConditionPresent$ Permanent | GainControl$ True | Defined$ Remembered | Origin$ Exile | Optional$ True | Imprint$ True | Destination$ Battlefield | SubAbility$ DBToken\nSVar:DBToken:DB$ Draw | NumCards$ 1 | ConditionDefined$ Imprinted | ConditionPresent$ Card | ConditionCompare$ EQ0 | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearImprinted$ True | ClearRemembered$ True\n";

const ROTISSERIE: &str = "Name:Rotisserie Elemental\nTypes:Creature Elemental\nPT:4/4\nT:Mode$ DamageDone | ValidSource$ Card.Self | ValidTarget$ Player | CombatDamage$ True | Execute$ TrigPutCounter | TriggerZones$ Battlefield | TriggerDescription$ X.\nSVar:TrigPutCounter:DB$ PutCounter | Defined$ Self | CounterType$ SKEWER | CounterNum$ 1 | SubAbility$ DBMaySac\nSVar:DBMaySac:DB$ Sacrifice | SacValid$ Card.Self | RememberSacrificed$ True | Optional$ True | SubAbility$ DBExile\nSVar:DBExile:DB$ Dig | ConditionDefined$ Remembered | ConditionPresent$ Card | Defined$ You | DestinationZone$ Exile | DigNum$ X | ChangeNum$ All | Imprint$ True | SubAbility$ DBEffect\nSVar:DBEffect:DB$ Effect | RememberObjects$ Imprinted | StaticAbilities$ Play | SubAbility$ DBCleanup | ForgetOnMoved$ Exile\nSVar:Play:Mode$ Continuous | MayPlay$ True | Affected$ Card.IsRemembered | AffectedZone$ Exile | Description$ You may play those cards this turn.\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True | ClearImprinted$ True\nSVar:X:Count$CardCounters.SKEWER\n";

const WHAT_MUST_BE_DONE: &str = "Name:What Must Be Done\nTypes:Sorcery\nA:SP$ Charm | Choices$ DBDestroyAll,DBConditionEffect\nSVar:DBDestroyAll:DB$ DestroyAll | ValidCards$ Artifact,Creature | SpellDescription$ Let the World Burn \u{2014} Destroy all artifacts and creatures.\nSVar:DBConditionEffect:DB$ Effect | RememberObjects$ Targeted & Self | ReplacementEffects$ ETBCreat | ExileOnMoved$ Graveyard,Stack | SubAbility$ DBChangeZone\nSVar:DBChangeZone:DB$ ChangeZone | Origin$ Graveyard | Destination$ Battlefield | ValidTgts$ Permanent.Historic+YouOwn | TgtPrompt$ Select target historic permanent card | SpellDescription$ Release Juno \u{2014} Return target historic permanent card from your graveyard to the battlefield.\nSVar:ETBCreat:Event$ Moved | ValidCard$ Creature.IsRemembered | Destination$ Battlefield | ReplaceWith$ DBPutP1P1 | ReplacementResult$ Updated | Description$ X.\nSVar:DBPutP1P1:DB$ PutCounter | Defined$ ReplacedCard | CounterType$ P1P1 | ETB$ True | CounterNum$ 2\n";

#[test]
fn the_pack_cards_have_no_gaps() {
    for script in [FISHING_GEAR, ROTISSERIE, WHAT_MUST_BE_DONE] {
        assert_eq!(gaps(script), Vec::<String>::new(), "{script}");
    }
}

#[test]
fn the_second_mode_takes_the_name_of_its_sub_ability() {
    let modes = &abilities(WHAT_MUST_BE_DONE)[0]["Spell"]["effects"][0]["Modal"]["modes"];
    assert_eq!(modes[0]["label"], json!("Let the World Burn"));
    assert_eq!(modes[1]["label"], json!("Release Juno"));
    // Historic in the order the writers know: an artifact, a legendary object or a Saga.
    assert_eq!(
        modes[1]["targets"][0]["what"]["Object"][0],
        json!({"AnyOf": [{"Type": "Artifact"}, {"Supertype": "Legendary"}, {"Subtype": "Saga"}]})
    );
}

#[test]
fn a_step_that_follows_the_imprint_in_the_condition_that_took_it_reads_the_imprint() {
    let ability: &Value = &abilities(ROTISSERIE)[0];
    let then = &ability["Triggered"]["effects"][2]["If"]["then"];
    assert_eq!(
        then[0]["ExileTop"]["as"],
        then[1]["Rule"]["rule"]["MayCast"]["only"]["Bound"]
    );
}

/// The token step of Fishing Gear counts the imprinted cards; this step moves them, which an optional step that may not
/// have run cannot give.
fn moves_the_imprint() -> String {
    FISHING_GEAR.replace(
        "DB$ Draw | NumCards$ 1 |",
        "DB$ ChangeZone | Defined$ Imprinted | Origin$ Battlefield | Destination$ Hand |",
    )
}

#[test]
fn the_cards_an_optional_step_imprinted_cannot_be_used_after_it() {
    assert_eq!(
        gaps(&moves_the_imprint()),
        vec!["defined:Imprinted-without-a-change".to_owned()]
    );
}

#[test]
fn a_count_of_the_imprint_is_a_condition_and_nothing_else() {
    let effects = &abilities(FISHING_GEAR)[1]["Triggered"]["effects"];
    assert_eq!(effects[2]["If"]["condition"]["CountOf"]["op"], json!("Eq"));
    assert_eq!(effects[2]["If"]["condition"]["CountOf"]["right"], json!(0));
}
