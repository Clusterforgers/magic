//! Goal: the writers of the triggers that untap creatures and add a combat phase give the oracle wording of a payment
//! that adds the phase ("untap all attacking creatures and after this phase, there is an additional combat phase"),
//! energy got and then paid ("..., then you may pay eight {E}. If you pay, ..."), the source and a group untapped
//! together ("untap it and all Samurai you control") and an attack at the player with the most life; a changed amount,
//! group, comparison or number of phases never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const CHARGER: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Pay":{"cost":"{5}{R}{R}","paid":[{"Untap":{"what":{"Each":[{"Type":"Creature"},"Attacking"]}}},{"AddPhase":{"phases":["Combat"]}}]}}]}}"#;
const RUNNER: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"PutCounters":{"on":"You","counter":{"Other":"energy"},"count":2}},{"Pay":{"cost":[{"PayEnergy":8}],"paid":[{"Untap":{"what":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}},{"AddPhase":{"phases":["Combat"]}}]}}]}}"#;
const GODO: &str = r#"{"Triggered":{"when":{"Nth":{"event":{"Attacks":{"what":["This"]}},"nth":1}},"effects":[{"Untap":{"what":"This"}},{"Untap":{"what":{"Each":[{"Subtype":"Samurai"},{"ControlledBy":"You"}]}}},{"AddPhase":{"phases":["Combat"]}}]}}"#;
const SCOURGE: &str = r#"{"Triggered":{"when":{"Nth":{"event":{"Attacks":{"what":["This"]}},"nth":1}},"if":{"Compare":{"left":{"Life":"Defending"},"op":"Ge","right":{"HighestLife":"EachPlayer"}}},"effects":[{"Untap":{"what":{"Each":[{"Type":"Creature"},"Attacking"]}}},{"AddPhase":{"phases":["Combat"]}}]}}"#;

#[rstest]
#[case::paid_untap_and_phase(
    CHARGER,
    "Whenever ~ attacks, you may pay {5}{R}{R}. If you do, untap all attacking creatures and after this phase, there is an additional combat phase."
)]
#[case::energy_then_payment(
    RUNNER,
    "Whenever ~ attacks, you get {E}{E}, then you may pay eight {E}. If you pay, untap all creatures you control, and after this phase, there is an additional combat phase."
)]
#[case::the_source_and_its_group(
    GODO,
    "Whenever ~ attacks for the first time each turn, untap it and all Samurai you control. After this phase, there is an additional combat phase."
)]
#[case::the_player_with_the_most_life(
    SCOURGE,
    "Whenever ~ attacks for the first time each turn, if it's attacking the player with the most life or tied for most life, untap all attacking creatures. After this phase, there is an additional combat phase."
)]
fn a_trigger_that_untaps_and_adds_a_combat_phase_is_written_as_the_card_says(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::two_phases(
    CHARGER.replace(r#"["Combat"]"#, r#"["Combat","Combat"]"#),
    "Whenever ~ attacks, you may pay {5}{R}{R}. If you do, untap all attacking creatures and after this phase, there is an additional combat phase."
)]
#[case::another_cost(
    CHARGER.replace("{5}{R}{R}", "{5}{R}"),
    "Whenever ~ attacks, you may pay {5}{R}{R}. If you do, untap all attacking creatures and after this phase, there is an additional combat phase."
)]
#[case::another_amount_of_energy(
    RUNNER.replace(r#""PayEnergy":8"#, r#""PayEnergy":7"#),
    "Whenever ~ attacks, you get {E}{E}, then you may pay eight {E}. If you pay, untap all creatures you control, and after this phase, there is an additional combat phase."
)]
#[case::another_group(
    GODO.replace("Samurai", "Elf"),
    "Whenever ~ attacks for the first time each turn, untap it and all Samurai you control. After this phase, there is an additional combat phase."
)]
#[case::the_second_time(
    GODO.replace(r#""nth":1"#, r#""nth":2"#),
    "Whenever ~ attacks for the first time each turn, untap it and all Samurai you control. After this phase, there is an additional combat phase."
)]
#[case::the_least_life(
    SCOURGE.replace("HighestLife", "LowestLife"),
    "Whenever ~ attacks for the first time each turn, if it's attacking the player with the most life or tied for most life, untap all attacking creatures. After this phase, there is an additional combat phase."
)]
#[case::strictly_the_most_life(
    SCOURGE.replace(r#""op":"Ge""#, r#""op":"Gt""#),
    "Whenever ~ attacks for the first time each turn, if it's attacking the player with the most life or tied for most life, untap all attacking creatures. After this phase, there is an additional combat phase."
)]
fn a_changed_trigger_does_not_read_back_as_the_card(#[case] json: String, #[case] clause: &str) {
    assert!(!written(&json, clause), "{clause}");
}
