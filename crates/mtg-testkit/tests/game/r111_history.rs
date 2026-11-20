//! Goal: facts kept across turns and within one, at 2 to 4 seats: a player can be attacked under Arboria only when they
//! cast a spell or put a nontoken permanent onto the battlefield during their last turn; Territorial Hellkite attacks an
//! opponent it didn't attack in its controller's last combat, or is tapped when there is none; Suffocation is cast only
//! after a red instant or sorcery spell dealt you damage and hits the controller of the last one; Backdraft deals half
//! the damage of the sorcery spell its controller chooses. Each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::refs::{Ref, Targetable};
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::events::Event;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{burn, cost, damage, forest, free_creature, island, mountain, one_target};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::{on_battlefield, pid};

/// A sorcery for `{R}`: `amount` damage to any target.
fn blaze(name: &str, amount: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(Targetable::AnyTarget)],
                effects: vec![damage(Ref::Target(0), amount)],
            }),
    )
}

/// Every seat with ten Forests in its library, so no one decks out.
fn libraries(mut scenario: Scenario, seats: usize) -> Scenario {
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 10]);
    }
    scenario
}

/// The log replays to the live state.
fn replays(scenario: &Scenario, driver: &Driver) {
    let setup = scenario.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

/// Answers passively until turn `turn` asks for its attackers; the players the creature named may attack then, none when
/// no declaration is asked.
fn defenders_on(driver: &mut Driver, turn: u32, name: &str) -> Vec<usize> {
    for _ in 0..2000 {
        let state = driver.state();
        if state.turn().number() > turn
            || (state.turn().number() == turn && state.turn().step() > Step::DeclareAttackers)
        {
            return Vec::new();
        }
        let decision = driver.decision().unwrap().clone();
        if let DecisionKind::DeclareAttackers(options) = &decision.kind
            && driver.state().turn().number() == turn
        {
            let id = on_battlefield(driver.state(), name).unwrap();
            return options
                .attackers
                .iter()
                .filter(|option| option.object == id)
                .flat_map(|option| option.defenders.iter())
                .filter_map(|target| match target {
                    AttackTarget::Player(player) => Some(player.index()),
                    AttackTarget::Planeswalker(_) => None,
                })
                .collect();
        }
        driver.answer_passively().unwrap();
    }
    panic!("turn {turn} never came");
}

/// What seat 1 does during its turn under Arboria.
#[derive(Debug, Clone, Copy)]
enum Deed {
    Nothing,
    PlaysLand,
    CastsCreature,
}

#[rstest]
fn r111_arboria_opens_the_players_who_did_something(
    #[values(2, 3, 4)] seats: usize,
    #[values(Deed::Nothing, Deed::PlaysLand, Deed::CastsCreature)] deed: Deed,
) {
    let scenario = libraries(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(
                0,
                [
                    ready(card("Arboria").unwrap()),
                    ready(free_creature("Raider", 2, 2)),
                ],
            )
            .hand(1, [mountain(), free_creature("Cub", 1, 1)]),
        seats,
    );
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    assert_eq!(
        defenders_on(&mut driver, 1, "Raider"),
        Vec::<usize>::new(),
        "no one has had a turn"
    );
    driver.idle_until(2, Step::Main1).unwrap();
    match deed {
        Deed::Nothing => {}
        Deed::PlaysLand => {
            driver.play_land("Mountain").unwrap();
        }
        Deed::CastsCreature => {
            driver.cast("Cub").unwrap();
            while !driver.state().stack().is_empty() {
                driver.pass().unwrap();
            }
        }
    }
    let open = defenders_on(&mut driver, u32::try_from(seats).unwrap() + 1, "Raider");
    let expected: Vec<usize> = match deed {
        Deed::Nothing => Vec::new(),
        _ => vec![1],
    };
    assert_eq!(open, expected, "{deed:?}");
    replays(&scenario, &driver);
}

/// The players Territorial Hellkite attacked in the declaration of the turn.
fn hellkite_attacks(driver: &Driver, from: usize) -> Vec<usize> {
    let id = on_battlefield(driver.state(), "Territorial Hellkite").unwrap();
    driver.engine().events()[from..]
        .iter()
        .filter_map(|event| match event {
            Event::AttackersDeclared { attackers, .. } => Some(attackers.clone()),
            _ => None,
        })
        .flatten()
        .filter(|(object, _)| *object == id)
        .filter_map(|(_, target)| match target {
            AttackTarget::Player(player) => Some(player.index()),
            AttackTarget::Planeswalker(_) => None,
        })
        .collect()
}

#[rstest]
fn r111_territorial_hellkite_attacks_someone_new(#[values(2, 3, 4)] seats: usize) {
    let scenario = libraries(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, [ready(card("Territorial Hellkite").unwrap())]),
        seats,
    );
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let from = driver.engine().events().len();
    driver.idle_until(1, Step::End).unwrap();
    let first = hellkite_attacks(&driver, from);
    assert_eq!(first.len(), 1, "it attacks one opponent the first time");
    assert_ne!(first[0], 0);
    let next = u32::try_from(seats).unwrap() + 1;
    let from = driver.engine().events().len();
    driver.idle_until(next, Step::End).unwrap();
    let second = hellkite_attacks(&driver, from);
    let hellkite = on_battlefield(driver.state(), "Territorial Hellkite").unwrap();
    let tapped = driver
        .state()
        .object(hellkite)
        .and_then(|object| object.permanent())
        .is_some_and(mtg_engine::object::PermanentState::tapped);
    if seats == 2 {
        assert!(
            second.is_empty(),
            "the only opponent was attacked last combat"
        );
        assert!(tapped, "it is tapped instead");
    } else {
        assert_eq!(second.len(), 1);
        assert_ne!(second[0], first[0], "another opponent");
        assert_ne!(second[0], 0);
    }
    replays(&scenario, &driver);
}

