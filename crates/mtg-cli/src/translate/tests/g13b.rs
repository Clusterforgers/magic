//! Goal: round 13 pack g13b reads a number about a target or event player (its creatures, its cards a valid-string
//! selects, what it drew or lost this turn), "as you cast" counts where the spell reads them while cast, the creatures
//! that left the battlefield under you, a target judged by a valid-string, the targets of a cast spell, copies of an
//! entering creature and a spell target's stats, and leaves the shapes that would mean something else a gap.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of an instant with the given script lines, as compact JSON, and its gaps.
fn card(lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:Instant\n{lines}\nOracle:Subject.\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

/// A creature with the given script lines, as compact JSON, and its gaps.
fn creature(lines: &str) -> (String, Vec<String>) {
    let text =
        format!("Name:Subject\nTypes:Creature Shapeshifter\nPT:1/1\n{lines}\nOracle:Subject.\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

const TARGET_LOSES: &str = "A:SP$ LoseLife | ValidTgts$ Player | LifeAmount$ X\nSVar:X:";

#[rstest]
#[case::creatures_they_control(
    "TargetedPlayer$CreaturesInPlay",
    r#"{"Count":[{"Type":"Creature"},{"ControlledBy":{"Target":0}}]}"#
)]
#[case::their_tapped_artifacts(
    "TargetedPlayer$Valid Artifact.tapped+YouCtrl",
    r#"{"Largest":{"of":[{"Type":"Artifact"},"Tapped",{"ControlledBy":"You"}],"who":{"Target":0}}}"#
)]
#[case::creature_cards_in_their_graveyard(
    "TargetedPlayer$ValidGraveyard Creature.YouOwn",
    r#"{"Largest":{"of":[{"Type":"Creature"},{"OwnedBy":"You"},{"In":"Graveyard"}],"who":{"Target":0}}}"#
)]
#[case::cards_they_drew(
    "TargetedPlayer$CardsDrawn",
    r#"{"Tallied":{"what":"Drawn","who":{"Target":0}}}"#
)]
#[case::life_they_lost(
    "TargetedPlayer$LifeLostThisTurn",
    r#"{"Tallied":{"what":"LifeLost","who":{"Target":0}}}"#
)]
#[case::cards_they_discarded(
    "TargetedPlayer$CardsDiscardedThisTurn",
    r#"{"Tallied":{"what":"Discarded","who":{"Target":0}}}"#
)]
#[case::their_poison(
    "TargetedPlayer$Counters.Poison",
    r#"{"Counters":{"counter":"poison","on":{"Target":0}}}"#
)]
#[case::their_domain(
    "TargetedPlayer$DomainPlayer",
    r#"{"Distinct":{"of":[{"Type":"Land"},{"ControlledBy":{"Target":0}}],"stat":"BasicLandTypes"}}"#
)]
#[case::the_line_s_own_player(
    "ThisTargetedPlayer$Valid Creature.YouCtrl",
    r#"{"Largest":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"who":{"Target":0}}}"#
)]
fn a_number_about_the_target_player(#[case] count: &str, #[case] amount: &str) {
    let (json, found) = card(&format!("{TARGET_LOSES}{count}"));
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(&format!(r#""amount":{amount}"#)), "{json}");
}

#[rstest]
// A selector about the opponents would be read for each player taken as "you": the opponents of the target.
#[case::the_opponents_of_the_target(
    "TargetedPlayer$Valid Creature.OppCtrl",
    "amount:TargetedPlayer$Valid-of-the-opponents"
)]
#[case::an_unknown_word("TargetedPlayer$Handsize", "amount:TargetedPlayer$Handsize")]
fn other_numbers_about_the_target_player_stay_gaps(#[case] count: &str, #[case] gap: &str) {
    let (_, found) = card(&format!("{TARGET_LOSES}{count}"));
    assert!(found.iter().any(|g| g == gap), "{found:?}");
}

#[test]
fn an_as_cast_count_is_read_live_for_the_bounds_and_the_division() {
    let (json, found) = card(
        "A:SP$ DealDamage | ValidTgts$ Creature | NumDmg$ N | TargetMin$ 0 | TargetMax$ N | DividedAsYouChoose$ N\nSVar:N:Count$LastStateBattlefieldWithFallback Creature",
    );
    assert!(found.is_empty(), "{found:?}");
    let live = r#"{"Count":[{"Type":"Creature"}]}"#;
    assert!(json.contains(&format!(r#""divide":{live}"#)), "{json}");
    assert!(json.contains(&format!(r#""max":{live}"#)), "{json}");
}

#[rstest]
// Read at resolution the count would be the battlefield then, not as the spell was cast.
#[case::an_amount_at_resolution(
    "A:SP$ Pump | ValidTgts$ Creature | NumAtt$ -X | NumDef$ -X\nSVar:X:Count$LastStateBattlefieldWithFallback Creature.YouCtrl",
    "amount:Count$LastStateBattlefieldWithFallback"
)]
#[case::a_graveyard_amount_at_resolution(
    "A:SP$ Draw | NumCards$ X\nSVar:X:Count$LastStateGraveyardWithFallback Creature.YouOwn",
    "amount:Count$LastStateGraveyardWithFallback"
)]
// Forge reads zero for a permanent target and adds `Targeted$`: the sum would count a spell twice.
#[case::a_spell_or_a_permanent(
    "A:SP$ ChangeZone | ValidTgts$ Permanent,Card.inZoneStack | TgtZone$ Stack,Battlefield | Origin$ Battlefield,Stack | Destination$ Hand | SubAbility$ D\nSVar:D:DB$ Draw | NumCards$ X\nSVar:X:SpellTargeted$CardManaCostLKI",
    "amount:SpellTargeted"
)]
fn readings_that_would_mean_something_else_stay_gaps(#[case] lines: &str, #[case] gap: &str) {
    let (_, found) = card(lines);
    assert!(found.iter().any(|g| g == gap), "{found:?}");
}

