//! Goal: limits on the attacks at one player or planeswalker play through the engine at tables of two, three and four
//! seats: Crawlspace lets two creatures attack its controller and leaves the other defenders open, a declaration of more
//! is refused by name and leaves no trace, Judoon Enforcers lets one of the creatures that must attack reach its
//! controller (the others attack elsewhere or stay home), The Eternal Wanderer takes one attacker, and bot games under
//! Crawlspace never send more than two creatures at its controller; every answer keeps the invariants and the log
//! replays to the state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::modify::GameRule;
use mtg_cards::refs::{Ref, ZoneKind};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::turn::Step;
use mtg_engine::{EngineError, Event};
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, RandomBot, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::pid;
use crate::w8a_declaration_limits::{w8a_berserker, w8a_until};

type W10aResult = Result<(), ScriptError>;

/// Whether `result` is the refusal of a declaration over the limit `allowed`.
fn w10a_too_many(result: &Result<Vec<Event>, ScriptError>, allowed: usize) -> bool {
    matches!(
        result,
        Err(ScriptError::Engine(EngineError::Illegal(IllegalAnswer::TooManyCreatures { allowed: a, .. })))
            if *a == allowed
    )
}

/// The attack options of the decision the driver waits on.
fn w10a_options(driver: &Driver) -> mtg_engine::decision::AttackOptions {
    let DecisionKind::DeclareAttackers(options) = driver.decision().unwrap().kind.clone() else {
        panic!("not declaring attackers");
    };
    options
}

#[rstest]
fn w10a_crawlspace_lets_two_attack_its_controller(#[values(2, 3, 4)] seats: usize) -> W10aResult {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(free_creature("Ash", 2, 2)),
                ready(free_creature("Birch", 2, 2)),
                ready(free_creature("Cedar", 2, 2)),
            ],
        )
        .battlefield(1, [ready(card("Crawlspace").unwrap())]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    w8a_until(&mut driver, 0, "declare-attackers")?;
    let options = w10a_options(&driver);
    assert_eq!(options.most_at, [(AttackTarget::Player(pid(1)), 2)]);
    assert_eq!(options.most, None);
    let log = driver.engine().events().len();
    assert!(w10a_too_many(
        &driver.attack(&[("Ash", 1), ("Birch", 1), ("Cedar", 1)]),
        2
    ));
    assert_eq!(
        driver.engine().events().len(),
        log,
        "a refused attack left a trace"
    );
    if seats > 2 {
        driver.attack(&[("Ash", 1), ("Birch", 1), ("Cedar", 2)])?;
    } else {
        driver.attack(&[("Ash", 1), ("Birch", 1)])?;
    }
    driver.idle_until(2, Step::Main2)?;
    assert_eq!(
        driver.state().player(pid(1)).unwrap().life(),
        16,
        "two 2/2s connect"
    );
    if seats > 2 {
        assert_eq!(driver.state().player(pid(2)).unwrap().life(), 18);
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w10a_judoon_enforcers_let_one_creature_that_must_attack_reach_it(
    #[values(2, 3, 4)] seats: usize,
) -> W10aResult {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(w8a_berserker("Ash")),
                ready(w8a_berserker("Birch")),
                ready(free_creature("Fox", 1, 1)),
            ],
        )
        .battlefield(1, [ready(card("Judoon Enforcers").unwrap())]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    w8a_until(&mut driver, 0, "declare-attackers")?;
    let options = w10a_options(&driver);
    let required = options.required_attack();
    assert_eq!(required.len(), if seats > 2 { 2 } else { 1 });
    let at_one = required
        .iter()
        .filter(|(_, at)| *at == AttackTarget::Player(pid(1)))
        .count();
    assert_eq!(at_one, 1);
    assert!(w10a_too_many(
        &driver.attack(&[("Ash", 1), ("Birch", 1)]),
        1
    ));
    if seats > 2 {
        let refused = driver.attack(&[("Ash", 1), ("Fox", 2)]);
        assert!(
            matches!(
                refused,
                Err(ScriptError::Engine(EngineError::Illegal(
                    IllegalAnswer::Required(_)
                )))
            ),
            "Birch could attack seat 2: {refused:?}"
        );
        driver.attack(&[("Ash", 1), ("Birch", 2)])?;
    } else {
        driver.attack(&[("Birch", 1)])?;
    }
    driver.idle_until(2, Step::Main2)?;
    replays(&driver, &setup);
    Ok(())
}

/// The id of the permanent named `name`.
fn w10a_named(driver: &Driver, name: &str) -> ObjectId {
    driver.id_of(name).unwrap()
}

/// A planeswalker with The Eternal Wanderer's "No more than one creature can attack ~ each combat."
fn w10a_wanderer() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("The Eternal Wanderer")
            .with_type(CardType::Planeswalker)
            .with_loyalty(4)
            .with_rules(Ability::Rule {
                rule: GameRule::AttackLimitAt {
                    most: 1,
                    at: Ref::This,
                },
                condition: None,
                from: ZoneKind::Battlefield,
            }),
    )
}

