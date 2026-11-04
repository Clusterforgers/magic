//! Goal: piece P6 of the rules plan translates its constructs, one rstest table per rule.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a sorcery with the given spell line (and its `SVar` lines) as compact JSON, and its gaps.
fn sorcery(lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:Sorcery\n{lines}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

#[rstest]
#[case::defined_once(
    "A:SP$ ChangeZone | Origin$ Graveyard | Destination$ Exile | ChangeType$ Card.YouOwn | RememberChanged$ True | SubAbility$ P\nSVar:P:DB$ Play | Defined$ Remembered | WithoutManaCost$ True | Optional$ True | ValidSA$ Spell",
    &["CastFromZone", "\"free\":true"],
    &["RepeatEach", "\"play\""]
)]
#[case::all_exiled_with_source(
    "A:SP$ Draw | SubAbility$ P\nSVar:P:DB$ Play | Valid$ Card.YouOwn | ValidSA$ Spell | ValidZone$ Exile | WithoutManaCost$ True | Amount$ All | Controller$ You | Optional$ True",
    &["RepeatEach", "CastFromZone"],
    &["\"Choose\"", "\"play\""]
)]
#[case::up_to_three_from_hand(
    "A:SP$ Draw | SubAbility$ P\nSVar:P:DB$ Play | Valid$ Card.YouCtrl+nonLand | ValidZone$ Hand | Controller$ You | WithoutManaCost$ True | Amount$ 3 | Optional$ True",
    &["\"Choose\"", "\"max\":3", "RepeatEach"],
    &[]
)]
#[case::one_chosen_from_graveyard(
    "A:SP$ Draw | SubAbility$ P\nSVar:P:DB$ Play | Valid$ Card.YouOwn | ValidZone$ Graveyard | ValidSA$ Spell | WithoutManaCost$ True | Optional$ True",
    &["\"Choose\"", "\"max\":1", "CastFromZone"],
    &["RepeatEach"]
)]
#[case::copy_of_exiled(
    "A:SP$ Draw | SubAbility$ P\nSVar:P:DB$ Play | Valid$ Card.YouOwn | ValidZone$ Exile | WithoutManaCost$ True | ValidSA$ Spell | Optional$ True | CopyCard$ True",
    &["CopyCard", "CastFromZone"],
    &[]
)]
fn play_words(#[case] lines: &str, #[case] has: &[&str], #[case] lacks: &[&str]) {
    let (json, gaps) = sorcery(lines);
    assert!(
        gaps.iter().all(|gap| !gap.contains("Play")),
        "gaps: {gaps:?}"
    );
    for word in has {
        assert!(json.contains(word), "{word} missing in {json} ({gaps:?})");
    }
    for word in lacks {
        assert!(!json.contains(word), "{word} present in {json}");
    }
}

#[rstest]
#[case::mandatory(
    "DB$ Play | Defined$ Remembered | WithoutManaCost$ True",
    "key:Play.Optional"
)]
#[case::total_cmc(
    "DB$ Play | Valid$ Card.YouOwn | ValidZone$ Exile | WithTotalCMC$ 10 | Optional$ True | Amount$ All",
    "key:Play.WithTotalCMC"
)]
#[case::copy_outside_exile(
    "DB$ Play | Defined$ Remembered | CopyCard$ True | Optional$ True",
    "key:Play.CopyCard"
)]
#[case::spell_filter(
    "DB$ Play | Defined$ Remembered | ValidSA$ Spell.cmcLEX | Optional$ True",
    "key:Play.ValidSA"
)]
#[case::amount_variable(
    "DB$ Play | Defined$ Remembered | Amount$ X | Optional$ True",
    "key:Play.Amount"
)]
fn play_unknowns_stay_gaps(#[case] line: &str, #[case] gap: &str) {
    let (_, found) = sorcery(&format!("A:SP$ Draw | SubAbility$ P\nSVar:P:{line}"));
    assert!(found.iter().any(|g| g == gap), "{found:?}");
}

