//! Goal: "Whenever you activate an ability of <permanent>" reads into `Did { ActivatedAny }`, a mana ability included, and writes back as the card's text for you and an opponent; the mana ability left out, a card elsewhere and the card itself stay elsewhere.
use super::*;
use crate::did::Action;
use crate::trigger::TriggerPattern;

#[rstest]
#[case::elrond(
    "Whenever you activate an ability of a creature, draw a card.",
    r#"{"Triggered":{"when":{"Did":{"action":"ActivatedAny","who":"You","what":[{"Type":"Creature"},{"In":"Battlefield"}]}},"effects":[{"Draw":{}}]}}"#
)]
#[case::searblades(
    "Whenever you activate an ability of an Elemental, ~ gets +1/+0 until end of turn.",
    r#"{"Triggered":{"when":{"Did":{"action":"ActivatedAny","who":"You","what":[{"Subtype":"Elemental"},{"In":"Battlefield"}]}},"effects":[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":1,"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::opponent_artifact(
    "Whenever an opponent activates an ability of an artifact, you gain 1 life.",
    r#"{"Triggered":{"when":{"Did":{"action":"ActivatedAny","who":"Opponents","what":[{"Type":"Artifact"},{"In":"Battlefield"}]}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
fn l294_activations_read_and_write_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::not_mana(
    "Whenever you activate an ability of a creature that isn't a mana ability, draw a card."
)]
#[case::graveyard("Whenever you activate an ability of a card in your graveyard, draw a card.")]
#[case::itself("Whenever you activate an ability of ~, draw a card.")]
#[case::loyalty("Whenever you activate a loyalty ability of a Chandra planeswalker, draw a card.")]
fn l294_activations_near_misses_count_no_mana_ability(#[case] text: &str) {
    let read = clause(text, ClauseKind::Triggered, false);
    assert!(
        !matches!(
            read,
            Some(Ability::Triggered {
                when: TriggerPattern::Did {
                    action: Action::ActivatedAny,
                    ..
                },
                ..
            })
        ),
        "{text}: {read:?}"
    );
}

#[rstest]
#[case::anywhere(r#"{"Triggered":{"when":{"Did":{"action":"ActivatedAny","who":"You","what":[{"Type":"Creature"}]}},"effects":[{"Draw":{}}]}}"#)]
#[case::nothing(r#"{"Triggered":{"when":{"Did":{"action":"ActivatedAny","who":"You","what":[{"In":"Battlefield"}]}},"effects":[{"Draw":{}}]}}"#)]
fn l294_activations_off_the_battlefield_write_nothing(#[case] json: &str) {
    assert!(render_variants(&ability(json)).is_empty(), "{json}");
}

#[test]
fn l294_activations_write_a_planeswalker_subtype() {
    let json = r#"{"Triggered":{"when":{"Did":{"action":"ActivatedAny","who":"You","what":[{"Type":"Planeswalker"},{"Subtype":"Sarkhan"},{"In":"Battlefield"}]}},"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}}]}}"#;
    assert!(
        render_variants(&ability(json)).contains(
            &"Whenever you activate an ability of a Sarkhan planeswalker, ~ deals 1 damage to any target."
                .to_owned()
        )
    );
}

#[rstest]
#[case::scry(
    r#"{"Triggered":{"when":{"Did":{"action":"Enlisted","what":["This"]}},"effects":[{"Scry":{"count":2}}]}}"#,
    "Whenever ~ enlists a creature, scry 2."
)]
fn l294_enlisting_writes_back(#[case] json: &str, #[case] text: &str) {
    let written = render_variants(&ability(json));
    assert!(written.iter().any(|w| w == text), "{written:?}");
}

#[rstest]
#[case::another_creature(r#"{"Triggered":{"when":{"Did":{"action":"Enlisted","what":[{"Type":"Creature"}],"who":"You"}},"effects":[{"Scry":{"count":2}}]}}"#)]
fn l294_enlisting_of_others_writes_nothing(#[case] json: &str) {
    let written = render_variants(&ability(json));
    assert!(written.iter().all(|w| !w.contains("enlist")), "{written:?}");
}