#[rstest]
#[case::creatures_that_left_under_you(
    "A:SP$ Draw | NumCards$ X\nSVar:X:Count$LeftBattlefieldThisTurn Creature.YouCtrl",
    r#"{"Tallied":{"what":"CreaturesLeftUnder","who":"You"}}"#
)]
#[case::the_countered_spell_s_mana_value(
    "A:SP$ Counter | TargetType$ Spell | ValidTgts$ Card | SubAbility$ D\nSVar:D:DB$ Draw | NumCards$ X\nSVar:X:SpellTargeted$CardManaCostLKI",
    r#"{"Stat":{"of":{"Target":0},"stat":"ManaValue"}}"#
)]
#[case::power_plus_toughness(
    "A:SP$ Destroy | ValidTgts$ Creature | SubAbility$ D\nSVar:D:DB$ Draw | NumCards$ X\nSVar:X:Targeted$CardSumPT",
    r#"{"Sum":[{"Stat":{"of":{"Target":0},"stat":"Power"}},{"Stat":{"of":{"Target":0},"stat":"Toughness"}}]}"#
)]
fn counts_of_the_turn_and_of_targets(#[case] lines: &str, #[case] count: &str) {
    let (json, found) = card(lines);
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(&format!(r#""count":{count}"#)), "{json}");
}

#[rstest]
#[case::not_a_creature(
    "Targeted.!Creature",
    r#"{"Each":[{"Is":{"Target":0}},{"Not":{"Type":"Creature"}}]}"#
)]
#[case::a_power(
    "Targeted.powerLE0",
    r#"{"Each":[{"Is":{"Target":0}},{"Compare":{"op":"Le","stat":"Power","to":0}}]}"#
)]
#[case::not_yours(
    "Targeted.YouDontCtrl",
    r#"{"Each":[{"Is":{"Target":0}},{"Not":{"ControlledBy":"You"}}]}"#
)]
fn a_target_judged_by_a_valid_string(#[case] word: &str, #[case] what: &str) {
    let (json, found) = card(&format!(
        "A:SP$ Tap | ValidTgts$ Creature | SubAbility$ D\nSVar:D:DB$ Destroy | Defined$ {word}"
    ));
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(&format!(r#""what":{what}"#)), "{json}");
}

#[test]
fn the_targets_of_a_cast_spell_that_match() {
    let (json, found) = creature(
        "T:Mode$ SpellCast | ValidActivatingPlayer$ You | TargetsValid$ Creature.inZoneBattlefield | TriggerZones$ Battlefield | Execute$ P\nSVar:P:DB$ Pump | Defined$ TriggeredSpellAbilityTargets | KW$ Flying",
    );
    assert!(found.is_empty(), "{found:?}");
    let those =
        r#"{"Each":[{"TargetedBy":"Triggering"},{"Type":"Creature"},{"In":"Battlefield"}]}"#;
    assert!(json.contains(&format!(r#""affected":{those}"#)), "{json}");
}

#[rstest]
#[case::an_entering_creature(
    "Origin$ Any | Destination$ Battlefield | ValidCard$ Creature.Other",
    None
)]
// A creature card that went to a graveyard is a new object there (CR 400.7): the copy would be of the card.
#[case::a_dying_creature(
    "Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Creature",
    Some("defined:Clone.TriggeredCardLKICopy")
)]
fn a_copy_of_the_triggering_creature(#[case] change: &str, #[case] gap: Option<&str>) {
    let (json, found) = creature(&format!(
        "T:Mode$ ChangesZone | {change} | TriggerZones$ Battlefield | Execute$ C\nSVar:C:DB$ Clone | Defined$ TriggeredCardLKICopy"
    ));
    match gap {
        None => {
            assert!(found.is_empty(), "{found:?}");
            assert!(json.contains(r#""of":"Triggering""#), "{json}");
        }
        Some(gap) => assert!(found.iter().any(|g| g == gap), "{found:?}"),
    }
}

#[rstest]
#[case::lose_all_poison(
    "A:SP$ Poison | ValidTgts$ Player | Num$ -X\nSVar:X:TargetedPlayer$Counters.Poison",
    r#"{"RemoveCounters":{"count":{"Counters":{"counter":"poison","on":{"Target":0}}},"counter":"poison","from":{"Target":0}}}"#
)]
#[case::counter_unless_its_controller_pays(
    "A:SP$ Counter | TargetType$ Spell | ValidTgts$ Card | UnlessCost$ 3 | UnlessPayer$ ThisTargetedController",
    r#""who":{"ControllerOf":{"Target":0}}"#
)]
fn other_event_words(#[case] lines: &str, #[case] part: &str) {
    let (json, found) = card(lines);
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(part), "{json}");
}
