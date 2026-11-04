//! Goal: statics and replacements of round 4 map to the nodes that carry them, and the shapes the nodes cannot carry stay gaps.
use rstest::rstest;

use super::{abilities, gaps};

fn creature(rest: &str) -> String {
    format!("Name:T\nTypes:Creature Wall\nPT:1/1\n{rest}\n")
}

#[rstest]
#[case::can_attack_this_turn(
    "A:AB$ Effect | Cost$ W | StaticAbilities$ S | Duration$ UntilHostLeavesPlayOrEOT\nSVar:S:Mode$ CanAttackDefender | ValidCard$ Card.EffectSource",
    r#""duration":"UntilEndOfTurn","rule":{"Combat":{"quirk":"AttackDespiteDefender","who":"This"}}"#
)]
#[case::cant_be_blocked_except(
    "A:AB$ Effect | Cost$ 1 | StaticAbilities$ S | Duration$ UntilHostLeavesPlayOrEOT\nSVar:S:Mode$ CantBlockBy | ValidAttacker$ Card.EffectSource | ValidBlocker$ Creature.withoutHaste",
    r#""affected":"This","duration":"UntilEndOfTurn""#
)]
fn an_effect_about_its_host_lasts_until_the_end_of_the_turn(
    #[case] rest: &str,
    #[case] expected: &str,
) {
    let text = creature(rest);
    let found = abilities(&text);
    assert_eq!(found.len(), 1, "{:?}", gaps(&text));
    assert!(found[0].to_string().contains(expected), "{}", found[0]);
}

#[rstest]
#[case::a_mode_it_does_not_know(
    "A:AB$ Effect | Cost$ W | StaticAbilities$ S | Duration$ UntilHostLeavesPlayOrEOT\nSVar:S:Mode$ NoSuchMode | ValidCard$ Card.EffectSource",
    "static:NoSuchMode"
)]
#[case::a_duration_it_does_not_know(
    "A:AB$ Effect | Cost$ W | StaticAbilities$ S | Duration$ UntilYourNextUpkeep\nSVar:S:Mode$ CanAttackDefender | ValidCard$ Card.EffectSource",
    "duration:UntilYourNextUpkeep"
)]
fn shapes_the_nodes_cannot_carry_stay_gaps(#[case] rest: &str, #[case] gap: &str) {
    let text = creature(rest);
    assert!(abilities(&text).is_empty(), "{:?}", abilities(&text));
    assert_eq!(gaps(&text), [gap]);
}

const EXILE: &str = "SVar:Exile:DB$ ChangeZone | Hidden$ True | Origin$ All | Destination$ Exile | Defined$ ReplacedCard";

#[rstest]
#[case::from_anywhere(
    "R:Event$ Moved | ValidCard$ Card.Self | Destination$ Graveyard | ReplaceWith$ Exile",
    r#"{"Replacement":{"by":{"MoveTo":"Exile"},"when":{"ZoneChange":{"to":"Graveyard","what":["This"]}}}}"#
)]
fn a_card_that_is_exiled_instead_of_going_to_a_graveyard(
    #[case] line: &str,
    #[case] expected: &str,
) {
    let text = creature(&format!("{line}\n{EXILE}"));
    let found = abilities(&text);
    assert_eq!(found.len(), 1, "{:?}", gaps(&text));
    assert_eq!(found[0].to_string(), expected);
}

#[rstest]
#[case::only_from_the_battlefield(
    "R:Event$ Moved | ValidCard$ Card.Self | Origin$ Battlefield | Destination$ Graveyard | ReplaceWith$ Exile",
    "key:Replacement.Moved.Origin"
)]
#[case::shuffled_into_the_library_instead(
    "R:Event$ Moved | ValidCard$ Card.Self | Destination$ Graveyard | ReplaceWith$ Shuffle\nSVar:Shuffle:DB$ ChangeZone | Hidden$ True | Origin$ All | Destination$ Library | Defined$ ReplacedCard | Reveal$ True | Shuffle$ True",
    "key:Replacement.Reveal"
)]
fn a_move_to_a_graveyard_of_another_shape_stays_a_gap(#[case] line: &str, #[case] gap: &str) {
    let text = creature(&format!("{line}\n{EXILE}"));
    assert!(abilities(&text).is_empty());
    assert!(
        gaps(&text).iter().any(|g| g.starts_with(gap)),
        "{:?}",
        gaps(&text)
    );
}
