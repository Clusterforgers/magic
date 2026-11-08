//! Goal: the d20e forms read what the card prints: the milled triggers, the secondary line of "can't lose the game and
//! opponents can't win", the first counters put on a creature, a name shared with another creature or the mana
//! spent against the source's stat; a near miss keeps its gap.
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

const GAIN: &str = "SVar:Trig:DB$ GainLife | LifeAmount$ 1";
const CANT_LOSE: &str = "Description$ You can't lose the game and your opponents can't win the game.";

#[rstest]
#[case::one_card(
    &format!("T:Mode$ Milled | ValidPlayer$ Player | ValidCard$ Card.nonLand | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ x\n{GAIN}"),
    r#""when":{"Milled":{"what":[{"Not":{"Type":"Land"}}]}}"#
)]
#[case::opponent_only(
    &format!("T:Mode$ Milled | ValidPlayer$ Opponent | ValidCard$ Card | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ x\n{GAIN}"),
    r#""by":"Opponents""#
)]
#[case::a_batch(
    &format!("T:Mode$ MilledAll | ValidPlayer$ Player | ValidCard$ Card.nonLand | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ x\n{GAIN}"),
    r#""Batch":{"Milled":{"what":[{"Not":{"Type":"Land"}}]}}"#
)]
#[case::a_batch_for_each_player(
    &format!("T:Mode$ MilledOnce | ValidPlayer$ Player | ValidCard$ Creature | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ x\n{GAIN}"),
    r#""Batch":{"Milled":{"per_player":true,"what":[{"Type":"Creature"}]}}"#
)]
#[case::first_counters_of_the_turn(
    &format!("T:Mode$ CounterTypeAddedAll | ValidObject$ Card.Self | FirstTime$ True | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ x\n{GAIN}"),
    r#""Nth":{"event":{"CountersPut":{"what":["This"]}},"nth":1}"#
)]
#[case::every_counter_batch(
    &format!("T:Mode$ CounterTypeAddedAll | ValidObject$ Card.Self | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ x\n{GAIN}"),
    r#""Batch":{"CountersPut":{"what":["This"]}}"#
)]
#[case::mana_spent_over_power(
    &format!("T:Mode$ SpellCast | ValidCard$ Card | ValidSA$ Spell.ManaSpent GTX | ValidActivatingPlayer$ You | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ x\n{GAIN}\nSVar:X:Count$CardPower"),
    r#"{"Test":{"left":{"Stat":{"of":"Candidate","stat":"ManaSpent"}},"op":"Gt","right":{"Stat":{"of":"This","stat":"Power"}}}}"#
)]
#[case::new_name_among_creatures(
    &format!("T:Mode$ ChangesZone | ValidCard$ Creature.!token+YouCtrl+doesNotShareNameWith OtherYourBattlefield | Origin$ Any | Destination$ Battlefield | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ x\n{GAIN}"),
    r#"{"Not":{"SharesName":{"Each":[{"In":"Battlefield"},{"ControlledBy":"You"},{"Not":{"Is":"Candidate"}}]}}}"#
)]
#[case::cant_lose_and_cant_win(
    &format!("R:Event$ GameLoss | ActiveZones$ Battlefield | ValidPlayer$ You | Layer$ CantHappen | {CANT_LOSE}\nR:Event$ GameWin | ActiveZones$ Battlefield | ValidPlayer$ Opponent | Layer$ CantHappen | Secondary$ True | {CANT_LOSE}"),
    r#""CantLoseGame":{"opponents_cant_win":true,"who":"You"}"#
)]
fn d20e_a_form_reads_what_the_card_prints(#[case] lines: &str, #[case] expected: &str) {
    let (json, found) = card(lines);
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[rstest]
#[case::win_for_you(
    "R:Event$ GameWin | ActiveZones$ Battlefield | ValidPlayer$ You | Layer$ CantHappen | Secondary$ True | Description$ You can't lose the game and your opponents can't win the game.",
    "replacement:GameWin"
)]
#[case::another_text(
    "R:Event$ GameWin | ActiveZones$ Battlefield | ValidPlayer$ Opponent | Layer$ CantHappen | Secondary$ True | Description$ Opponents can't win.",
    "replacement:GameWin"
)]
#[case::not_secondary(
    "R:Event$ GameWin | ActiveZones$ Battlefield | ValidPlayer$ Opponent | Layer$ CantHappen | Description$ You can't lose the game and your opponents can't win the game.",
    "replacement:GameWin"
)]
#[case::mana_spent_against_a_count_it_cannot_read(
    "T:Mode$ SpellCast | ValidCard$ Card | ValidSA$ Spell.ManaSpent GTX | ValidActivatingPlayer$ You | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ x\nSVar:Trig:DB$ GainLife | LifeAmount$ 1\nSVar:X:Count$xPaid",
    "valid:ManaSpent GTX"
)]
#[case::name_pool_of_other_permanents_elsewhere(
    "T:Mode$ ChangesZone | ValidCard$ Creature.doesNotShareNameWith OtherTheirBattlefield | Origin$ Any | Destination$ Battlefield | TriggerZones$ Battlefield | Execute$ Trig | TriggerDescription$ x\nSVar:Trig:DB$ GainLife | LifeAmount$ 1",
    "valid:doesNotShareNameWith OtherTheirBattlefield"
)]
fn d20e_a_near_miss_keeps_its_gap(#[case] lines: &str, #[case] gap: &str) {
    let (_, found) = card(lines);
    assert!(found.iter().any(|name| name == gap), "{found:?}");
}
