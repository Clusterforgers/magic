//! Goal: round 10 pack g10b binds what a counter, an attach, an amass and a sacrifice of named objects did ("if that spell
//! is countered this way", "if you do", "the amassed Army") and the cards a dig revealed as they are after their moves, so
//! the lines after them read the outcome; a countered ability, a choice that excludes what was remembered and a found card
//! that goes elsewhere stay gaps.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// Asserts that the card reads `expected` with no gap, or stays the gap `gap` when one is given.
fn reads(types: &str, lines: &str, expected: &[&str], gap: &str) {
    let text = format!("Name:Subject\nTypes:{types}\n{lines}\n");
    let found = gaps(&text);
    let json = serde_json::to_string(&abilities(&text)).unwrap();
    if gap.is_empty() {
        assert!(found.is_empty(), "{found:?}");
        for part in expected {
            assert!(json.contains(part), "{part} in {json}");
        }
    } else {
        assert!(found.iter().any(|g| g.contains(gap)), "{found:?} {json}");
    }
}

#[rstest]
#[case::countered_this_way_exiled_and_free(
    "Instant",
    "A:SP$ Counter | TargetType$ Spell | ValidTgts$ Card | RememberCountered$ True | Destination$ Exile | SubAbility$ E\nSVar:E:DB$ Effect | RememberObjects$ Remembered | StaticAbilities$ P | Duration$ Permanent | ForgetOnMoved$ Exile | SubAbility$ C\nSVar:P:Mode$ Continuous | MayPlay$ True | MayPlayWithoutManaCost$ True | Affected$ Card.IsRemembered | AffectedZone$ Exile\nSVar:C:DB$ Cleanup | ClearRemembered$ True",
    &[r#"{"Counter":{"as":"countered","what":{"Target":0}}}"#, r#""only":{"Bound":"countered"}"#, r#""MoveTo":"Exile""#],
    ""
)]
#[case::if_countered_its_controller_may(
    "Instant",
    "A:SP$ Counter | TargetType$ Spell | ValidTgts$ Card | RememberCountered$ True | SubAbility$ D\nSVar:D:DB$ Draw | ConditionDefined$ Remembered | ConditionPresent$ Card | ConditionCompare$ GE1",
    &[r#""subject":{"Bound":"countered"}"#],
    ""
)]
#[case::the_spell_of_a_cast_trigger(
    "Creature",
    "T:Mode$ SpellCast | ValidCard$ Card | ValidActivatingPlayer$ Opponent | Execute$ T\nSVar:T:DB$ Counter | Defined$ TriggeredSpellAbility | RememberCountered$ True | SubAbility$ D\nSVar:D:DB$ Draw | ConditionDefined$ Remembered | ConditionPresent$ Card",
    &[r#"{"Counter":{"as":"countered","what":"Triggering"}}"#],
    ""
)]
#[case::a_countered_ability_names_its_source_in_forge(
    "Instant",
    "A:SP$ Counter | TargetType$ Activated,Triggered | ValidTgts$ Card | RememberCountered$ True | SubAbility$ D\nSVar:D:DB$ Draw | ConditionDefined$ Remembered | ConditionPresent$ Card",
    &[],
    "key:Counter.RememberCountered-of-an-ability"
)]
#[case::attached_if_you_do(
    "Artifact",
    "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ A | OptionalDecider$ You\nSVar:A:DB$ Attach | ValidTgts$ Creature.YouCtrl | RememberAttached$ True | SubAbility$ D\nSVar:D:DB$ Draw | ConditionDefined$ Remembered | ConditionPresent$ Card | ConditionCompare$ GE1",
    &[r#"{"Attach":{"as":"attached","to":{"Target":0},"what":"This"}}"#, r#""subject":{"Bound":"attached"}"#],
    ""
)]
#[case::an_equipment_chosen_and_unattached_later(
    "Sorcery",
    "A:SP$ Attach | Optional$ True | Choices$ Equipment.YouCtrl | Defined$ Self | RememberAttached$ True | SubAbility$ D\nSVar:D:DB$ Draw | ConditionDefined$ Remembered | ConditionPresent$ Card",
    &[r#""as":"attached"#, r#""May""#],
    ""
)]
#[case::a_choice_that_excludes_the_remembered(
    "Sorcery",
    "A:SP$ Attach | Choices$ Equipment.YouCtrl+!IsRemembered | Defined$ Self | RememberAttached$ True",
    &[],
    "key:Attach.RememberAttached-with-remembered-choices"
)]
#[case::the_amassed_army(
    "Sorcery",
    "A:SP$ Amass | Type$ Orc | Num$ 2 | RememberAmass$ True | SubAbility$ D\nSVar:D:DB$ Draw | NumCards$ X\nSVar:X:Remembered$CardPower",
    &[r#""as":"amassed""#, r#"{"Bound":"amassed"}"#],
    ""
)]
#[case::revealed_until_a_land_all_to_the_bottom(
    "Artifact",
    "A:AB$ DigUntil | Cost$ T | Valid$ Land | RememberRevealed$ True | RevealedDestination$ Library | RevealedLibraryPosition$ -1 | SubAbility$ D\nSVar:D:DB$ DealDamage | ValidTgts$ Any | NumDmg$ X\nSVar:X:Remembered$Valid Card.nonLand",
    &[r#"{"Move":{"as":"revealed3","#, r#"{"Is":{"Bound":"revealed3"}}"#, r#"{"Not":{"Type":"Land"}}"#],
    ""
)]
#[case::the_found_card_goes_elsewhere(
    "Sorcery",
    "A:SP$ DigUntil | Valid$ Artifact | FoundDestination$ Battlefield | RevealedDestination$ Library | RevealedLibraryPosition$ -1 | RememberRevealed$ True | SubAbility$ D\nSVar:D:DB$ LoseLife | LifeAmount$ X\nSVar:X:Remembered$Amount",
    &[],
    "key:DigUntil.RememberRevealed-with-a-found-destination"
)]
#[case::the_top_card_revealed_and_kept(
    "Sorcery",
    "A:SP$ Dig | DigNum$ 1 | Reveal$ True | RememberRevealed$ True | DestinationZone$ Library | LibraryPosition$ 0 | SubAbility$ D\nSVar:D:DB$ Draw | ConditionDefined$ Remembered | ConditionPresent$ Creature",
    &[r#"{"AnyOf":[{"Is":{"Bound":"revealed4"}},{"Is":{"Bound":"revealed5"}}]}"#],
    ""
)]
#[case::dug_without_a_reveal(
    "Sorcery",
    "A:SP$ Dig | DigNum$ 1 | RememberRevealed$ True | DestinationZone$ Library | LibraryPosition$ 0",
    &[],
    "key:Dig.RememberRevealed-shape"
)]
#[case::sacrifice_all_then_that_many(
    "Sorcery",
    "A:SP$ SacrificeAll | ValidCards$ Creature.YouCtrl | RememberSacrificed$ True | SubAbility$ D\nSVar:D:DB$ Draw | NumCards$ X\nSVar:X:Remembered$Amount",
    &[r#""as":"sacrificed""#, r#"{"Bound":"sacrificed~moved"}"#],
    ""
)]
#[case::sacrifice_itself_if_you_do(
    "Creature",
    "A:AB$ SacrificeAll | Cost$ 1 | Defined$ Self | RememberSacrificed$ True | SubAbility$ D\nSVar:D:DB$ Draw | ConditionDefined$ Remembered | ConditionPresent$ Card",
    &[r#"{"Sacrifice":{"as":"sacrificed","filter":[{"Is":"This"}]"#],
    ""
)]
fn a_step_binds_what_it_did(
    #[case] types: &str,
    #[case] lines: &str,
    #[case] expected: &[&str],
    #[case] gap: &str,
) {
    reads(types, lines, expected, gap);
}
