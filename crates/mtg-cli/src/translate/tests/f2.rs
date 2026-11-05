//! Goal: a delayed trigger that remembers the card an object became (`TriggeredNewCardLKICopy`) names the object one move on in the zone the pattern names; a pattern that names no zone, or a word for the old card, stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn creature(body: &str) -> String {
    format!("Name:T\nTypes:Creature Elf\nPT:1/1\n{body}\n")
}

fn delayed(trigger: &str, word: &str) -> String {
    creature(&format!(
        "T:{trigger} | Execute$ Delay\nSVar:Delay:DB$ DelayedTrigger | Mode$ Phase | Phase$ End of Turn | RememberObjects$ {word} | Execute$ Back\nSVar:Back:DB$ ChangeZone | Origin$ Graveyard | Destination$ Hand | Defined$ DelayTriggerRememberedLKI"
    ))
}

#[rstest]
#[case::this_dies(
    "Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Card.Self",
    json!({"Arrived": {"of": "This", "zone": "Graveyard"}})
)]
#[case::another_dies(
    "Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Creature.YouCtrl",
    json!({"Arrived": {"of": "Triggering", "zone": "Graveyard"}})
)]
#[case::exiled(
    "Mode$ ChangesZone | Origin$ Battlefield | Destination$ Exile | ValidCard$ Creature.YouCtrl",
    json!({"Arrived": {"of": "Triggering", "zone": "Exile"}})
)]
fn the_card_an_object_became_is_the_object_one_move_on(
    #[case] trigger: &str,
    #[case] named: Value,
) {
    let script = delayed(trigger, "TriggeredNewCardLKICopy");
    assert_eq!(gaps(&script), Vec::<String>::new());
    let effects = &abilities(&script)[0]["Triggered"]["effects"];
    assert_eq!(effects[0], json!({"Bind": {"as": "event", "what": named}}));
}

#[rstest]
#[case::no_zone_named(
    "Mode$ ChangesZone | Origin$ Battlefield | ValidCard$ Card.Self",
    "TriggeredNewCardLKICopy"
)]
#[case::from_anywhere(
    "Mode$ ChangesZone | Destination$ Graveyard | ValidCard$ Card.Self",
    "TriggeredNewCardLKICopy"
)]
#[case::the_old_card(
    "Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Card.Self",
    "TriggeredCard"
)]
fn a_word_the_pattern_cannot_place_stays_a_gap(#[case] trigger: &str, #[case] word: &str) {
    let script = delayed(trigger, word);
    assert_eq!(
        gaps(&script),
        vec![format!("key:RememberObjects={word}-after-a-move")]
    );
}
