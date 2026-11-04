//! Goal: round 3 piece P3 (zones): a move from the stack names the spell itself or the spell a cast trigger is about.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

/// The effects of the first ability of the card, or of its first trigger.
fn effects(text: &str) -> Vec<Value> {
    let found = abilities(text);
    let ability = found.first().cloned().unwrap_or_default();
    let inner = ability.as_object().and_then(|o| o.values().next()).cloned();
    inner
        .and_then(|a| a.get("effects").cloned())
        .and_then(|e| e.as_array().cloned())
        .unwrap_or_default()
}

#[rstest]
#[case::green_suns_zenith_shuffle(
    "Sorcery",
    "A:SP$ Draw | NumCards$ 1 | SubAbility$ S\nSVar:S:DB$ ChangeZone | Origin$ Stack | Destination$ Library | Shuffle$ True | Defined$ Parent\n",
    json!({"Move": {"what": "This", "to": {"Library": {"place": "Shuffled"}}}})
)]
#[case::healing_technique_exile(
    "Sorcery",
    "A:SP$ Draw | NumCards$ 1 | SubAbility$ S\nSVar:S:DB$ ChangeZone | Origin$ Stack | Destination$ Exile\n",
    json!({"Exile": {"what": "This"}})
)]
#[case::bottom(
    "Instant",
    "A:SP$ Draw | NumCards$ 1 | SubAbility$ S\nSVar:S:DB$ ChangeZone | Origin$ Stack | Destination$ Library | LibraryPosition$ -1 | Defined$ Parent\n",
    json!({"Move": {"what": "This", "to": {"Library": {"place": "Bottom"}}}})
)]
#[case::hand(
    "Sorcery",
    "A:SP$ Draw | NumCards$ 1 | SubAbility$ S\nSVar:S:DB$ ChangeZone | Origin$ Stack | Destination$ Hand | Defined$ Parent\n",
    json!({"Move": {"what": "This", "to": "Hand"}})
)]
fn a_spell_moves_itself_from_the_stack(
    #[case] types: &str,
    #[case] lines: &str,
    #[case] last: Value,
) {
    let text = format!("Name:T\nTypes:{types}\n{lines}");
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(effects(&text).last(), Some(&last));
}

#[test]
fn krark_returns_the_spell_that_was_cast() {
    let text = "Name:T\nTypes:Creature Goblin\nT:Mode$ SpellCast | ValidCard$ Instant,Sorcery | ValidActivatingPlayer$ You | TriggerZones$ Battlefield | Execute$ B | TriggerDescription$ x\nSVar:B:DB$ ChangeZone | Defined$ TriggeredSpellAbility | Origin$ Stack | Destination$ Hand\n";
    assert_eq!(gaps(text), Vec::<String>::new());
    assert_eq!(
        effects(text).last(),
        Some(&json!({"Move": {"what": "Triggering", "to": "Hand"}}))
    );
}

#[rstest]
#[case::remembered("Defined$ Remembered", "defined:Stack-Remembered")]
#[case::replaced("Defined$ ReplacedCard", "defined:Stack-ReplacedCard")]
fn another_name_for_a_spell_stays_a_gap(#[case] defined: &str, #[case] gap: &str) {
    let text = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ Draw | NumCards$ 1 | SubAbility$ S\nSVar:S:DB$ ChangeZone | Origin$ Stack | Destination$ Hand | {defined}\n"
    );
    assert_eq!(gaps(&text), [gap]);
}

#[rstest]
#[case::bojuka_bog("Player", "Graveyard", "Exile", json!({"Exile": {"what": {"Each": [{"In": "Graveyard"}, {"OwnedBy": {"Target": 0}}]}}}))]
#[case::blessed_respite_shuffle("Player", "Graveyard", "Library", json!({"Move": {"what": {"Each": [{"In": "Graveyard"}, {"OwnedBy": {"Target": 0}}]}, "to": {"Library": {"place": "Shuffled"}}}}))]
#[case::opponents_hand("Opponent", "Hand", "Exile", json!({"Exile": {"what": {"Each": [{"In": "Hand"}, {"OwnedBy": {"Target": 0}}]}}}))]
fn a_zone_of_a_targeted_player_moves_whole(
    #[case] tgt: &str,
    #[case] origin: &str,
    #[case] destination: &str,
    #[case] expected: Value,
) {
    let shuffle = if destination == "Library" {
        " | Shuffle$ True"
    } else {
        ""
    };
    let text = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ ChangeZoneAll | ValidTgts$ {tgt} | Origin$ {origin} | Destination$ {destination} | ChangeType$ Card{shuffle}\n"
    );
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(effects(&text).last(), Some(&expected));
}

#[test]
fn a_targeted_battlefield_sweep_stays_a_gap() {
    let text = "Name:T\nTypes:Sorcery\nA:SP$ ChangeZoneAll | ValidTgts$ Player | Origin$ Battlefield | Destination$ Hand | ChangeType$ Creature\n";
    assert_eq!(gaps(text), ["key:ChangeZoneAll.ValidTgts"]);
}

#[test]
fn cultivate_style_search_leaves_the_found_cards_in_the_library() {
    let text = "Name:T\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Library | Destination$ Library | ChangeType$ Land.Basic | ChangeNum$ 2 | RememberChanged$ True | Reveal$ True | Shuffle$ False | SubAbility$ H\nSVar:H:DB$ ChangeZone | Origin$ Library | Destination$ Hand | ChangeType$ Land.IsRemembered | Mandatory$ True | NoLooking$ True\n";
    assert_eq!(gaps(text), Vec::<String>::new());
    let found = effects(text);
    assert_eq!(found.len(), 4, "{found:?}");
    assert!(found[0].get("Search").is_some());
    assert_eq!(
        found[0]["Search"]["count"],
        json!({"min": 0, "max": 2}),
        "up to two"
    );
}

#[test]
fn a_search_to_the_library_without_a_place_or_memory_stays_a_gap() {
    let text = "Name:T\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Library | Destination$ Library | ChangeType$ Land.Basic | Shuffle$ False\n";
    assert_eq!(gaps(text), ["key:ChangeZone.LibraryPosition"]);
}
