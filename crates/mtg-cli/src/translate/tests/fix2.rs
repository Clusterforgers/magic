//! Goal: lines that were read wrongly are right or a named gap: a chosen card from a hand is not optional unless the card says so, the controller of what a per-player target set moved is a gap, and restricted mana for a spell from "your graveyard" names the caster's own.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

#[rstest]
#[case::must_put_one(
    "Target player puts a card from their hand on top of their library.",
    1
)]
#[case::may_put_one("You may put a card from your hand on top of your library.", 0)]
#[case::up_to_one("Put up to one card from your hand on top of your library.", 0)]
fn a_card_picked_from_a_hand_is_optional_only_when_the_card_says_so(
    #[case] description: &str,
    #[case] least: u32,
) {
    let script = format!(
        "Name:Subject\nTypes:Artifact\nA:AB$ ChangeZone | Cost$ T | ValidTgts$ Player | Origin$ Hand | Destination$ Library | ChangeType$ Card | ChangeNum$ 1 | Chooser$ Targeted | LibraryPosition$ 0 | SpellDescription$ {description}\nOracle:{description}\n"
    );
    let text = serde_json::to_string(&abilities(&script)).unwrap();
    let wanted = if least == 0 {
        r#""count":{"max":1,"min":0}"#
    } else {
        r#""count""#
    };
    assert_eq!(text.contains(wanted), least == 0, "{text}");
}

#[test]
fn the_controller_of_what_a_target_per_player_exiled_is_a_gap() {
    let script = "Name:Subject\nTypes:Artifact\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ E\nSVar:E:DB$ ChangeZone | Origin$ Battlefield | Destination$ Exile | ValidTgts$ Permanent.OppCtrl | TargetMin$ 0 | TargetMax$ OneEach | TargetsForEachPlayer$ True | RememberLKI$ True | SubAbility$ D\nSVar:D:DB$ Draw | Defined$ RememberedController | SubAbility$ C\nSVar:C:DB$ Cleanup | ClearRemembered$ True\nSVar:OneEach:PlayerCountOpponents$Amount\n";
    assert_eq!(
        gaps(script),
        ["defined:RememberedController-of-a-target-per-player"]
    );
}

#[rstest]
#[case::your_graveyard("wasCastFromYourGraveyard", "FromYourGraveyard")]
#[case::a_graveyard("wasCastFromGraveyard", "FromGraveyard")]
fn mana_for_spells_from_a_graveyard_names_whose(#[case] word: &str, #[case] fact: &str) {
    let script = format!(
        "Name:Subject\nTypes:Creature Demon\nPT:1/1\nA:AB$ Mana | Cost$ PayLife<1> | Produced$ C | RestrictValid$ Spell.{word} | SpellDescription$ x\n"
    );
    let text = serde_json::to_string(&abilities(&script)).unwrap();
    assert!(text.contains(&format!(r#""facts":["{fact}"]"#)), "{text}");
}
