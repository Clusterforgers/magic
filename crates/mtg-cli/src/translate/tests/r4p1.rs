//! Goal: zone-change lines of round 4 (optional moves, searches and choices for another player, moves from every zone) translate to what the card does, and a shape the nodes cannot carry stays a named gap.
use rstest::rstest;

use super::{abilities, gaps};

fn card(types: &str, lines: &str) -> String {
    format!("Name:T\nTypes:{types}\n{lines}\n")
}

#[rstest]
#[case::may_put_on_top(
    "A:SP$ ChangeZone | ValidTgts$ Card.YouOwn | TgtZone$ Graveyard | Origin$ Graveyard | Destination$ Library | LibraryPosition$ 0 | Optional$ True | SpellDescription$ You may put target card on top of your library.",
    r#""May""#
)]
#[case::optional_you(
    "A:SP$ ChangeZone | ValidTgts$ Creature.YouCtrl | TgtZone$ Graveyard | Origin$ Graveyard | Destination$ Hand | Optional$ You | SpellDescription$ You may return up to one target creature card to your hand.",
    r#""May""#
)]
#[case::not_optional(
    "A:SP$ ChangeZone | ValidTgts$ Card.YouOwn | TgtZone$ Graveyard | Origin$ Graveyard | Destination$ Hand | SpellDescription$ Return target card to your hand.",
    r#""Move""#
)]
fn an_optional_move_is_a_may(#[case] lines: &str, #[case] node: &str) {
    let script = card("Sorcery", lines);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let text = found[0].to_string();
    assert!(text.contains(node), "{text}");
    assert_eq!(text.contains("May"), node == r#""May""#, "{text}");
}

#[rstest]
#[case::targeted_controller(
    "A:SP$ ChangeZone | Origin$ Battlefield | Destination$ Exile | ValidTgts$ Creature | SubAbility$ S\nSVar:S:DB$ ChangeZone | Optional$ True | Origin$ Library | Destination$ Battlefield | Tapped$ True | ChangeType$ Land.Basic | DefinedPlayer$ TargetedController | ShuffleNonMandatory$ True",
    &[r#""who":{"ControllerOf":{"Target":0}}"#, r#""Shuffle":{"who":{"ControllerOf":{"Target":0}}}"#]
)]
#[case::each_player(
    "A:SP$ ChangeZone | Origin$ Library | Destination$ Battlefield | ChangeType$ Land.Basic | DefinedPlayer$ Player | Shuffle$ True",
    &[r#""over":"EachPlayer""#, r#""Shuffle":{"who":{"Bound":"player"}}"#]
)]
#[case::each_opponent_may(
    "A:SP$ ChangeZone | Optional$ True | Origin$ Library | Destination$ Battlefield | ChangeType$ Land.Basic | DefinedPlayer$ Player.Opponent | ChangeNum$ 1 | ShuffleNonMandatory$ True",
    &[r#""over":"Opponents""#, r#""May":{"effects""#, r#""who":{"Bound":"player"}}"#]
)]
fn another_player_searches_their_own_library(#[case] lines: &str, #[case] parts: &[&str]) {
    let script = card("Sorcery", lines);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let text = found[0].to_string();
    for part in parts {
        assert!(text.contains(part), "{part} in {text}");
    }
}

#[rstest]
#[case::chooser_apart(
    "A:SP$ ChangeZone | DefinedPlayer$ TriggeredTarget | Origin$ Library | Destination$ Hand | ChangeType$ Card | Chooser$ You",
    "key:ChangeZone.Search.Chooser"
)]
#[case::unknown_player(
    "A:SP$ ChangeZone | DefinedPlayer$ Banana | Origin$ Library | Destination$ Hand | ChangeType$ Card",
    "key:ChangeZone.Search.DefinedPlayer"
)]
#[case::unknown_optional(
    "A:SP$ ChangeZone | ValidTgts$ Card.YouOwn | TgtZone$ Graveyard | Origin$ Graveyard | Destination$ Hand | Optional$ Maybe",
    "key:ChangeZone.Optional"
)]
fn shapes_that_do_not_map_stay_named_gaps(#[case] lines: &str, #[case] gap: &str) {
    let script = card("Sorcery", lines);
    assert!(abilities(&script).is_empty(), "{:?}", abilities(&script));
    assert_eq!(gaps(&script), [gap]);
}

