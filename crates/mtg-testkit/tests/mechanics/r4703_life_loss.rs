//! Goal: a loss of life made a multiple (`ReplaceBy::LossTimes`, Bloodletter of Aclazotz: "If an opponent would lose
//! life during your turn, they lose twice that much life instead") doubles a loss by an effect and the loss damage
//! causes (the damage stays as dealt), only for the controller's opponents and only during the controller's turn; two
//! of them quadruple it, and a life-loss trigger reads the life lost once, at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A free sorcery with the effects `effects` (JSON).
fn sorcery(effects: &str) -> Arc<CardDefinition> {
    let spell: Ability =
        serde_json::from_str(&format!(r#"{{"Spell":{{"effects":{effects}}}}}"#)).unwrap();
    Arc::new(
        CardDefinition::new("Bleed")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(spell),
    )
}

/// A creature: "Whenever an opponent loses life, you gain that much life."
fn leech() -> Arc<CardDefinition> {
    let trigger: Ability = serde_json::from_str(
        r#"{"Triggered":{"when":{"LifeLost":{"who":"Opponents"}},"effects":[{"GainLife":{"amount":"Triggering"}}]}}"#,
    )
    .unwrap();
    let base = free_creature("Leech", 1, 1);
    Arc::new((*base).clone().with_rules(trigger))
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// Seat 0 (the active player) casts the sorcery with `bloodletters` Bloodletters under seat `owner`'s control, and the
/// stack resolves.
fn bleed(
    seats: usize,
    owner: usize,
    bloodletters: usize,
    effects: &str,
) -> Result<Driver, ScriptError> {
    let mut permanents = vec![ready(leech())];
    let mut theirs = Vec::new();
    for _ in 0..bloodletters {
        let letter = ready(card("Bloodletter of Aclazotz").unwrap());
        if owner == 0 {
            permanents.push(letter);
        } else {
            theirs.push(letter);
        }
    }
    let mut scenario = Scenario::new(seats)
        .battlefield(0, permanents)
        .hand(0, vec![sorcery(effects)]);
    if owner != 0 {
        scenario = scenario.battlefield(owner, theirs);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Bleed")?;
    // The sorcery resolves, then each loss's trigger goes on the stack and resolves.
    for _ in 0..40 {
        if driver.state().zone(Zone::Stack).is_empty()
            && driver.state().pending_triggers().is_empty()
        {
            break;
        }
        driver.answer_passively()?;
    }
    Ok(driver)
}

const OPPONENTS_LOSE: &str = r#"[{"LoseLife":{"who":"Opponents","amount":3}}]"#;
const DAMAGE: &str = r#"[{"DealDamage":{"to":"Opponents","amount":3}}]"#;
const YOU_LOSE: &str = r#"[{"LoseLife":{"amount":3}}]"#;

#[rstest]
#[case::a_loss_is_doubled(OPPONENTS_LOSE, 1, 14)]
#[case::damage_takes_twice_the_life(DAMAGE, 1, 14)]
#[case::two_quadruple_it(OPPONENTS_LOSE, 2, 8)]
#[case::without_one_it_is_as_it_was(OPPONENTS_LOSE, 0, 17)]
fn opponents_lose_more_on_your_turn(
    #[values(2, 3, 4)] seats: usize,
    #[case] effects: &str,
    #[case] bloodletters: usize,
    #[case] left: i32,
) -> Result<(), ScriptError> {
    let driver = bleed(seats, 0, bloodletters, effects)?;
    for seat in 1..seats {
        assert_eq!(life(&driver, seat), left, "seat {seat}");
    }
    // The leech's trigger reads each opponent's loss once, as much as was lost.
    let gained = (20 - left) * i32::try_from(seats - 1).unwrap();
    assert_eq!(life(&driver, 0), 20 + gained);
    Ok(())
}

/// Seat 0's own loss is no opponent's, and on seat 0's turn seat 1's Bloodletter does nothing: it is not its turn.
#[rstest]
#[case::your_own_loss(YOU_LOSE, 0, 0, 17)]
#[case::not_its_controllers_turn(YOU_LOSE, 1, 0, 17)]
#[case::its_controllers_opponent_on_another_turn(OPPONENTS_LOSE, 1, 1, 17)]
fn other_losses_are_as_they_were(
    #[values(2, 3, 4)] seats: usize,
    #[case] effects: &str,
    #[case] owner: usize,
    #[case] seat: usize,
    #[case] left: i32,
) -> Result<(), ScriptError> {
    let driver = bleed(seats, owner, 1, effects)?;
    assert_eq!(life(&driver, seat), left);
    Ok(())
}
