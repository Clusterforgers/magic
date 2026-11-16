//! Goal: myriad and the combat-damage trigger of Battle Angels of Tyr play whole in team games with Isshin doubling the attack trigger: each trigger makes one copy for the one other opponent (a teammate is no opponent), every copy's damage lowers the life of the player it attacks, and each damage trigger reads "that player" as the player its own source hurt.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::card::Card;
use mtg_core::card::CardType;
use mtg_core::house::HouseRules;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::choose::YesNoChoice;
use mtg_engine::combat::{AttackTarget, DamageTarget};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::forest;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

const ANGELS: &str = "Battle Angels of Tyr";

/// The Angels as the committed draft reads: a 4/4 with the draft's abilities.
fn angels() -> Arc<CardDefinition> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../mtg-cli/drafts/battle_angels_of_tyr.json");
    let draft: Card = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let card = CardDefinition::new(ANGELS)
        .with_type(CardType::Creature)
        .with_power_toughness(4, 4);
    Arc::new(
        draft
            .faces
            .into_iter()
            .flat_map(|face| face.abilities)
            .fold(card, |card, ability: Ability| card.with_rules(ability)),
    )
}

/// What a seat starts with: its life, the cards in its hand and the lands it controls.
#[derive(Clone, Copy)]
struct Seat {
    life: i32,
    hand: usize,
    lands: usize,
}

const fn seat(life: i32, hand: usize, lands: usize) -> Seat {
    Seat { life, hand, lands }
}

/// A table of four where seat 0 controls Isshin and the Angels and has the turn.
fn table(mode: TeamMode, teams: [u32; 4], seats: [Seat; 4]) -> Scenario {
    let mut scenario = Scenario::new(4)
        .at(1, 0, Step::Main1)
        .house_rules(HouseRules {
            teams: Some(Teams {
                mode,
                seats: teams.to_vec(),
            }),
            ..HouseRules::default()
        });
    for (index, start) in seats.iter().enumerate() {
        let mut permanents: Vec<_> = (0..start.lands).map(|_| ready(forest())).collect();
        if index == 0 {
            permanents.push(ready(angels()));
            permanents.push(ready(card("Isshin, Two Heavens as One").unwrap()));
        }
        scenario = scenario
            .life(index, start.life)
            .hand(index, (0..start.hand).map(|_| forest()))
            .library(index, (0..8).map(|_| forest()))
            .battlefield(index, permanents);
    }
    scenario
}

/// Seat 0 attacks seat `defender` with the Angels and answers every myriad question with yes, passing until the stack
/// is empty; how many were asked.
fn attack(driver: &mut Driver, defender: usize) -> Result<usize, ScriptError> {
    driver.idle_until(1, Step::DeclareAttackers)?;
    driver.attack(&[(ANGELS, defender)])?;
    let mut asked = 0;
    for _ in 0..40 {
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                asked += 1;
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            DecisionKind::Priority(_) if !driver.state().stack().is_empty() => {
                driver.pass()?;
            }
            _ => break,
        }
    }
    Ok(asked)
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn hand_size(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

fn treasures(driver: &Driver) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| name_of(state, **id) == "Treasure")
        .count()
}

/// Whom each copy of the Angels made so far attacks.
fn copies(driver: &Driver) -> Vec<Option<AttackTarget>> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::TokenCreated(token) if token.name == ANGELS => Some(token.attacking),
            _ => None,
        })
        .collect()
}

/// The damage dealt to each player, summed per seat.
fn hits(driver: &Driver) -> [i32; 4] {
    let mut hit = [0; 4];
    for event in driver.engine().events() {
        if let Event::DamageDealt {
            target: DamageTarget::Player(player),
            amount,
            ..
        } = event
        {
            hit[player.index()] += i32::try_from(*amount).unwrap();
        }
    }
    hit
}

