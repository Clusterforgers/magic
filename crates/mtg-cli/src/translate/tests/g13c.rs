//! Goal: round 13 trigger forms read what the card prints: the attacker of an attack or block trigger deals the damage
//! its "it deals" names, a discard names its player, counters put together and a clash won are their events, a search
//! of one's own library and a land played from a zone or a creature exiled from the battlefield test where they came
//! from, and two lines of one printed trigger settle a one-sided decider by the printed words or stay apart when only
//! their condition differs; any other shape keeps its gap.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a creature with the given lines as compact JSON, and its gaps.
fn card(lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:Creature Ooze\nPT:1/1\n{lines}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

const DAMAGE: &str = "SVar:Trig:DB$ DealDamage | ValidTgts$ Any | NumDmg$ 2";
const DRAW: &str = "SVar:Trig:DB$ Draw | NumCards$ 1";

#[rstest]
#[case::attacker_deals(
    &format!("T:Mode$ Attacks | ValidCard$ Creature.YouCtrl | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ Whenever a creature you control attacks, it deals 2 damage to any target.\n{DAMAGE}"),
    r#""source":"Triggering""#
)]
#[case::blocked_creature_deals(
    "T:Mode$ AttackerBlocked | ValidCard$ Creature.EquippedBy | Execute$ Trig | TriggerDescription$ Whenever equipped creature becomes blocked, it deals 1 damage to defending player.\nSVar:Trig:DB$ DealDamage | Defined$ TriggeredDefendingPlayer | NumDmg$ 1",
    r#""source":"Triggering""#
)]
#[case::nickname_deals_itself(
    &format!("T:Mode$ DamageDone | ValidSource$ Card.Self | ValidTarget$ Opponent | CombatDamage$ True | Execute$ Trig | TriggerDescription$ Whenever NICKNAME deals combat damage to an opponent, it deals 2 damage to any target.\n{DAMAGE}"),
    r#""DealDamage":{"amount":2,"to":{"Target":0}}"#
)]
#[case::opponent_discards(
    &format!("T:Mode$ Discarded | ValidPlayer$ Opponent | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ Whenever an opponent discards a card, draw a card.\n{DRAW}"),
    r#""Discarded":{"by":"Opponents"}"#
)]
#[case::you_discard_a_nonland(
    &format!("T:Mode$ Discarded | ValidPlayer$ You | ValidCard$ Card.nonLand | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ x\n{DRAW}"),
    r#""Discarded":{"by":"You","what":[{"Not":{"Type":"Land"}}]}"#
)]
#[case::own_discard(
    &format!("T:Mode$ Discarded | ValidCard$ Card.Self | TriggerController$ TriggeredCardController | Execute$ Trig | TriggerDescription$ When you discard this card, draw a card.\n{DRAW}"),
    r#""Discarded":{"what":["This"]}"#
)]
#[case::counters_on_several(
    &format!("T:Mode$ CounterAddedAll | CounterType$ P1P1 | Valid$ Creature.YouCtrl | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ x\n{DRAW}"),
    r#""Batch":{"CountersPut":{"counter":"+1/+1","what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#
)]
#[case::clash_won(
    &format!("T:Mode$ Clashed | ValidPlayer$ You | Won$ True | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ Whenever you clash and win, draw a card.\n{DRAW}"),
    r#""W15cClashWon":{"who":"You"}"#
)]
#[case::own_library(
    &format!("T:Mode$ SearchedLibrary | ValidPlayer$ Player.Opponent | SearchOwnLibrary$ True | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ x\n{DRAW}"),
    r#""Did":{"action":"SearchedOwnLibrary","who":"Opponents"}"#
)]
#[case::any_library(
    &format!("T:Mode$ SearchedLibrary | ValidPlayer$ Player.Opponent | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ x\n{DRAW}"),
    r#""Did":{"action":"SearchedLibrary","who":"Opponents"}"#
)]
#[case::land_from_exile(
    &format!("T:Mode$ LandPlayed | Origin$ Exile | ValidCard$ Land.YouCtrl | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ x\n{DRAW}"),
    r#"{"ControlledBy":"You"},{"ArrivedFrom":"Exile"}]"#
)]
#[case::land_not_from_hand(
    &format!("T:Mode$ LandPlayed | Origin$ Ante,Exile,Graveyard | ValidCard$ Land.YouCtrl | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ x\n{DRAW}"),
    r#"{"AnyOf":[{"ArrivedFrom":"Exile"},{"ArrivedFrom":"Graveyard"}]}"#
)]
#[case::exiled_from_battlefield(
    &format!("T:Mode$ Exiled | Origin$ Battlefield | ValidCard$ Creature | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ x\n{DRAW}"),
    r#""Did":{"action":"Exiled","what":[{"Type":"Creature"},{"In":"Battlefield"}]}"#
)]
fn g13c_a_trigger_form_reads_its_event(#[case] lines: &str, #[case] expected: &str) {
    let (json, found) = card(lines);
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[rstest]
#[case::dealt_damage_batch(
    &format!("T:Mode$ DamageDoneOnce | ValidTarget$ Creature.YouCtrl | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ Whenever a creature you control is dealt damage, it deals 2 damage to any target.\n{DAMAGE}"),
    "trigger:printed-it-deals-from-host"
)]
#[case::player_and_owner(
    &format!("T:Mode$ Discarded | ValidPlayer$ Opponent | ValidCard$ Card.YouOwn | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ x\n{DRAW}"),
    "key:Trigger.Discarded.ValidPlayer-and-card-owner"
)]
#[case::other_discard_controller(
    &format!("T:Mode$ Discarded | ValidCard$ Card.YouOwn | TriggerController$ TriggeredCardController | Execute$ Trig | TriggerDescription$ x\n{DRAW}"),
    "key:Trigger.Discarded.TriggerController"
)]
#[case::clash_lost(
    &format!("T:Mode$ Clashed | ValidPlayer$ You | Won$ False | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ x\n{DRAW}"),
    "trigger:Clashed"
)]
#[case::land_from_the_battlefield(
    &format!("T:Mode$ LandPlayed | Origin$ Battlefield | ValidCard$ Land.YouCtrl | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ x\n{DRAW}"),
    "key:Trigger.LandPlayed.Origin=Battlefield"
)]
#[case::exiled_from_a_graveyard(
    &format!("T:Mode$ Exiled | Origin$ Graveyard | ValidCard$ Creature | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ x\n{DRAW}"),
    "key:Trigger.Exiled.Origin"
)]
fn g13c_another_shape_keeps_its_gap(#[case] lines: &str, #[case] gap: &str) {
    let (_, found) = card(lines);
    assert!(found.iter().any(|g| g == gap), "{found:?}");
}

