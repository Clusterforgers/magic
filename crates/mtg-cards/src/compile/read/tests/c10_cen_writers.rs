//! Goal: the c10 census writers give the oracle wording of a look at the top card that puts a land onto the battlefield tapped and of damage equal to the power of the ability's own damage source; another binding, card type, untapped move, extra step or a power of another object never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn c10_cen_written(json: &str, clause: &str) -> bool {
    let variants = written_variants(&ability(json));
    variants.iter().any(|wording| same_text(wording, clause))
}

const C10_CEN_RAIDERS_KARVE: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"LookAtTop":{"as":"top"}},{"If":{"condition":{"Matches":{"subject":{"Bound":"top"},"selector":[{"Type":"Land"}]}},"then":[{"May":{"effects":[{"Move":{"what":{"Bound":"top"},"to":{"Battlefield":{"tapped":true}}}}]}}]}}]}}"#;
const C10_CEN_GLEEFUL_ARSONIST: &str = r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Not":{"Type":"Creature"}}],"by":"Opponents"}},"effects":[{"DealDamage":{"source":"This","to":{"ControllerOf":"Triggering"},"amount":{"Stat":{"of":"This","stat":"Power"}}}}]}}"#;
const C10_CEN_LOTHLORIEN_BLADE: &str = r#"{"Triggered":{"when":{"Attacks":{"what":[{"Is":"Attached"}]}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Defending"}]}}],"effects":[{"DealDamage":{"source":"Attached","to":{"Target":0},"amount":{"Stat":{"of":"Attached","stat":"Power"}}}}]}}"#;
const C10_CEN_FIENDLASH: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":[],"to":"Attached"}},"targets":[{"what":{"Either":["Player",{"Object":[{"Type":"Planeswalker"}]}]}}],"effects":[{"DealDamage":{"source":"Attached","to":{"Target":0},"amount":{"Stat":{"of":"Attached","stat":"Power"}}}}]}}"#;

const C10_CEN_KARVE_TEXT: &str = "Whenever ~ attacks, look at the top card of your library. If it's a land card, you may put it onto the battlefield tapped.";
const C10_CEN_ARSONIST_TEXT: &str = "Whenever an opponent casts a noncreature spell, ~ deals damage equal to its power to that player.";
const C10_CEN_BLADE_TEXT: &str = "Whenever equipped creature attacks, it deals damage equal to its power to target creature defending player controls.";
const C10_CEN_FIENDLASH_TEXT: &str = "Whenever equipped creature is dealt damage, it deals damage equal to its power to target player or planeswalker.";

#[rstest]
#[case::karve(C10_CEN_RAIDERS_KARVE, C10_CEN_KARVE_TEXT)]
#[case::karve_by_another_name(&C10_CEN_RAIDERS_KARVE.replace("top", "card"), C10_CEN_KARVE_TEXT)]
#[case::arsonist(C10_CEN_GLEEFUL_ARSONIST, C10_CEN_ARSONIST_TEXT)]
#[case::blade(C10_CEN_LOTHLORIEN_BLADE, C10_CEN_BLADE_TEXT)]
#[case::fiendlash(C10_CEN_FIENDLASH, C10_CEN_FIENDLASH_TEXT)]
fn c10_cen_the_oracle_wording_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(
        c10_cen_written(json, clause),
        "{clause}\n{:#?}",
        written_variants(&ability(json))
    );
}

#[rstest]
#[case::karve_puts_it_untapped(
    &C10_CEN_RAIDERS_KARVE.replace(r#"{"tapped":true}"#, "{}"),
    C10_CEN_KARVE_TEXT
)]
#[case::karve_for_a_creature(
    &C10_CEN_RAIDERS_KARVE.replace(r#""Land""#, r#""Creature""#),
    C10_CEN_KARVE_TEXT
)]
#[case::karve_moves_another_card(
    &C10_CEN_RAIDERS_KARVE.replace(r#""what":{"Bound":"top"}"#, r#""what":{"Bound":"other"}"#),
    C10_CEN_KARVE_TEXT
)]
#[case::karve_looks_at_two(
    &C10_CEN_RAIDERS_KARVE.replace(r#"{"as":"top"}"#, r#"{"count":2,"as":"top"}"#),
    C10_CEN_KARVE_TEXT
)]
#[case::arsonist_by_another_power(
    &C10_CEN_GLEEFUL_ARSONIST.replace(r#""of":"This""#, r#""of":"Triggering""#),
    C10_CEN_ARSONIST_TEXT
)]
#[case::arsonist_by_toughness(
    &C10_CEN_GLEEFUL_ARSONIST.replace("Power", "Toughness"),
    C10_CEN_ARSONIST_TEXT
)]
#[case::blade_by_its_holder(
    &C10_CEN_LOTHLORIEN_BLADE.replace(r#""of":"Attached""#, r#""of":"This""#),
    C10_CEN_BLADE_TEXT
)]
fn c10_cen_a_near_miss_never_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(!c10_cen_written(json, clause), "{clause}");
}