/// Whether the deciding player may cast the card named.
fn castable(driver: &mut Driver, name: &str) -> bool {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        return false;
    };
    let options = options.clone();
    options.castable.iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|object| object.card().name == name)
    })
}

#[rstest]
#[case::a_red_instant("Bolt", true)]
#[case::a_green_instant("Sting", false)]
fn r111_suffocation_hits_the_last_red_spell_controller(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] red: bool,
) {
    let sting = Arc::new(
        CardDefinition::new("Sting")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Green]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(Targetable::AnyTarget)],
                effects: vec![damage(Ref::Target(0), 3)],
            }),
    );
    let scenario = libraries(
        Scenario::new(seats)
            .at(1, 1, Step::Main1)
            .battlefield(0, [ready(island()), ready(island())])
            .hand(0, [card("Suffocation").unwrap()])
            .battlefield(1, [ready(mountain()), ready(forest())])
            .hand(1, [burn("Bolt", 3), sting]),
        seats,
    );
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let victim = driver.target_player(0);
    driver.cast_at(spell, &[victim]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    while driver.decision().unwrap().player != pid(0) {
        driver.pass().unwrap();
    }
    assert_eq!(castable(&mut driver, "Suffocation"), red, "{spell}");
    if red {
        let life = driver.state().player(pid(1)).unwrap().life();
        driver.cast("Suffocation").unwrap();
        while !driver.state().stack().is_empty() {
            driver.pass().unwrap();
        }
        assert_eq!(driver.state().player(pid(1)).unwrap().life(), life - 4);
    }
    replays(&scenario, &driver);
}

#[rstest]
#[case::the_bigger_one(1, 2)]
#[case::the_smaller_one(0, 1)]
fn r111_backdraft_halves_the_chosen_sorcery(
    #[values(2, 3, 4)] seats: usize,
    #[case] pick: usize,
    #[case] dealt: i32,
) {
    let scenario = libraries(
        Scenario::new(seats)
            .at(1, 1, Step::Main1)
            .battlefield(0, [ready(mountain()), ready(mountain())])
            .hand(0, [card("Backdraft").unwrap()])
            .battlefield(1, [ready(mountain()), ready(mountain())])
            .hand(1, [blaze("Small Blaze", 3), blaze("Big Blaze", 5)]),
        seats,
    );
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    for name in ["Small Blaze", "Big Blaze"] {
        let victim = driver.target_player(0);
        driver.cast_at(name, &[victim]).unwrap();
        while !driver.state().stack().is_empty() {
            driver.pass().unwrap();
        }
    }
    while driver.decision().unwrap().player != pid(0) {
        driver.pass().unwrap();
    }
    let life = driver.state().player(pid(1)).unwrap().life();
    driver.cast("Backdraft").unwrap();
    let mut offered = Vec::new();
    for _ in 0..50 {
        if driver.state().stack().is_empty() {
            break;
        }
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::ChooseOption(options)
                if options.text == "Choose one of those spells." =>
            {
                offered = options.options.clone();
                driver
                    .answer(Answer::ChooseOption(OptionChoice(pick)))
                    .unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    assert_eq!(
        offered,
        vec![
            ChoiceOption::Label("Small Blaze (3 damage)".into()),
            ChoiceOption::Label("Big Blaze (5 damage)".into()),
        ]
    );
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), life - dealt);
    replays(&scenario, &driver);
}