#[rstest]
#[case::library_and_graveyard(
    "A:SP$ ChangeZone | Origin$ Library,Graveyard | Destination$ Hand | ChangeType$ Creature.YouOwn | Optional$ True | SpellDescription$ You may search your library and/or graveyard for a creature card.",
    &["\"also\":[\"Graveyard\"]", "Searched"]
)]
#[case::library_hand_graveyard(
    "A:SP$ ChangeZone | Origin$ Graveyard,Hand,Library | Destination$ Battlefield | ChangeType$ Aura.YouOwn | Hidden$ True",
    &["\"also\":[\"Graveyard\",\"Hand\"]"]
)]
fn zone_lists_with_the_library_search(#[case] line: &str, #[case] has: &[&str]) {
    let (json, gaps) = sorcery(line);
    assert!(gaps.is_empty(), "{gaps:?}");
    for word in has {
        assert!(json.contains(word), "{word} missing in {json}");
    }
}

#[rstest]
#[case::exile_one_rest_to_graveyard(
    "A:SP$ Dig | DigNum$ 3 | ChangeNum$ 1 | DestinationZone$ Exile | DestinationZone2$ Graveyard",
    &["{\"Exile\":", "\"to\":\"Graveyard\""]
)]
#[case::rest_to_hand(
    "A:SP$ Dig | DigNum$ 4 | ChangeNum$ 1 | DestinationZone$ Graveyard | DestinationZone2$ Hand",
    &["\"to\":\"Hand\""]
)]
#[case::rest_unordered(
    "A:SP$ Dig | DigNum$ 3 | ChangeNum$ 1 | LibraryPosition2$ 0 | SkipReorder$ True",
    &["\"place\":\"Top\""]
)]
#[case::all_matching_remembered(
    "A:SP$ Dig | DigNum$ 5 | ChangeNum$ All | ChangeValid$ Land | DestinationZone$ Battlefield | RememberChanged$ True | SubAbility$ P\nSVar:P:DB$ Pump | Defined$ Remembered | NumAtt$ 1",
    &["\"as\":\"moved"]
)]
fn dig_words(#[case] line: &str, #[case] has: &[&str]) {
    let (json, gaps) = sorcery(line);
    assert!(gaps.is_empty(), "{gaps:?} {json}");
    for word in has {
        assert!(json.contains(word), "{word} missing in {json}");
    }
}

#[rstest]
#[case::face_down(
    "A:SP$ Dig | DigNum$ 2 | ChangeNum$ 1 | DestinationZone$ Exile | ExileFaceDown$ True",
    "key:Dig.ExileFaceDown"
)]
#[case::random_and_skip(
    "A:SP$ Dig | DigNum$ 3 | ChangeNum$ 1 | RestRandomOrder$ True | SkipReorder$ True",
    "key:Dig.SkipReorder-with-RestRandomOrder"
)]
#[case::unknown_rest_zone(
    "A:SP$ Dig | DigNum$ 3 | ChangeNum$ 1 | DestinationZone2$ PlanarDeck",
    "key:Dig.DestinationZone2=PlanarDeck"
)]
fn dig_unknowns_stay_gaps(#[case] line: &str, #[case] gap: &str) {
    let (_, found) = sorcery(line);
    assert_eq!(found, [gap]);
}

#[rstest]
#[case::hidden_battlefield_choice(
    "A:SP$ ChangeZone | Origin$ Battlefield | Destination$ Hand | Hidden$ True | ChangeType$ Land.YouCtrl | ChangeNum$ 1",
    &["\"Choose\"", "\"ControlledBy\":\"You\"", "\"to\":\"Hand\""]
)]
#[case::transformed_return(
    "A:SP$ ChangeZone | Origin$ Exile | Destination$ Battlefield | Defined$ Self | Transformed$ True",
    &["\"transformed\":true"]
)]
#[case::attached_to_host(
    "A:SP$ ChangeZone | Origin$ Graveyard | Destination$ Battlefield | Defined$ Self | AttachedTo$ Self",
    &["BattlefieldAttached"]
)]
fn change_zone_words(#[case] line: &str, #[case] has: &[&str]) {
    let (json, gaps) = sorcery(line);
    assert!(gaps.is_empty(), "{gaps:?} {json}");
    for word in has {
        assert!(json.contains(word), "{word} missing in {json}");
    }
}

