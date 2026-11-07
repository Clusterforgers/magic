//! Goal: round 15 pack e15c reads the spell's own optional additional cost (`Mode$ OptionalCost`) and what its text asks
//! of it (`Condition$ OptionalCost`, `OptionalGenericCostPaid`, "if evidence was collected"), "isn't a creature" lines
//! (`RemoveType$`), and "behold and exile" (`BeholdExile`), and leaves the lines about other spells a gap.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

fn translated(text: &str) -> (String, Vec<String>) {
    let json = serde_json::to_value(abilities(text)).unwrap();
    (serde_json::to_string(&json).unwrap(), gaps(text))
}

fn sorcery(statics: &str, rest: &str) -> String {
    format!("Name:Subject\nManaCost:2 R\nTypes:Sorcery\nS:{statics}\n{rest}")
}

const OWN: &str = "Mode$ OptionalCost | EffectZone$ All | ValidCard$ Card.Self | ValidSA$ Spell";

#[rstest]
#[case::blight("Cost$ Blight<1>", r#"{"AdditionalCost":{"cost":[{"Blight":1}]}}"#)]
#[case::mana_alone("Cost$ 2 R", r#"{"AdditionalCost":{"cost":"{2}{R}"}}"#)]
#[case::evidence(
    "Cost$ CollectEvidence<6>",
    r#"{"AdditionalCost":{"cost":[{"CollectEvidence":6}]}}"#
)]
#[case::behold(
    "Cost$ Behold<1/Creature>",
    r#"{"AdditionalCost":{"cost":[{"Behold":{"filter":[{"Type":"Creature"}]}}]}}"#
)]
fn the_spells_own_optional_cost_is_an_additional_cost_it_may_skip(
    #[case] cost: &str,
    #[case] expected: &str,
) {
    let text = sorcery(&format!("{OWN} | {cost}"), "A:SP$ Draw | NumCards$ 1\n");
    let (json, found) = translated(&text);
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[rstest]
#[case::other_spells(
    "Mode$ OptionalCost | ValidCard$ Permanent.White | ValidSA$ Spell | Activator$ You | Cost$ PayLife<2> | ReduceColor$ W",
    "key:Static.OptionalCost.Activator"
)]
#[case::abilities(
    "Mode$ OptionalCost | EffectZone$ All | ValidCard$ Card.Self | ValidSA$ Activated | Cost$ PayLife<2>",
    "static:OptionalCost-of-other-spells"
)]
#[case::from_the_battlefield(
    "Mode$ OptionalCost | EffectZone$ Battlefield | ValidCard$ Card.Self | ValidSA$ Spell | Cost$ PayLife<2>",
    "static:OptionalCost-of-other-spells"
)]
fn an_optional_cost_of_other_spells_or_abilities_stays_a_gap(
    #[case] line: &str,
    #[case] gap: &str,
) {
    let text = sorcery(line, "A:SP$ Draw | NumCards$ 1\n");
    let (_, found) = translated(&text);
    assert!(found.iter().any(|found| found == gap), "{found:?}");
}

