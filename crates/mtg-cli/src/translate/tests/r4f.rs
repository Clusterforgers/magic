//! Goal: each Forge parameter form of the r4f pack reads to the effect it spells, probed by a card per form; forms the
//! engine cannot state stay named gaps.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn first_effects(script: &str) -> Value {
    let ability = abilities(script).into_iter().next().unwrap();
    let (_, body) = ability.as_object().unwrap().iter().next().unwrap();
    body["effects"].clone()
}

#[rstest]
#[case::different_names(
    "Name:T\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Library | Destination$ Hand | ChangeType$ Aura | ChangeNum$ 3 | DifferentNames$ True\n",
    true
)]
#[case::any_names(
    "Name:T\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Library | Destination$ Hand | ChangeType$ Aura | ChangeNum$ 3\n",
    false
)]
fn a_search_with_different_names_finds_one_card_per_name(
    #[case] script: &str,
    #[case] distinct: bool,
) {
    let effects = first_effects(script);
    let want = json!(true);
    assert_eq!(
        effects[0]["Search"].get("distinct"),
        distinct.then_some(&want)
    );
    assert!(gaps(script).is_empty(), "{:?}", gaps(script));
}

#[rstest]
#[case::no_shuffle_to_hand(
    "Name:T\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Library | Destination$ Hand | ChangeType$ Instant | ChangeNum$ 1 | NoShuffle$ True\n",
    false
)]
#[case::shuffle(
    "Name:T\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Library | Destination$ Hand | ChangeType$ Instant | ChangeNum$ 1\n",
    true
)]
fn no_shuffle_leaves_the_library_as_the_search_left_it(
    #[case] script: &str,
    #[case] shuffled: bool,
) {
    let effects = first_effects(script);
    assert_eq!(
        effects
            .as_array()
            .unwrap()
            .contains(&json!({"Shuffle": {}})),
        shuffled
    );
}

#[test]
fn no_shuffle_into_the_library_stays_a_gap() {
    let script = "Name:T\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Library | Destination$ Library | ChangeType$ Instant | ChangeNum$ 1 | NoShuffle$ True\n";
    assert!(
        gaps(script)
            .iter()
            .any(|gap| gap == "key:ChangeZone.Search.NoShuffle")
    );
}

#[test]
fn a_commander_comes_from_the_command_zone_by_ownership() {
    let script = "Name:T\nTypes:Artifact\nA:AB$ ChangeZone | Cost$ T | ChangeType$ Card.IsCommander+YouOwn | ChangeNum$ 1 | Mandatory$ True | Origin$ Command | Destination$ Hand | Hidden$ True\n";
    let effects = first_effects(script);
    assert_eq!(
        effects[0]["Choose"]["from"],
        json!({"Each": ["Commander", {"OwnedBy": "You"}, {"In": "Command"}]})
    );
    assert_eq!(
        effects[1],
        json!({"Move": {"to": "Hand", "what": {"Bound": "picked"}}})
    );
}

#[test]
fn the_card_that_just_died_goes_back_without_an_origin_word() {
    let script = "Name:T\nTypes:Creature God\nT:Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Card.Self | Execute$ Back | TriggerDescription$ x\nSVar:Back:DB$ ChangeZone | Defined$ TriggeredNewCardLKICopy | Destination$ Library | LibraryPosition$ 2\n";
    assert!(gaps(script).is_empty(), "{:?}", gaps(script));
    let effects = first_effects(script);
    assert_eq!(effects[0]["Move"]["what"], json!("This"));
}

#[test]
fn a_spell_or_a_permanent_is_one_target_in_two_zones() {
    let script = "Name:T\nTypes:Instant\nA:SP$ ChangeZone | ValidTgts$ Permanent.nonLand,Card.inZoneStack | TgtZone$ Stack,Battlefield | Origin$ Battlefield,Stack | Destination$ Hand\n";
    assert!(gaps(script).is_empty(), "{:?}", gaps(script));
    let ability = abilities(script).remove(0);
    let what = &ability["Spell"]["targets"][0]["what"]["Object"][0]["AnyOf"];
    assert_eq!(what[1], json!({"In": "Stack"}));
    assert!(what[0].to_string().contains("Battlefield"));
}

#[test]
fn a_revealed_number_of_cards_is_picked_shown_and_then_discarded_from() {
    let script = "Name:T\nTypes:Sorcery\nA:SP$ Discard | ValidTgts$ Player | Mode$ RevealYouChoose | RevealNumber$ 3 | NumCards$ 1\n";
    let effects = first_effects(script);
    assert_eq!(effects[0]["Choose"]["count"], json!({"min": 3, "max": 3}));
    assert_eq!(effects[1], json!({"Reveal": {"what": {"Bound": "shown"}}}));
    assert_eq!(
        effects[2]["Discard"]["filter"],
        json!([{"Is": {"Bound": "shown"}}])
    );
    let looking = script.replace("RevealYouChoose", "LookYouChoose");
    assert!(
        gaps(&looking)
            .iter()
            .any(|gap| gap == "key:Discard.RevealNumber")
    );
}