#[rstest]
fn w10a_the_eternal_wanderer_takes_one_attacker(#[values(2, 3, 4)] seats: usize) -> W10aResult {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(free_creature("Ash", 2, 2)),
                ready(free_creature("Birch", 2, 2)),
            ],
        )
        .battlefield(1, [ready(w10a_wanderer())]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    w8a_until(&mut driver, 0, "declare-attackers")?;
    let walker = AttackTarget::Planeswalker(w10a_named(&driver, "The Eternal Wanderer"));
    assert_eq!(w10a_options(&driver).most_at, [(walker, 1)]);
    let (ash, birch) = (w10a_named(&driver, "Ash"), w10a_named(&driver, "Birch"));
    let both = driver.answer(Answer::DeclareAttackers(vec![
        (ash, walker),
        (birch, walker),
    ]));
    assert!(w10a_too_many(&both, 1));
    driver.answer(Answer::DeclareAttackers(vec![
        (ash, walker),
        (birch, AttackTarget::Player(PlayerId::from_index(1))),
    ]))?;
    driver.idle_until(2, Step::Main2)?;
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 18);
    replays(&driver, &setup);
    Ok(())
}

/// The most creatures one declaration in `events` sends at `target`.
fn w10a_most_at(events: &[Event], target: AttackTarget) -> usize {
    events
        .iter()
        .map(|event| match event {
            Event::AttackersDeclared { attackers, .. } => {
                attackers.iter().filter(|(_, at)| *at == target).count()
            }
            _ => 0,
        })
        .max()
        .unwrap_or(0)
}

#[rstest]
fn w10a_bots_under_crawlspace_send_two_creatures_at_most(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] seed: u64,
) -> W10aResult {
    let mut scenario = Scenario::new(seats).seed(seed).at(2, 0, Step::Main1);
    for seat in 0..seats {
        let crawlspace = (seat == 1).then(|| ready(card("Crawlspace").unwrap()));
        scenario = scenario.battlefield(
            seat,
            crawlspace.into_iter().chain([
                ready(w8a_berserker("Ash")),
                ready(w8a_berserker("Birch")),
                ready(w8a_berserker("Cedar")),
                ready(free_creature("Ox", 1, 4)),
            ]),
        );
    }
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let mut bots: Vec<RandomBot> = (0..seats as u64)
        .map(|seat| RandomBot::new(seed, seat))
        .collect();
    let mut declared = 0;
    for _ in 0..1500 {
        if driver.state().turn().number() > 8 {
            break;
        }
        let Ok(decision) = driver.decision().cloned() else {
            break;
        };
        let answer = bots[decision.player.index()].choose(&decision);
        let events = driver.answer(answer)?;
        let at = w10a_most_at(&events, AttackTarget::Player(pid(1)));
        assert!(at <= 2, "seed {seed}: {events:?}");
        declared += usize::from(at > 0);
    }
    assert!(
        declared > 0,
        "no creature ever attacked Crawlspace's controller"
    );
    replays(&driver, &setup);
    Ok(())
}