/// The objects whose combat-damage triggers went on the stack so far, in the order they triggered.
fn damage_triggers(driver: &Driver) -> Vec<u64> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::AbilityTriggered(triggered) => triggered.cause.source.map(|dealer| dealer.id.0),
            _ => None,
        })
        .collect()
}

fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::teams_with_the_teammate_after_the_attacker(TeamMode::Teams, [0, 1, 1, 0], 1, 2)]
#[case::teams_alternating(TeamMode::Teams, [0, 1, 0, 1], 1, 3)]
#[case::shared_turns(TeamMode::SharedTurns, [0, 0, 1, 1], 2, 3)]
fn isshin_makes_each_myriad_trigger_copy_for_the_one_other_opponent_and_each_copy_hits_that_player(
    #[case] mode: TeamMode,
    #[case] teams: [u32; 4],
    #[case] defender: usize,
    #[case] other: usize,
) -> Result<(), ScriptError> {
    let seats = [
        seat(30, 0, 3),
        seat(30, 0, 1),
        seat(30, 0, 1),
        seat(30, 0, 1),
    ];
    let mut driver = Driver::from_scenario(table(mode, teams, seats))?;
    // Isshin: the attack trigger of myriad triggers an additional time; a teammate is no opponent (CR 702.116a).
    assert_eq!(attack(&mut driver, defender)?, 2);
    assert_eq!(
        copies(&driver),
        vec![Some(AttackTarget::Player(pid(other))); 2],
        "one copy for each trigger, both at the one opponent who is not the defending player"
    );
    driver.idle_until(1, Step::Main2)?;
    let mut expected = [0; 4];
    expected[defender] = 4;
    expected[other] = 8;
    assert_eq!(
        hits(&driver),
        expected,
        "the Angels hit the defender, each copy its player"
    );
    for (seat, dealt) in expected.iter().enumerate() {
        assert_eq!(
            life(&driver, seat),
            30 - dealt,
            "seat {seat} after the combat"
        );
    }
    assert_eq!(damage_triggers(&driver).len(), 3);
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::the_copies_hurt_the_leader(
    [seat(20, 0, 3), seat(20, 1, 1), seat(30, 3, 4), seat(20, 2, 2)],
    2, 2, 23
)]
#[case::the_second_copy_reads_the_life_the_first_one_gained(
    [seat(5, 0, 3), seat(20, 1, 1), seat(30, 3, 4), seat(20, 2, 2)],
    2, 2, 11
)]
#[case::the_angels_hurt_the_leader(
    [seat(20, 0, 3), seat(40, 5, 5), seat(20, 3, 4), seat(20, 2, 2)],
    1, 1, 23
)]
#[case::no_one_leads(
    [seat(20, 2, 3), seat(20, 2, 3), seat(20, 2, 3), seat(20, 2, 3)],
    2, 0, 20
)]
fn each_damage_trigger_reads_the_player_its_own_source_hurt(
    #[case] seats: [Seat; 4],
    #[case] hand: usize,
    #[case] made: usize,
    #[case] controller_life: i32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(TeamMode::Teams, [0, 1, 1, 0], seats))?;
    attack(&mut driver, 1)?;
    driver.idle_until(1, Step::Main2)?;
    let triggers = damage_triggers(&driver);
    assert_eq!(triggers.len(), 3, "the Angels and each copy trigger once");
    assert_eq!(triggers.iter().collect::<BTreeSet<_>>().len(), 3);
    // "That player" is Bot 1 for the Angels and Bot 2 for each copy; the lead in cards, lands and life decides.
    assert_eq!(hand_size(&driver, 0), hand, "cards drawn");
    assert_eq!(treasures(&driver), made, "Treasures made");
    assert_eq!(life(&driver, 0), controller_life, "life of the controller");
    // The damage lowered the life of the players it hit, whatever the triggers did.
    assert_eq!(life(&driver, 1), seats[1].life - 4);
    assert_eq!(life(&driver, 2), seats[2].life - 8);
    assert_eq!(life(&driver, 3), seats[3].life);
    replays(&driver);
    Ok(())
}
