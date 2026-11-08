//! Goal: round 13 pack g13g reads an additional cost of one to many options, an untap ban that holds while a permanent is
//! present, and the card a mill put into a graveyard as one a permission casts from there; each near-miss keeps its gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn card(types: &str, lines: &str) -> String {
    format!("Name:T\nManaCost:R\nTypes:{types}\n{lines}\nOracle:x\n")
}

#[rstest]
#[case::sacrifice_a_creature_discard_or_pay_life(
    "K:AlternateAdditionalCost:Sac<1/Creature>:Discard<1/Card>:PayLife<4>",
    Some(3)
)]
#[case::two_options("K:AlternateAdditionalCost:Sac<1/Creature>:Discard<1/Card>", Some(2))]
#[case::one_option("K:AlternateAdditionalCost:Sac<1/Creature>", None)]
fn g13g_alternative_additional_costs(#[case] line: &str, #[case] options: Option<usize>) {
    let text = card("Creature", line);
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
    let found = abilities(&text);
    let cost = &found[0]["AdditionalCost"]["cost"];
    match options {
        Some(count) => assert_eq!(cost[0]["OneOf"].as_array().map(Vec::len), Some(count)),
        None => assert_eq!(
            cost,
            &json!([{"Sacrifice": {"filter": [{"Type": "Creature"}]}}])
        ),
    }
    assert_eq!(found[0]["AdditionalCost"]["required"], json!(true));
}

#[rstest]
#[case::while_you_control_an_elf(
    "R:Event$ Untap | ActiveZones$ Battlefield | ValidCard$ Card.Self | ValidStepTurnToController$ You | Layer$ CantHappen | IsPresent$ Elf.YouCtrl | Description$ x",
    true
)]
#[case::without_a_condition(
    "R:Event$ Untap | ActiveZones$ Battlefield | ValidCard$ Card.Self | ValidStepTurnToController$ You | Layer$ CantHappen | Description$ x",
    false
)]
fn g13g_a_conditional_untap_ban(#[case] line: &str, #[case] conditional: bool) {
    let text = card("Creature", line);
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
    let found = abilities(&text);
    assert_eq!(found[0]["Static"].get("condition").is_some(), conditional);
}

#[rstest]
#[case::the_milled_card_is_played(
    "SVar:Play:Mode$ Continuous | MayPlay$ True | EffectZone$ Command | Affected$ Card.IsRemembered | AffectedZone$ Graveyard,Exile | Description$ You may play that card this turn.",
    true
)]
#[case::another_card_filter_stays_open(
    "SVar:Play:Mode$ Continuous | MayPlay$ True | EffectZone$ Command | Affected$ Card.IsRemembered+nonLand | AffectedZone$ Graveyard,Exile | Description$ You may play that card this turn.",
    false
)]
fn g13g_a_remembered_card_in_two_zones(#[case] svar: &str, #[case] reads: bool) {
    let text = card(
        "Artifact",
        &format!(
            "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigMill | TriggerDescription$ x\nSVar:TrigMill:DB$ Mill | Defined$ You | NumCards$ 1 | RememberMilled$ True | SubAbility$ DBEffect\nSVar:DBEffect:DB$ Effect | RememberObjects$ Remembered | StaticAbilities$ Play | Duration$ EndOfTurn | ForgetOnMoved$ Graveyard | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\n{svar}"
        ),
    );
    assert_eq!(gaps(&text).is_empty(), reads, "{:?}", gaps(&text));
    if reads {
        assert!(
            Value::Array(abilities(&text))
                .to_string()
                .contains(r#""from":"Graveyard""#)
        );
    }
}
