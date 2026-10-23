//! Goal: manifest and cloak read from the top of a library in the shapes of the cards (an imperative, a number of cards, a third person for the controller of an exiled permanent, the library of the player an event is about) in every kind of ability, write back as the card's own text, and the near-misses stay with the table.
use super::*;
use crate::effect::Effect;
use crate::refs::Ref;

#[rstest]
#[case::manifest("Manifest the top card of your library.", r#"{"Manifest":{}}"#)]
#[case::two_cards(
    "Manifest the top two cards of your library.",
    r#"{"Manifest":{"count":2}}"#
)]
#[case::cloak("Cloak the top card of your library.", r#"{"Cloak":{}}"#)]
#[case::cloak_three(
    "Cloak the top three cards of your library.",
    r#"{"Cloak":{"count":3}}"#
)]
fn the_top_cards_of_your_library_are_put_face_down(#[case] text: &str, #[case] effect: &str) {
    let json = format!(r#"{{"Spell":{{"effects":[{effect}]}}}}"#);
    reads_as(text, ClauseKind::Spell, false, &json);
}

#[rstest]
#[case::dies(
    "When this creature dies, manifest the top card of your library.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Manifest":{}}]}}"#
)]
#[case::each_opponents_library(
    "At the beginning of each opponent's upkeep, you manifest the top card of that player's library.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"Opponents"}},"effects":[{"Manifest":{"library":"Triggering"}}]}}"#
)]
fn a_trigger_manifests(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::manifest(
    "{3}{W}: Manifest the top card of your library.",
    r#"{"Activated":{"cost":"{3}{W}","effects":[{"Manifest":{}}]}}"#
)]
fn an_activation_manifests(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Activated, false, json);
}

#[test]
fn the_controller_of_an_exiled_creature_manifests_from_their_own_library() {
    let read = golden(
        "Exile target creature. Its controller manifests the top card of their library.",
        ClauseKind::Spell,
        false,
    );
    let Ability::Spell { effects, .. } = read else {
        panic!("a spell");
    };
    assert!(matches!(
        effects.as_slice(),
        [
            Effect::Exile { .. },
            Effect::Manifest {
                who: Ref::ControllerOf(_),
                from: None,
                bind: None,
                ..
            }
        ]
    ));
}

#[rstest]
#[case::a_chosen_card("Manifest a card from your hand.")]
#[case::the_wrong_person("Manifests the top card of your library.")]
#[case::your_own_verb("Its controller manifest the top card of their library.")]
#[case::zero_cards("Manifest the top zero cards of your library.")]
#[case::dread("Manifest dread.")]
#[case::a_follow_up("Manifest the top card of your library, then put a +1/+1 counter on it.")]
#[case::another_library("Manifest the top card of target player's library.")]
fn a_wording_the_template_does_not_model_is_left_to_the_table(#[case] text: &str) {
    assert!(clause(text, ClauseKind::Spell, false).is_none(), "{text}");
}