#[rstest]
#[case::returned_from_anywhere(
    "A:SP$ ChangeZone | ValidTgts$ Creature | Origin$ Battlefield | Destination$ Exile | RememberChanged$ True | SubAbility$ R\nSVar:R:DB$ ChangeZone | Defined$ Remembered | Origin$ All | Destination$ Battlefield | SubAbility$ C\nSVar:C:DB$ Cleanup | ClearRemembered$ True",
    true
)]
#[case::taken_under_your_control(
    "A:SP$ ChangeZone | ValidTgts$ Creature | Origin$ Battlefield | Destination$ Exile | RememberChanged$ True | SubAbility$ R\nSVar:R:DB$ ChangeZone | Defined$ Remembered | Origin$ All | Destination$ Battlefield | GainControl$ True | SubAbility$ C\nSVar:C:DB$ Cleanup | ClearRemembered$ True",
    true
)]
#[case::a_target_of_any_zone_has_no_zone_to_filter(
    "A:SP$ ChangeZone | ValidTgts$ Creature | Origin$ All | Destination$ Battlefield",
    false
)]
#[case::a_pick_by_type_has_no_zone(
    "A:SP$ ChangeZone | ChangeType$ Creature | Origin$ All | Destination$ Battlefield",
    false
)]
fn a_move_from_every_zone_names_the_objects(#[case] lines: &str, #[case] maps: bool) {
    let script = card("Instant", lines);
    assert_eq!(
        abilities(&script).len(),
        usize::from(maps),
        "{:?}",
        gaps(&script)
    );
    if !maps {
        assert_eq!(gaps(&script), ["zone:Origin=All"]);
    }
}

#[rstest]
#[case::a_player_the_parent_targeted(
    "A:SP$ LoseLife | ValidTgts$ Opponent | LifeAmount$ 1 | SubAbility$ S\nSVar:S:DB$ ChangeZone | Origin$ Hand | Destination$ Exile | DefinedPlayer$ Targeted | Chooser$ You | ChangeType$ Card.nonLand | ChangeNum$ 1 | Mandatory$ True",
    r#"{"OwnedBy":{"Target":0}}"#
)]
#[case::the_controller_of_the_permanent_the_parent_targeted(
    "A:SP$ Destroy | ValidTgts$ Creature | SubAbility$ S\nSVar:S:DB$ ChangeZone | Origin$ Hand | Destination$ Exile | DefinedPlayer$ TargetedController | Chooser$ You | ChangeType$ Card | ChangeNum$ 1 | Mandatory$ True",
    r#"{"OwnedBy":{"ControllerOf":{"Target":0}}}"#
)]
#[case::each_other_player(
    "A:SP$ ChangeZone | Origin$ Hand | Destination$ Battlefield | ChangeType$ Artifact,Enchantment | DefinedPlayer$ Player.Other | ChangeNum$ 1",
    r#""over":"Opponents""#
)]
fn a_hidden_zone_choice_is_among_the_cards_of_the_player_named(
    #[case] lines: &str,
    #[case] part: &str,
) {
    let script = card("Sorcery", lines);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    assert!(
        found[0].to_string().contains(part),
        "{part} in {}",
        found[0]
    );
}

#[rstest]
#[case::the_target_is_not_a_player(
    "A:SP$ Destroy | ValidTgts$ Creature | SubAbility$ S\nSVar:S:DB$ ChangeZone | Origin$ Hand | Destination$ Exile | DefinedPlayer$ Targeted | Chooser$ You | ChangeType$ Card | ChangeNum$ 1 | Mandatory$ True",
    "defined:TargetedPlayer-without-a-player-target"
)]
#[case::two_targets_are_not_one_player(
    "A:SP$ LoseLife | ValidTgts$ Opponent | TargetMin$ 0 | TargetMax$ 2 | LifeAmount$ 1 | SubAbility$ S\nSVar:S:DB$ ChangeZone | Origin$ Hand | Destination$ Exile | DefinedPlayer$ Targeted | Chooser$ You | ChangeType$ Card | ChangeNum$ 1 | Mandatory$ True",
    "key:ChangeZone.Choose.DefinedPlayer"
)]
fn a_player_that_is_not_one_player_stays_a_named_gap(#[case] lines: &str, #[case] gap: &str) {
    let script = card("Sorcery", lines);
    assert!(abilities(&script).is_empty(), "{:?}", abilities(&script));
    assert_eq!(gaps(&script), [gap]);
}

