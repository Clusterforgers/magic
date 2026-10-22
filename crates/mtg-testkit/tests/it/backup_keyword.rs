//! Goal: backup plays whole at tables of two, three and four seats as its rules say: the enters trigger puts its +1/+1 counters on a target creature of any seat, the abilities listed after the keyword (a keyword or a triggered ability) go to another creature until end of turn and to nobody else, a backup creature that targets itself only gets the counters, the counters stay when the turn ends, a target that is gone when the trigger resolves takes nothing, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::keywords::has_keyword;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_engine::{Engine, GameSetup, replay};
use mtg_testkit::cards::{cost, free_creature, gain_life};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The log replays to the live state, and an engine rebuilt from it stands at the same decision.
#[allow(clippy::unnecessary_wraps)] // the tests end with it, as they end with their last script step
fn assert_replays(driver: &Driver, setup: &GameSetup) -> Result<(), ScriptError> {
    let log = driver.engine().events().to_vec();
    let replayed = replay(setup, &log).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    let resumed = Engine::resume(setup.clone(), log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
    assert_eq!(resumed.decision(), driver.engine().decision());
    Ok(())
}

/// What the backup creature lists after its keyword.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Listed {
    Deathtouch,
    AttackTrigger,
    Both,
    Nothing,
}

fn attack_trigger() -> Ability {
    Ability::Triggered {
        when: TriggerPattern::Attacks {
            what: Selector::this(),
        },
        intervening_if: None,
        limit: None,
        from: ZoneKind::Battlefield,
        targets: Vec::new(),
        effects: vec![gain_life(1)],
    }
}

/// A 2/2 for no mana with backup `count` and the abilities `listed` after it, which it has itself as well.
fn backer(count: u32, listed: Listed) -> Arc<CardDefinition> {
    let abilities: Vec<Ability> = match listed {
        Listed::Deathtouch => vec![Ability::Keyword(Keyword::Deathtouch)],
        Listed::AttackTrigger => vec![attack_trigger()],
        Listed::Both => vec![Ability::Keyword(Keyword::Deathtouch), attack_trigger()],
        Listed::Nothing => Vec::new(),
    };
    let mut card = CardDefinition::new("Backer")
        .with_type(CardType::Creature)
        .with_mana_cost(cost(0, &[]))
        .with_power_toughness(2, 2)
        .with_rules(Ability::Keyword(Keyword::Backup {
            count,
            abilities: abilities.clone(),
        }));
    for ability in abilities {
        card = card.with_rules(ability);
    }
    Arc::new(card)
}

/// Seat 0 holds `hand` and a ready Friend in the first main phase of turn 1; every other seat has a Foe.
fn table(seats: usize, hand: Vec<Arc<CardDefinition>>) -> Scenario {
    let friend: PresetPermanent = ready(free_creature("Friend", 1, 1));
    let mut table = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, hand)
        .battlefield(0, [friend]);
    for seat in 1..seats {
        table = table.battlefield(seat, [ready(free_creature("Foe", 1, 1))]);
    }
    table
}

/// Casts the Backer, passes until its trigger asks for a target, aims it at `target` and lets it resolve.
fn back_up(driver: &mut Driver, target: &str) -> Result<(), ScriptError> {
    driver.cast("Backer")?;
    for _ in 0..8 {
        if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
            break;
        }
        driver.pass()?;
    }
    let aimed = driver.target_named(target)?;
    driver.choose_targets(&[aimed])?;
    while !driver.state().stack().is_empty()
        && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
    {
        driver.pass()?;
    }
    Ok(())
}

fn plus_counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        })
}

fn has(driver: &Driver, name: &str, keyword: &Keyword) -> bool {
    has_keyword(driver.state(), driver.id_of(name).unwrap(), keyword)
}

#[rstest]
fn backup_puts_its_counters_on_a_creature_of_any_seat_and_the_abilities_on_another_creature(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2)] count: u32,
    #[values("Friend", "Foe", "Backer")] target: &str,
) -> Result<(), ScriptError> {
    let build = || table(seats, vec![backer(count, Listed::Deathtouch)]);
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    back_up(&mut driver, target)?;
    assert_eq!(
        plus_counters(&driver, target),
        count,
        "counters on {target}"
    );
    let others = ["Friend", "Foe", "Backer"]
        .into_iter()
        .filter(|name| *name != target);
    for name in others {
        assert_eq!(plus_counters(&driver, name), 0, "no counters on {name}");
    }
    // Another creature gains deathtouch; the Backer has its own whichever creature the trigger aims at.
    assert!(has(&driver, "Backer", &Keyword::Deathtouch));
    assert!(has(&driver, target, &Keyword::Deathtouch));
    for name in ["Friend", "Foe"].into_iter().filter(|name| *name != target) {
        assert!(
            !has(&driver, name, &Keyword::Deathtouch),
            "{name} gains nothing"
        );
    }
    assert_replays(&driver, &setup)
}

#[rstest]
fn the_abilities_last_until_end_of_turn_and_the_counters_stay(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || table(seats, vec![backer(1, Listed::Deathtouch)]);
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    back_up(&mut driver, "Friend")?;
    assert!(has(&driver, "Friend", &Keyword::Deathtouch));
    driver.idle_until(2, Step::Upkeep)?;
    assert!(!has(&driver, "Friend", &Keyword::Deathtouch), "it wore off");
    assert_eq!(plus_counters(&driver, "Friend"), 1);
    assert_replays(&driver, &setup)
}

#[rstest]
fn a_listed_triggered_ability_works_for_the_creature_that_gained_it_this_turn_only(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || table(seats, vec![backer(1, Listed::AttackTrigger)]);
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    back_up(&mut driver, "Friend")?;
    driver.pass_until(Step::DeclareAttackers)?;
    let life = |driver: &Driver| driver.state().player(pid(0)).unwrap().life();
    let before = life(&driver);
    driver.attack(&[("Friend", 1)])?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert_eq!(
        life(&driver),
        before + 1,
        "the Friend's granted attack trigger"
    );
    assert_replays(&driver, &setup)
}

#[rstest]
fn a_backup_with_nothing_listed_only_puts_counters(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || table(seats, vec![backer(1, Listed::Nothing)]);
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    back_up(&mut driver, "Friend")?;
    assert_eq!(plus_counters(&driver, "Friend"), 1);
    assert_replays(&driver, &setup)
}

#[rstest]
fn a_target_that_left_in_response_takes_nothing(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || table(seats, vec![backer(1, Listed::Both)]);
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    driver.cast("Backer")?;
    for _ in 0..8 {
        if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
            break;
        }
        driver.pass()?;
    }
    let friend = driver.target_named("Friend")?;
    driver.choose_targets(&[friend])?;
    let id = driver.id_of("Friend")?;
    driver.manual(mtg_engine::manual::ManualAction::Move {
        object: id,
        to: mtg_engine::manual::ManualZone::Graveyard,
    })?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert!(driver.id_of("Friend").is_err());
    assert_eq!(plus_counters(&driver, "Backer"), 0);
    assert_replays(&driver, &setup)
}
