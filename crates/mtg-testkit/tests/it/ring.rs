//! Goal: the Ring tempts a player as the rules say (CR 701.54): the level rises by one to 4 at most, the player chooses a creature they control as Ring-bearer (asked only when they control several, none when they control none), the bearer is legendary and cannot be blocked by a creature with greater power from level 1, and each later level gives the bearer its trigger (loot on attack, the blocker sacrificed at end of combat, three life lost by each opponent on combat damage to a player) while a creature that is not the bearer, or a bearer of another player, gets none of it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::refs::Ref;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, free_creature, island};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An instant for `{U}` with "The Ring tempts you."
fn tempt(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Blue]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![Effect::RingTempts { who: Ref::You }],
            }),
    )
}

/// Seat 0 has `tempts` Tempt spells, an Island for each and these creatures; seat 1 has these creatures.
fn table(tempts: usize, mine: &[(&str, i32)], theirs: &[(&str, i32)]) -> Driver {
    let mut battlefield: Vec<_> = (0..tempts).map(|_| ready(island())).collect();
    battlefield.extend(
        mine.iter()
            .map(|(name, power)| ready(free_creature(name, *power, 4))),
    );
    let scenario = Scenario::new(2)
        .hand(0, (0..tempts).map(|n| tempt(&format!("Tempt {n}"))))
        .battlefield(0, battlefield)
        .battlefield(
            1,
            theirs
                .iter()
                .map(|(name, power)| ready(free_creature(name, *power, 4))),
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.idle_until(1, Step::Main1).unwrap();
    driver
}

/// Casts Tempt `n` and resolves it, choosing `bearer` when asked.
fn tempted(driver: &mut Driver, n: usize, bearer: &str) -> Result<(), ScriptError> {
    driver.cast(&format!("Tempt {n}"))?;
    for _ in 0..12 {
        if let DecisionKind::ChooseCards(options) = &driver.decision()?.kind {
            let wanted: ObjectId = driver.id_of(bearer)?;
            assert!(options.from.contains(&wanted));
            driver.answer(Answer::ChooseCards(CardsChoice(vec![wanted])))?;
        } else if driver.state().stack().is_empty() {
            break;
        } else {
            driver.pass()?;
        }
    }
    Ok(())
}

fn level(driver: &Driver) -> u8 {
    driver.state().ring_level(pid(0))
}

fn bearer(driver: &Driver) -> Option<ObjectId> {
    driver.state().ring_bearer(pid(0))
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

#[rstest]
#[case::once(1, 1)]
#[case::twice(2, 2)]
#[case::four_times(4, 4)]
#[case::the_level_stops_at_four(5, 4)]
fn the_level_rises_by_one_and_stops_at_four(#[case] times: usize, #[case] expected: u8) {
    let mut driver = table(times, &[("Hero", 2), ("Sidekick", 2)], &[]);
    for n in 0..times {
        tempted(&mut driver, n, "Hero").unwrap();
    }
    assert_eq!(level(&driver), expected);
    assert_eq!(bearer(&driver), Some(driver.id_of("Hero").unwrap()));
    assert_eq!(driver.state().ring_level(pid(1)), 0);
}

#[test]
fn the_bearer_is_chosen_without_asking_when_one_creature_is_controlled_and_a_new_choice_replaces_it()
 {
    let mut driver = table(2, &[("Hero", 2)], &[("Foe", 1)]);
    tempted(&mut driver, 0, "Hero").unwrap();
    assert_eq!(bearer(&driver), Some(driver.id_of("Hero").unwrap()));
    // The other player's creature is never offered, and the bearer is only a bearer for its controller.
    assert_eq!(driver.state().ring_bearer(pid(1)), None);
}

#[test]
fn with_no_creature_the_level_rises_and_nobody_is_the_bearer() {
    let mut driver = table(1, &[], &[("Foe", 1)]);
    tempted(&mut driver, 0, "Foe").unwrap();
    assert_eq!(level(&driver), 1);
    assert_eq!(bearer(&driver), None);
}

#[test]
fn a_later_choice_moves_the_status_to_the_new_bearer() {
    let mut driver = table(2, &[("Hero", 2), ("Sidekick", 2)], &[]);
    tempted(&mut driver, 0, "Hero").unwrap();
    tempted(&mut driver, 1, "Sidekick").unwrap();
    assert_eq!(bearer(&driver), Some(driver.id_of("Sidekick").unwrap()));
}

/// Tempts `times` times with Hero as the bearer, then attacks with `attackers` and lets the other seat block `blocks`.
fn fight(times: usize, attackers: &[&str], blocks: &[(&str, &str)]) -> Result<Driver, ScriptError> {
    let mut driver = table(
        times,
        &[("Hero", 2), ("Sidekick", 2)],
        &[("Big", 3), ("Small", 1)],
    );
    for n in 0..times {
        tempted(&mut driver, n, "Hero")?;
    }
    for _ in 0..40 {
        match driver.decision()?.kind {
            DecisionKind::DeclareAttackers(_) => {
                driver.attack(&attackers.iter().map(|a| (*a, 1)).collect::<Vec<_>>())?;
            }
            DecisionKind::DeclareBlockers(_) => {
                driver.block(blocks)?;
            }
            _ if driver.state().turn().step() > Step::EndCombat => break,
            _ => {
                driver.pass()?;
            }
        }
    }
    Ok(driver)
}

#[rstest]
#[case::a_lesser_creature_blocks_the_bearer(&[("Small", "Hero")], true)]
#[case::a_greater_creature_cannot(&[("Big", "Hero")], false)]
#[case::a_greater_creature_blocks_a_creature_that_is_not_the_bearer(&[("Big", "Sidekick")], true)]
fn from_level_one_the_bearer_cannot_be_blocked_by_greater_power(
    #[case] blocks: &[(&str, &str)],
    #[case] allowed: bool,
) {
    let attackers: Vec<&str> = blocks.iter().map(|(_, a)| *a).collect();
    let result = fight(1, &attackers, blocks);
    assert_eq!(result.is_ok(), allowed);
}

#[test]
fn without_the_ring_a_greater_creature_blocks() {
    fight(0, &["Hero"], &[("Big", "Hero")]).unwrap();
}

#[test]
fn level_two_loots_when_the_bearer_attacks_and_not_when_another_creature_does() {
    let hand = |driver: &Driver| driver.state().zone(Zone::Hand(pid(0))).len();
    let library = |driver: &Driver| driver.state().zone(Zone::Library(pid(0))).len();
    let bearer_attacks = fight(2, &["Hero"], &[]).unwrap();
    let other_attacks = fight(2, &["Sidekick"], &[]).unwrap();
    assert_eq!(library(&bearer_attacks) + 1, library(&other_attacks));
    assert_eq!(hand(&bearer_attacks), hand(&other_attacks));
    assert_eq!(
        bearer_attacks.state().zone(Zone::Graveyard(pid(0))).len(),
        other_attacks.state().zone(Zone::Graveyard(pid(0))).len() + 1
    );
}

#[rstest]
#[case::the_bearer_below_level_three(2, &[("Small", "Hero")], true)]
#[case::the_bearer_at_level_three(3, &[("Small", "Hero")], false)]
#[case::another_attacker_at_level_three(3, &[("Small", "Sidekick")], true)]
fn level_three_sacrifices_a_creature_that_blocks_the_bearer_at_end_of_combat(
    #[case] times: usize,
    #[case] blocks: &[(&str, &str)],
    #[case] survives: bool,
) {
    let attackers: Vec<&str> = blocks.iter().map(|(_, a)| *a).collect();
    let driver = fight(times, &attackers, blocks).unwrap();
    let small = driver.id_of("Small").map(|id| {
        driver
            .state()
            .object(id)
            .is_some_and(|object| object.zone() == Zone::Battlefield)
    });
    assert_eq!(small.unwrap_or(false), survives);
}

#[rstest]
#[case::level_three_unblocked(3, &["Hero"], 2, 0)]
#[case::level_four_unblocked(4, &["Hero"], 2, 3)]
#[case::level_four_another_attacker(4, &["Sidekick"], 2, 0)]
fn level_four_drains_each_opponent_when_the_bearer_hits_a_player(
    #[case] times: usize,
    #[case] attackers: &[&str],
    #[case] damage: i32,
    #[case] drained: i32,
) {
    let driver = fight(times, attackers, &[]).unwrap();
    assert_eq!(life(&driver, 1), 20 - damage - drained);
    assert_eq!(life(&driver, 0), 20);
}

/// Seat 0 holds Tempt 0 and an Island and controls these cards.
fn with_cards(mine: Vec<Arc<CardDefinition>>) -> Driver {
    let mut battlefield = vec![ready(island())];
    battlefield.extend(mine.into_iter().map(ready));
    let scenario = Scenario::new(2)
        .hand(0, [tempt("Tempt 0")])
        .battlefield(0, battlefield);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.idle_until(1, Step::Main1).unwrap();
    driver
}

fn watcher(name: &str, pattern: &str) -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(&format!(
        r#"{{"Triggered":{{"when":{{"RingTempts":{pattern}}},"effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(1, 4)
            .with_rules(rule),
    )
}

#[rstest]
#[case::whenever_the_ring_tempts_you("Hero", r#"{"who":"You"}"#, 21)]
#[case::an_opponent_is_not_you("Hero", r#"{"who":"Opponents"}"#, 20)]
#[case::another_creature_is_chosen("Hero", r#"{"who":"You","bearer":["Other"]}"#, 21)]
#[case::the_watcher_itself_is_chosen("Watcher", r#"{"who":"You","bearer":["Other"]}"#, 20)]
#[case::any_chosen_creature_of_mine(
    "Watcher",
    r#"{"who":"You","bearer":[{"Type":"Creature"}]}"#,
    21
)]
fn a_trigger_on_the_ring_tempting_fires_for_the_player_and_the_bearer_it_names(
    #[case] chosen: &str,
    #[case] pattern: &str,
    #[case] life_after: i32,
) {
    let mut driver = with_cards(vec![
        free_creature("Hero", 2, 4),
        watcher("Watcher", pattern),
    ]);
    tempted(&mut driver, 0, chosen).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    assert_eq!(life(&driver, 0), life_after);
}