#[rstest]
#[case::the_caster_chooses_from_a_targeted_hand(
    "A:SP$ ChangeZone | Origin$ Hand | Destination$ Library | LibraryPosition$ 0 | ValidTgts$ Opponent | ChangeType$ Card | ChangeNum$ 1 | Mandatory$ True",
    &[r#"{"OwnedBy":{"Target":0}}"#, r#""Choose":{"as":"picked""#]
)]
#[case::the_target_chooses_from_their_hand(
    "A:SP$ ChangeZone | Origin$ Hand | Destination$ Library | LibraryPosition$ 0 | ValidTgts$ Opponent | Chooser$ Targeted | ChangeType$ Card | ChangeNum$ 1 | Mandatory$ True",
    &[r#""who":{"Target":0}"#, r#"{"OwnedBy":{"Target":0}}"#]
)]
#[case::the_target_searches_their_own_library(
    "A:SP$ ChangeZone | ValidTgts$ Player | Origin$ Library | Destination$ Battlefield | Tapped$ True | ChangeType$ Land.Basic | Chooser$ Targeted",
    &[r#""who":{"Target":0}"#, r#""Shuffle":{"who":{"Target":0}}"#]
)]
#[case::the_player_a_parent_targeted_searches(
    "A:SP$ LoseLife | ValidTgts$ Player | LifeAmount$ 1 | SubAbility$ S\nSVar:S:DB$ ChangeZone | Origin$ Library | Destination$ Battlefield | DefinedPlayer$ ParentTarget | ChangeType$ Land.Basic",
    &[r#""who":{"Target":0}"#]
)]
fn the_targeted_player_is_the_owner_of_the_cards_and_may_be_the_chooser(
    #[case] lines: &str,
    #[case] parts: &[&str],
) {
    let script = card("Sorcery", lines);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    for part in parts {
        assert!(
            found[0].to_string().contains(part),
            "{part} in {}",
            found[0]
        );
    }
}

#[rstest]
#[case::you_search_their_library(
    "A:SP$ ChangeZone | ValidTgts$ Opponent | Origin$ Library | Destination$ Battlefield | ChangeType$ Creature | GainControl$ True",
    "key:ChangeZone.Search.ValidTgts"
)]
#[case::a_chooser_apart_from_the_target(
    "A:SP$ ChangeZone | ValidTgts$ Player | Origin$ Library | Destination$ Exile | ChangeType$ Card | Chooser$ You",
    "key:ChangeZone.Search.ValidTgts"
)]
#[case::several_targets_search(
    "A:SP$ ChangeZone | ValidTgts$ Player | TargetMin$ 0 | TargetMax$ 2 | Chooser$ Targeted | Origin$ Library | Destination$ Hand | ChangeType$ Card",
    "key:ChangeZone.Search.ValidTgts"
)]
fn a_search_of_a_targets_library_stays_a_named_gap(#[case] lines: &str, #[case] gap: &str) {
    let script = card("Sorcery", lines);
    assert!(abilities(&script).is_empty(), "{:?}", abilities(&script));
    assert_eq!(gaps(&script), [gap]);
}