const GAIN: &str = "SVar:Trig:DB$ GainLife | LifeAmount$ 3";

/// Enters and attacks lines of one printed trigger, the decider written on the attack line only.
fn enters_or_attacks(printed: &str) -> String {
    format!(
        "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ Trig | TriggerDescription$ {printed}\nT:Mode$ Attacks | ValidCard$ Card.Self | Execute$ Trig | OptionalDecider$ You | Secondary$ True | TriggerDescription$ {printed}\n{GAIN}"
    )
}

#[rstest]
#[case::printed_forced("When CARDNAME enters or attacks, you gain 3 life.", false)]
#[case::printed_may("When CARDNAME enters or attacks, you may gain 3 life.", true)]
fn g13c_a_one_sided_decider_follows_the_printed_words(#[case] printed: &str, #[case] asks: bool) {
    let (json, found) = card(&enters_or_attacks(printed));
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(r#""AnyOf""#), "{json}");
    assert_eq!(json.contains(r#""May""#), asks, "{json}");
}

#[rstest]
#[case::different_events(
    "T:Mode$ SpellCast | ValidActivatingPlayer$ Opponent | PlayerTurn$ True | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ x\nT:Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Card.Self | Execute$ Trig | Secondary$ True | TriggerDescription$ x",
    Some(2)
)]
// Two zone changes can be one event (the card itself is another creature too): the lines keep their gap.
#[case::same_event_kind(
    "T:Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Card.Self | PlayerTurn$ True | Execute$ Trig | TriggerDescription$ x\nT:Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Creature.YouCtrl | TriggerZones$ Battlefield | Execute$ Trig | Secondary$ True | TriggerDescription$ x",
    None
)]
fn g13c_lines_of_different_events_with_their_own_conditions_stay_apart(
    #[case] lines: &str,
    #[case] triggers: Option<usize>,
) {
    let (json, found) = card(&format!("{lines}\n{DRAW}"));
    match triggers {
        Some(count) => {
            assert!(found.is_empty(), "{found:?}");
            assert_eq!(json.matches(r#""Triggered""#).count(), count, "{json}");
        }
        None => assert!(
            found
                .iter()
                .any(|g| g == "trigger:Secondary-with-different-effects"),
            "{found:?}"
        ),
    }
}
