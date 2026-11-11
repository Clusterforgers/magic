//! Goal: the writers of ep33 write the card's words and nothing near them. "Whenever you collect evidence" is the action of the card's controller, and a card put onto the battlefield face down is said with its controller and, from the graveyard, beside a face-up return; another player, action, effect, controller or way of entering does not read as it.
use super::*;

const EXAMINER: &str = "Whenever you collect evidence, investigate.";

fn collected(who: &str, action: &str, effect: &str) -> Ability {
    ability(&format!(
        r#"{{"Triggered":{{"when":{{"Did":{{"action":"{action}","who":"{who}"}}}},"effects":[{effect}]}}}}"#
    ))
}

const CLUE: &str = r#"{"CreateToken":{"token":{"Named":"clue"}}}"#;

#[rstest]
#[case::examiner(collected("You", "CollectedEvidence", CLUE), EXAMINER, true)]
#[case::an_opponent(collected("Opponents", "CollectedEvidence", CLUE), EXAMINER, false)]
#[case::any_player(collected("EachPlayer", "CollectedEvidence", CLUE), EXAMINER, false)]
#[case::another_action(collected("You", "Surveilled", CLUE), EXAMINER, false)]
#[case::another_effect(
    collected("You", "CollectedEvidence", r#"{"Draw":{}}"#),
    EXAMINER,
    false
)]
#[case::opponent_words(
    collected("Opponents", "CollectedEvidence", CLUE),
    "Whenever an opponent collects evidence, investigate.",
    true
)]
fn ep33_collected_evidence_near_misses_differ(
    #[case] ability: Ability,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability);
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

const PHOENIX: &str = "When ~ dies, return it to the battlefield face down under your control.";

const RAPTOR: &str = "Whenever a permanent you control is turned face up, you may return ~ from your graveyard to the battlefield face up or face down.";

fn dies(to: &str) -> Ability {
    ability(&format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"from":"Battlefield","to":"Graveyard"}}}},"effects":[{{"Move":{{"what":"This","to":{to}}}}}]}}}}"#
    ))
}

fn raptor(up: &str, down: &str, may: bool) -> Ability {
    let either = format!(
        r#"{{"Either":{{"first":[{{"Move":{{"what":"This","to":{up}}}}}],"second":[{{"Move":{{"what":"This","to":{down}}}}}]}}}}"#
    );
    let effect = if may {
        format!(r#"{{"May":{{"effects":[{either}]}}}}"#)
    } else {
        either
    };
    ability(&format!(
        r#"{{"Triggered":{{"when":{{"TurnedFaceUp":{{"what":[{{"ControlledBy":"You"}}]}}}},"from":"Graveyard","effects":[{effect}]}}}}"#
    ))
}

const UP: &str = r#"{"Battlefield":{}}"#;
const DOWN: &str = r#"{"BattlefieldFaceDown":{}}"#;

#[rstest]
#[case::phoenix(dies(r#"{"BattlefieldFaceDown":{"under":"You"}}"#), PHOENIX, true)]
#[case::owners_control(
    dies(DOWN),
    "When ~ dies, return it to the battlefield face down under its owner's control.",
    true
)]
#[case::under_the_owner(dies(DOWN), PHOENIX, false)]
#[case::face_up(dies(r#"{"Battlefield":{"under":"You"}}"#), PHOENIX, false)]
#[case::under_an_opponent(
    dies(r#"{"BattlefieldFaceDown":{"under":"Opponents"}}"#),
    PHOENIX,
    false
)]
#[case::raptor(raptor(UP, DOWN, true), RAPTOR, true)]
#[case::raptor_must(raptor(UP, DOWN, false), RAPTOR, false)]
#[case::raptor_both_down(raptor(DOWN, DOWN, true), RAPTOR, false)]
#[case::raptor_tapped(
    raptor(r#"{"Battlefield":{"tapped":true}}"#, DOWN, true),
    RAPTOR,
    false
)]
#[case::raptor_under_you(
    raptor(UP, r#"{"BattlefieldFaceDown":{"under":"You"}}"#, true),
    RAPTOR,
    false
)]
fn ep33_face_down_near_misses_differ(
    #[case] ability: Ability,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability);
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}
