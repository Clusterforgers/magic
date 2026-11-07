//! Goal: round 13 pack e reads the small forms of the tail: an empty spell line, who untaps, a cost worded per ability,
//! each player's own draw choice, an optional investigate and the animate keys that change types, combat and leaving.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a card with the given header and lines as compact JSON, and its gaps.
fn card(types: &str, lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:{types}\n{lines}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

#[test]
fn a_spell_line_that_does_nothing_leaves_the_keywords_alone() {
    let (json, found) = card("Sorcery", "A:SP$ Pump | StackDescription$ None\nK:Cascade");
    assert!(found.is_empty(), "{found:?}");
    assert!(!json.contains("Spell"), "{json}");
    assert!(json.contains("Cascade"), "{json}");
}

#[test]
fn a_spell_line_with_a_target_is_still_a_spell() {
    let (json, found) = card(
        "Instant",
        "A:SP$ Pump | ValidTgts$ Creature | NumAtt$ +1 | NumDef$ +1",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains("Spell"), "{json}");
}

#[test]
fn the_player_who_untaps_changes_nothing() {
    let (json, found) = card(
        "Sorcery",
        "A:SP$ UntapAll | ControllerUntaps$ True | ValidTgts$ Player | ValidCards$ Land.Basic",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(r#""ControlledBy":{"Target":0}"#), "{json}");
}

#[test]
fn a_card_that_words_two_sacrifices_differently_reads_each_by_its_own_mana() {
    let text = "Name:Subject\nTypes:Creature Imp\nA:AB$ DealDamage | Cost$ 1 R Sac<1/Creature.Other/another creature> | ValidTgts$ Player | NumDmg$ 2\nA:AB$ ChangeZone | Cost$ 1 B Sac<1/Creature> | Origin$ Graveyard | Destination$ Hand | Defined$ Self | ActivationZone$ Graveyard\nOracle:{1}{R}, Sacrifice another creature: Subject deals 2 damage to target player.\\n{1}{B}, Sacrifice a creature: Return Subject from your graveyard to your hand.\n";
    let found = gaps(text);
    assert!(found.is_empty(), "{found:?}");
}

#[rstest]
#[case::each_other_player("Player.Other", "Opponents")]
#[case::each_player("Player", "EachPlayer")]
fn each_player_chooses_their_own_up_to_number_of_cards(#[case] who: &str, #[case] over: &str) {
    let (json, found) = card(
        "Sorcery",
        &format!("A:SP$ Draw | Defined$ {who} | NumCards$ 3 | Upto$ True"),
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(&format!(r#""over":"{over}""#)), "{json}");
    assert!(json.contains(r#""Number":[0,3]"#), "{json}");
}

#[test]
fn an_optional_investigate_is_a_may() {
    let (json, found) = card(
        "Creature Detective",
        "T:Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Card.Self | Execute$ TrigInvestigate | TriggerDescription$ When this dies, you may investigate.\nSVar:TrigInvestigate:DB$ Investigate | Optional$ True",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(r#""May""#), "{json}");
}

#[rstest]
#[case::a_card_type("Artifact", r#""RemoveType":"Artifact""#)]
#[case::the_creature_type_llama_a_land_becomes("Llama", "Llama")]
fn an_animate_line_removes_a_card_type_or_adds_a_type_no_card_prints(
    #[case] word: &str,
    #[case] expected: &str,
) {
    let line = if word == "Llama" {
        "A:AB$ Animate | Cost$ 2 G W | Defined$ Self | Power$ 3 | Toughness$ 3 | Types$ Creature,Llama | RemoveCreatureTypes$ True".to_owned()
    } else {
        format!(
            "A:AB$ Animate | Cost$ U | ValidTgts$ Creature | RemoveTypes$ {word} | Colors$ Blue | OverwriteColors$ True"
        )
    };
    let (json, found) = card("Land", &line);
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[test]
fn an_animate_line_that_removes_a_subtype_is_still_a_gap() {
    let (_, found) = card(
        "Artifact Equipment",
        "A:AB$ Animate | Cost$ 0 | Defined$ Self | Types$ Creature | RemoveTypes$ Equipment",
    );
    assert_eq!(found, vec!["key:Animate.RemoveTypes".to_owned()]);
}

#[test]
fn an_animated_land_must_be_blocked_if_able() {
    let (json, found) = card(
        "Sorcery",
        "A:SP$ Animate | ValidTgts$ Land.YouCtrl | Power$ 4 | Toughness$ 4 | Types$ Creature | HiddenKeywords$ CARDNAME must be blocked if able.",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(r#""MustBeBlocked":"AtLeastOne""#), "{json}");
}

#[test]
fn a_returned_creature_is_exiled_if_it_would_leave() {
    let (json, found) = card(
        "Artifact",
        "A:AB$ ChangeZone | Cost$ T | ValidTgts$ Creature.YouOwn | Origin$ Graveyard | Destination$ Battlefield | RememberChanged$ True | SubAbility$ DBUnearthed\nSVar:DBUnearthed:DB$ Animate | Defined$ Remembered | Keywords$ Haste | LeaveBattlefield$ Exile | sVars$ MustAttack | Duration$ Permanent | AtEOT$ Exile | SubAbility$ DBCleanup\nSVar:MustAttack:SVar:MustAttack:True\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(r#""Is":{"Bound":"moved"}"#), "{json}");
    assert!(json.contains(r#""MoveTo":"Exile""#), "{json}");
}

#[test]
fn a_sub_ability_variable_that_is_not_a_hint_is_a_gap() {
    let (_, found) = card(
        "Artifact",
        "A:AB$ Animate | Cost$ 1 | Defined$ Self | Types$ Creature | Power$ 2 | Toughness$ 2 | sVars$ Count\nSVar:Count:Count$Valid Creature",
    );
    assert_eq!(found, vec!["key:Animate.sVars".to_owned()]);
}