#[rstest]
#[case::battlefield_without_owner(
    "A:SP$ ChangeZone | Origin$ Battlefield | Destination$ Hand | Hidden$ True | ChangeType$ Land",
    "key:ChangeZone.Choose.Battlefield-owner"
)]
#[case::transformed_to_hand(
    "A:SP$ ChangeZone | Origin$ Battlefield | Destination$ Hand | Defined$ Self | Transformed$ True",
    "key:ChangeZone.Transformed"
)]
#[case::attached_and_tapped(
    "A:SP$ ChangeZone | Origin$ Graveyard | Destination$ Battlefield | Defined$ Self | AttachedTo$ Self | Tapped$ True",
    "key:ChangeZone.AttachedTo-with-Tapped"
)]
fn change_zone_unknowns_stay_gaps(#[case] line: &str, #[case] gap: &str) {
    let (_, found) = sorcery(line);
    assert_eq!(found, [gap]);
}

#[rstest]
#[case::each_player_from_own_hand(
    "A:SP$ ChangeZone | Origin$ Hand | Destination$ Library | ChangeType$ Card | DefinedPlayer$ Player | Hidden$ True | Mandatory$ True",
    &["RepeatEach", "EachPlayer", "\"who\":{\"Bound\":\"player"]
)]
#[case::each_opponent_chosen_by_you(
    "A:SP$ ChangeZone | Origin$ Graveyard | Destination$ Exile | ChangeType$ Card | DefinedPlayer$ Player.Opponent | Chooser$ You | Hidden$ True",
    &["RepeatEach", "Opponents"]
)]
#[case::chooser_you_without_player(
    "A:SP$ ChangeZone | Origin$ Hand | Destination$ Battlefield | ChangeType$ Creature | Chooser$ You | Hidden$ True",
    &["\"Choose\"", "\"OwnedBy\":\"You\""]
)]
fn who_picks_in_a_hidden_move(#[case] line: &str, #[case] has: &[&str]) {
    let (json, gaps) = sorcery(line);
    assert!(gaps.is_empty(), "{gaps:?} {json}");
    for word in has {
        assert!(json.contains(word), "{word} missing in {json}");
    }
}

#[rstest]
#[case::chooser_opponent(
    "A:SP$ ChangeZone | Origin$ Hand | Destination$ Exile | ChangeType$ Card | Chooser$ Opponent | Hidden$ True",
    "key:ChangeZone.Choose.Chooser"
)]
#[case::each_player_remembered(
    "A:SP$ ChangeZone | Origin$ Hand | Destination$ Exile | ChangeType$ Card | DefinedPlayer$ Player | RememberChanged$ True | Hidden$ True",
    "key:ChangeZone.Choose.RememberChanged-for-each-player"
)]
#[case::targeted_controller_with_target(
    "A:SP$ ChangeZone | Origin$ Hand | Destination$ Exile | ChangeType$ Card | ValidTgts$ Creature | DefinedPlayer$ TargetedController | Hidden$ True",
    "key:ChangeZone.Choose.DefinedPlayer"
)]
fn who_picks_unknowns_stay_gaps(#[case] line: &str, #[case] gap: &str) {
    let (_, found) = sorcery(line);
    assert_eq!(found, [gap]);
}

#[rstest]
#[case::stack("Stack", "zone:Origin=Stack")]
#[case::stack_with_battlefield("Battlefield,Stack", "zone:Origin=Battlefield,Stack")]
#[case::command("Command", "zone:Origin=Command")]
#[case::repeated("Graveyard,Graveyard", "zone:Origin=Graveyard,Graveyard")]
fn zones_without_a_node_stay_gaps(#[case] origin: &str, #[case] gap: &str) {
    let (_, found) = sorcery(&format!(
        "A:SP$ ChangeZone | Origin$ {origin} | Destination$ Exile | Defined$ Targeted"
    ));
    assert_eq!(found, [gap]);
}

const MOVE_BACK: &str = "A:SP$ ChangeZone | Origin$ Graveyard | Destination$ Battlefield | ValidTgts$ Artifact.YouOwn | StaticEffect$ Animate | WithCountersType$ P1P1 | WithCountersAmount$ 2";