#[rstest]
#[case::paid("ConditionOptionalPaid$ True", r#""condition":"AdditionalCostPaid""#)]
#[case::not_paid(
    "ConditionOptionalPaid$ False",
    r#""condition":{"Not":"AdditionalCostPaid"}"#
)]
fn a_step_on_the_paid_cost_reads_the_additional_cost(#[case] paid: &str, #[case] expected: &str) {
    let rest = format!(
        "A:SP$ Draw | NumCards$ 1 | SubAbility$ DBGain\nSVar:DBGain:DB$ GainLife | LifeAmount$ 2 | Condition$ OptionalCost | {paid}\n"
    );
    let (json, found) = translated(&sorcery(&format!("{OWN} | Cost$ Blight<1>"), &rest));
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[test]
fn a_condition_on_the_paid_cost_without_its_answer_stays_a_gap() {
    let rest = "A:SP$ Draw | NumCards$ 1 | SubAbility$ DBGain\nSVar:DBGain:DB$ GainLife | LifeAmount$ 2 | Condition$ OptionalCost\n";
    let (_, found) = translated(&sorcery(&format!("{OWN} | Cost$ Blight<1>"), rest));
    assert!(
        found.iter().any(|found| found == "condition:OptionalCost"),
        "{found:?}"
    );
}

#[test]
fn the_amount_of_a_paid_optional_cost_picks_a_branch() {
    let rest = "A:SP$ Dig | Defined$ You | DigNum$ X | ChangeNum$ All | DestinationZone$ Exile\nSVar:X:Count$OptionalGenericCostPaid.3.2\n";
    let (json, found) = translated(&sorcery(&format!("{OWN} | Cost$ Blight<1>"), rest));
    assert!(found.is_empty(), "{found:?}");
    assert!(
        json.contains(r#"{"When":{"condition":"AdditionalCostPaid","otherwise":2,"then":3}}"#),
        "{json}"
    );
}

#[rstest]
#[case::collected(
    "ConditionDefined$ Collected | ConditionPresent$ Card",
    r#""condition":"AdditionalCostPaid""#
)]
#[case::not_collected(
    "ConditionDefined$ Collected | ConditionPresent$ Card | ConditionCompare$ EQ0",
    r#""condition":{"Not":"AdditionalCostPaid"}"#
)]
fn evidence_collected_is_the_paid_additional_cost(#[case] keys: &str, #[case] expected: &str) {
    let rest = format!(
        "A:SP$ Draw | NumCards$ 1 | SubAbility$ DBGain\nSVar:DBGain:DB$ GainLife | LifeAmount$ 2 | {keys}\nOracle:As an additional cost to cast this spell, you may collect evidence 6.\\nDraw a card. If evidence was collected, you gain 2 life.\n"
    );
    let (json, found) = translated(&sorcery(
        &format!("{OWN} | Cost$ CollectEvidence<6>"),
        &rest,
    ));
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[test]
fn collected_cards_without_the_evidence_wording_stay_a_gap() {
    let rest = "A:SP$ Draw | NumCards$ 1 | SubAbility$ DBGain\nSVar:DBGain:DB$ GainLife | LifeAmount$ 2 | ConditionDefined$ Collected | ConditionPresent$ Card\nOracle:Draw a card. You gain 2 life for each card exiled this way.\n";
    let (_, found) = translated(&sorcery(&format!("{OWN} | Cost$ CollectEvidence<6>"), rest));
    assert!(!found.is_empty());
}

fn god(keys: &str) -> String {
    format!(
        "Name:Subject\nManaCost:3 W\nTypes:Legendary Enchantment Creature God\nPT:5/6\nS:Mode$ Continuous | AffectedDefined$ Self | {keys}\nSVar:X:Count$Devotion.White\n"
    )
}

#[rstest]
#[case::devotion(
    "RemoveType$ Creature | CheckSVar$ X | SVarCompare$ LT5",
    r#"{"Static":{"affected":"This","condition":{"Compare":{"left":{"Devotion":["White"]},"op":"Lt","right":5}},"mods":[{"RemoveType":"Creature"}]}}"#
)]
#[case::always("RemoveType$ Creature", r#""mods":[{"RemoveType":"Creature"}]"#)]
fn a_card_type_taken_away_is_a_modification(#[case] keys: &str, #[case] expected: &str) {
    let (json, found) = translated(&god(keys));
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[rstest]
#[case::supertype("RemoveType$ Legendary")]
#[case::subtype("RemoveType$ God")]
fn a_supertype_or_subtype_taken_away_stays_a_gap(#[case] keys: &str) {
    let (_, found) = translated(&god(keys));
    assert!(
        found
            .iter()
            .any(|found| found == "key:Static.Continuous.RemoveType"),
        "{found:?}"
    );
}

#[rstest]
#[case::behold_and_exile(
    "BeholdExile<1/Creature>",
    r#"{"AdditionalCost":{"cost":[{"Behold":{"exile":true,"filter":[{"Type":"Creature"}]}}],"required":true}}"#
)]
#[case::behold_only(
    "Behold<1/Creature>",
    r#"{"AdditionalCost":{"cost":[{"Behold":{"filter":[{"Type":"Creature"}]}}],"required":true}}"#
)]
fn a_required_behold_of_the_spell_itself_is_its_additional_cost(
    #[case] cost: &str,
    #[case] expected: &str,
) {
    let text = format!(
        "Name:Subject\nManaCost:3 R\nTypes:Creature Elemental\nPT:7/3\nS:Mode$ RaiseCost | ValidCard$ Card.Self | Activator$ You | Type$ Spell | Cost$ {cost} | EffectZone$ All\n"
    );
    let (json, found) = translated(&text);
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[test]
fn a_waterbend_of_a_spell_stays_a_gap() {
    let text = "Name:Subject\nManaCost:U U\nTypes:Sorcery\nS:Mode$ RaiseCost | ValidCard$ Card.Self | Activator$ You | Type$ Spell | Cost$ Waterbend<5> | EffectZone$ All\nA:SP$ Draw | NumCards$ 2\n";
    let (_, found) = translated(text);
    assert!(
        found
            .iter()
            .any(|found| found == "cost:Waterbend-outside-an-ability"),
        "{found:?}"
    );
}