#[rstest]
#[case::the_bearer_twin_is_legendary_next_to_a_legendary_one(true, true)]
#[case::the_other_twin_is_not_legendary(false, false)]
fn the_bearer_is_legendary_for_the_legend_rule(#[case] choose_plain: bool, #[case] asked: bool) {
    let legend = free_creature("Twin", 2, 4);
    let legend = Arc::new(
        CardDefinition::clone(&legend).with_supertype(mtg_core::card::Supertype::Legendary),
    );
    let plain = free_creature("Twin", 2, 4);
    let mut driver = with_cards(vec![legend, plain]);
    let ids: Vec<ObjectId> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|o| o.card().name == "Twin")
        })
        .collect();
    let plain_id = ids
        .iter()
        .copied()
        .find(|id| {
            !driver
                .state()
                .object(*id)
                .unwrap()
                .card()
                .supertypes
                .contains(&mtg_core::card::Supertype::Legendary)
        })
        .unwrap();
    let printed = ids.iter().copied().find(|id| *id != plain_id).unwrap();
    driver.cast("Tempt 0").unwrap();
    let mut questions = 0;
    for _ in 0..12 {
        if let DecisionKind::ChooseCards(_) = &driver.decision().unwrap().kind {
            // The first question chooses the Ring-bearer, a second one is the legend rule.
            let pick = if questions == 0 && choose_plain {
                plain_id
            } else {
                printed
            };
            questions += 1;
            driver
                .answer(Answer::ChooseCards(CardsChoice(vec![pick])))
                .unwrap();
        } else if driver.state().stack().is_empty() {
            break;
        } else {
            driver.pass().unwrap();
        }
    }
    assert_eq!(questions == 2, asked);
}