#[rstest]
#[case::until_the_source_leaves(
    "A:SP$ ChangeZone | ValidTgts$ Creature | Origin$ Battlefield | Destination$ Exile | Duration$ UntilHostLeavesPlay",
    true
)]
#[case::another_duration(
    "A:SP$ ChangeZone | ValidTgts$ Creature | Origin$ Battlefield | Destination$ Exile | Duration$ UntilYourNextTurn",
    false
)]
#[case::not_an_exile(
    "A:SP$ ChangeZone | ValidTgts$ Creature | Origin$ Battlefield | Destination$ Hand | Duration$ UntilHostLeavesPlay",
    false
)]
#[case::not_from_the_battlefield(
    "A:SP$ ChangeZone | ValidTgts$ Card | Origin$ Graveyard | Destination$ Exile | Duration$ UntilHostLeavesPlay",
    false
)]
fn an_exile_with_a_duration_names_it_or_stays_a_gap(#[case] lines: &str, #[case] maps: bool) {
    let script = card("Sorcery", lines);
    let found = abilities(&script);
    assert_eq!(found.len(), usize::from(maps), "{:?}", gaps(&script));
    if maps {
        assert!(
            found[0]
                .to_string()
                .contains(r#""duration":"WhileSourceOnBattlefield""#)
        );
    } else {
        assert_eq!(gaps(&script), ["key:ChangeZone.Duration"]);
    }
}

#[rstest]
#[case::the_flag_on_a_list(
    "A:SP$ ChangeZoneAll | ChangeType$ Card | Origin$ Hand,Graveyard | Destination$ Library | Shuffle$ True | UseAllOriginZones$ True",
    true
)]
#[case::an_opponents_graveyard(
    "A:SP$ ChangeZoneAll | Origin$ Graveyard | Destination$ Exile | Defined$ Opponent | ChangeType$ Card",
    true
)]
#[case::the_player_a_parent_targeted(
    "A:SP$ LoseLife | ValidTgts$ Player | LifeAmount$ 1 | SubAbility$ S\nSVar:S:DB$ ChangeZoneAll | Origin$ Hand | Destination$ Exile | Defined$ Targeted | ChangeType$ Card",
    true
)]
#[case::battlefield_by_player(
    "A:SP$ ChangeZoneAll | Origin$ Battlefield | Destination$ Exile | Defined$ Opponent | ChangeType$ Card",
    false
)]
#[case::an_unknown_player(
    "A:SP$ ChangeZoneAll | Origin$ Graveyard | Destination$ Exile | Defined$ Banana | ChangeType$ Card",
    false
)]
#[case::an_unknown_flag(
    "A:SP$ ChangeZoneAll | Origin$ Graveyard | Destination$ Exile | UseAllOriginZones$ Maybe",
    false
)]
fn a_move_of_all_cards_maps_or_stays_a_gap(#[case] lines: &str, #[case] maps: bool) {
    let script = card("Sorcery", lines);
    assert_eq!(
        abilities(&script).len(),
        usize::from(maps),
        "{:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::another_destination_says_nothing("DestinationZone$ Hand | LibraryPosition$ 0", true)]
#[case::the_default_rest_position(
    "DestinationZone$ Battlefield | DestinationZone2$ Library | LibraryPosition$ -1",
    true
)]
#[case::a_library_destination_without_a_position("DestinationZone$ Library", true)]
#[case::a_library_destination_below_the_top("DestinationZone$ Library | LibraryPosition$ 3", false)]
fn a_library_position_of_a_dig(#[case] places: &str, #[case] maps: bool) {
    let script = card(
        "Sorcery",
        &format!("A:SP$ Dig | DigNum$ 3 | ChangeNum$ 1 | {places}"),
    );
    assert_eq!(
        abilities(&script).len(),
        usize::from(maps),
        "{:?}",
        gaps(&script)
    );
    if !maps {
        assert_eq!(gaps(&script), ["key:Dig.LibraryPosition"]);
    }
}

#[rstest]
#[case::the_revealed_cards_go_together(
    "A:SP$ DigUntil | Valid$ Card.nonLand | RevealedDestination$ Library | RevealedLibraryPosition$ -1",
    1
)]
#[case::a_found_card_goes_apart(
    "A:SP$ DigUntil | Valid$ Card.nonLand | FoundDestination$ Hand | RevealedDestination$ Graveyard",
    2
)]
fn a_dig_until_moves_the_found_card_only_when_it_has_a_destination(
    #[case] lines: &str,
    #[case] moves: usize,
) {
    let script = card("Sorcery", lines);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    assert_eq!(
        found[0].to_string().matches(r#""Move""#).count(),
        moves,
        "{}",
        found[0]
    );
}

#[test]
fn a_found_destination_it_does_not_know_stays_a_named_gap() {
    let script = card(
        "Sorcery",
        "A:SP$ DigUntil | Valid$ Card | FoundDestination$ Command | RevealedDestination$ Graveyard",
    );
    assert_eq!(abilities(&script), Vec::<serde_json::Value>::new());
    assert_eq!(gaps(&script), ["key:DigUntil.FoundDestination=Command"]);
}
