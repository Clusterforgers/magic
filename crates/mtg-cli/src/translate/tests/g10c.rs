//! Goal: round 10 pack g10c reads the damage a resolution dealt ("the damage dealt this way"), the damage players took
//! this turn, crimes committed this turn and +1/+1 counters put this turn, and leaves other words a gap by name.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of an instant with the given script lines and Oracle text, as compact JSON, and its gaps.
fn card(lines: &str, oracle: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:Instant\n{lines}\nOracle:{oracle}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

const DRAIN: &str = "A:SP$ DealDamage | ValidTgts$ Any | NumDmg$ 3 | SubAbility$ G\nSVar:G:DB$ GainLife | LifeAmount$ Y\nSVar:Y:";
const CRIME: &str = r#"{"When":{"condition":{"Compare":{"left":{"Tallied":{"what":"Crimes","who":"You"}},"op":"Ge","right":1}},"otherwise":0,"then":1}}"#;
const HURT_MOST: &str = r#"{"Tallied":{"what":"DamageTaken","who":"Opponents"}}"#;

#[rstest]
#[case::dealt_this_way(
    "Count$TotalDamageDoneByThisTurn",
    "Subject deals 3 damage to any target. You gain life equal to the damage dealt this way.",
    r#""DamageDealtThisWay""#
)]
#[case::most_one_opponent_took(
    "Count$MaxOppDamageThisTurn",
    "Subject deals 3 damage to any target. You gain life equal to the most damage an opponent took this turn.",
    HURT_MOST
)]
#[case::crime_one_or_none(
    "Count$CommittedCrimeThisTurn.1.0",
    "Subject deals 3 damage to any target. You gain 1 life if you've committed a crime this turn.",
    CRIME
)]
#[case::plus_one_counters_you_put(
    "Count$CountersAddedThisTurn P1P1 You Creature.YouCtrl",
    "Subject deals 3 damage to any target. You gain 1 life for each +1/+1 counter you've put on creatures you control this turn.",
    r#"{"Tally":{"event":"PlusOneCountersPut","filter":[{"Type":"Creature"},{"ControlledBy":"You"}],"who":"You"}}"#
)]
#[case::damage_dealt_to_this(
    "Count$TotalDamageReceivedThisTurn",
    "Subject deals 3 damage to any target. You gain life equal to the damage dealt to Subject this turn.",
    r#"{"Tallied":{"what":"DamageDealtTo","who":"This"}}"#
)]
#[case::creature_cards_to_your_graveyard(
    "Count$ThisTurnEntered_Graveyard_Creature.YouOwn+!token",
    "Subject deals 3 damage to any target. You gain 1 life for each creature card put into your graveyard from anywhere this turn.",
    r#"{"Tallied":{"what":"CreatureCardsGraved","who":"You"}}"#
)]
#[case::cards_to_the_enchanted_players_graveyard(
    "Count$ThisTurnEntered_Graveyard_Card.EnchantedPlayerOwn+!token",
    "Subject deals 3 damage to any target. You gain 1 life for each card put into enchanted player's graveyard from anywhere this turn.",
    r#"{"Tallied":{"what":"CardsGraved","who":"AttachedPlayer"}}"#
)]
fn turn_facts_read_as_the_amount(#[case] count: &str, #[case] oracle: &str, #[case] amount: &str) {
    let (json, found) = card(&format!("{DRAIN}{count}"), oracle);
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(&format!(r#""amount":{amount}"#)), "{json}");
}

#[rstest]
// Forge's count is what the card dealt all turn: without the Oracle words it is not this resolution's damage.
#[case::dealt_by_the_card_all_turn(
    "Count$TotalDamageDoneByThisTurn",
    "Subject deals 3 damage to any target. You gain life equal to the damage Subject dealt this turn.",
    "amount:Count$TotalDamageDoneByThisTurn"
)]
#[case::another_kind_of_counter(
    "Count$CountersAddedThisTurn OIL You Card.YouCtrl",
    "Subject deals 3 damage to any target. You gain 1 life for each oil counter you've put this turn.",
    "amount:Count$CountersAddedThisTurn-of-a-kind"
)]
fn other_turn_facts_stay_gaps(#[case] count: &str, #[case] oracle: &str, #[case] gap: &str) {
    let (_, found) = card(&format!("{DRAIN}{count}"), oracle);
    assert!(found.iter().any(|g| g == gap), "{found:?}");
}

#[test]
fn the_damage_a_target_player_took_this_turn_is_their_tally() {
    let (json, found) = card(
        "A:SP$ LoseLife | ValidTgts$ Player | LifeAmount$ X | Defined$ Targeted\nSVar:X:TargetedPlayer$DamageThisTurn",
        "Target player loses life equal to the damage already dealt to that player this turn.",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(
        json.contains(r#"{"Tallied":{"what":"DamageTaken","who":{"Target":0}}}"#),
        "{json}"
    );
}