#[rstest]
#[case::spirit_with_flying(
    "SVar:Animate:Mode$ Continuous | Affected$ Card.IsRemembered | AddType$ Creature & Spirit | SetPower$ 1 | SetToughness$ 1 | AddKeyword$ Flying",
    &["\"as\":\"moved\"", "\"Continuous\":{\"affected\":{\"Bound\":\"moved\"}", "{\"AddType\":\"Creature\"}", "{\"AddSubtype\":\"Spirit\"}", "{\"SetPT\":{\"power\":1,\"toughness\":1}}", "\"AddKeyword\"", "\"duration\":\"Forever\""]
)]
#[case::one_type_without_affected(
    "SVar:Animate:Mode$ Continuous | AddType$ Artifact",
    &["{\"AddType\":\"Artifact\"}", "\"duration\":\"Forever\""]
)]
#[case::color_and_type(
    "SVar:Animate:Mode$ Continuous | AddType$ Nightmare | SetColor$ Black",
    &["{\"AddSubtype\":\"Nightmare\"}", "{\"SetColors\":[\"Black\"]}"]
)]
fn a_static_effect_is_a_forever_continuous_over_the_moved_object(
    #[case] svar: &str,
    #[case] has: &[&str],
) {
    let (json, found) = sorcery(&format!("{MOVE_BACK}\n{svar}"));
    assert!(found.is_empty(), "{found:?} {json}");
    for word in has {
        assert!(json.contains(word), "{word} missing in {json}");
    }
}

#[rstest]
#[case::adds_a_color("SVar:Animate:Mode$ Continuous | AddType$ Zombie | AddColor$ Black")]
#[case::takes_types_away(
    "SVar:Animate:Mode$ Continuous | AddType$ Artifact | RemoveCardTypes$ True"
)]
#[case::power_alone("SVar:Animate:Mode$ Continuous | SetPower$ 4")]
#[case::affects_creatures_only(
    "SVar:Animate:Mode$ Continuous | Affected$ Creature.IsRemembered | AddType$ Zombie"
)]
#[case::another_mode("SVar:Animate:Mode$ CantAttack | ValidCard$ Card.IsRemembered")]
#[case::missing_svar("SVar:Other:Mode$ Continuous | AddType$ Hero")]
fn a_static_effect_that_means_more_stays_a_gap(#[case] svar: &str) {
    let (_, found) = sorcery(&format!("{MOVE_BACK}\n{svar}"));
    assert!(!found.is_empty());
}

#[test]
fn manifest_dread_looks_at_two_keeps_one_and_buries_the_rest() {
    let (json, found) = sorcery(
        "A:SP$ ManifestDread | RememberManifested$ True | SubAbility$ P\nSVar:P:DB$ PutCounter | Defined$ Remembered | CounterType$ P1P1 | CounterNum$ 1",
    );
    assert!(found.is_empty(), "{found:?} {json}");
    let order = [
        "\"LookAtTop\":{\"as\":\"looked\",\"count\":2",
        "\"Choose\":{\"as\":\"kept2\",\"from\":{\"Bound\":\"looked\"}",
        "\"rest_as\":\"rest3\"",
        "\"Move\":{\"to\":\"Graveyard\",\"what\":{\"Bound\":\"rest3\"}}",
        "\"Manifest\":{\"as\":\"manifested4\"}",
    ];
    let mut at = 0;
    for word in order {
        let Some(found) = json[at..].find(word) else {
            panic!("{word} missing or out of order in {json}");
        };
        at += found;
    }
}

#[rstest]
#[case::many_times("A:SP$ ManifestDread | Amount$ X\nSVar:X:Count$xPaid")]
#[case::unknown_key("A:SP$ ManifestDread | Mandatory$ False")]
fn manifest_dread_beyond_once_stays_a_gap(#[case] line: &str) {
    let (_, found) = sorcery(line);
    assert!(!found.is_empty());
}

#[test]
fn a_static_effect_on_a_move_elsewhere_stays_a_gap() {
    let (_, found) = sorcery(
        "A:SP$ ChangeZone | Origin$ Graveyard | Destination$ Hand | ValidTgts$ Artifact.YouOwn | StaticEffect$ Animate\nSVar:Animate:Mode$ Continuous | AddType$ Hero",
    );
    assert!(!found.is_empty());
}
