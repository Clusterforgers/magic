//! Goal: card names chosen with another value, at 2 to 4 seats: Booby Trap keeps an opponent and a name, that player
//! reveals each card they draw and drawing the name costs them 10 life; Null Chamber keeps a name of its controller and
//! one of the opponent they choose, and neither can be cast; a basic land name is refused; Medomai's Prophecy keeps the
//! name chapter II chose and chapter III draws two cards for the first spell of that name only. Each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::{ChoiceOption, NameChoice, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::events::Event;
use mtg_engine::object::Chosen;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest, free_creature, island, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::{on_battlefield, pid};

/// Every seat with ten Forests in its library.
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

/// A free instant named `name` that does nothing.
fn trick(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: Vec::new(),
            }),
    )
}

/// Answers until the stack is empty: the opponent `seat` when an opponent is asked for, the names in `names` in turn when
/// a name is asked for, everything else passively. Returns the players asked to name, in order.
fn resolve(driver: &mut Driver, seat: usize, names: &[&str]) -> Vec<usize> {
    let mut namers = Vec::new();
    let mut names = names.iter();
    for _ in 0..300 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return namers,
            DecisionKind::ChooseOption(options) if options.text == "Choose an opponent." => {
                let at = options
                    .options
                    .iter()
                    .position(|option| *option == ChoiceOption::Player(pid(seat)))
                    .unwrap();
                driver
                    .answer(Answer::ChooseOption(OptionChoice(at)))
                    .unwrap();
            }
            DecisionKind::ChooseName(options) => {
                assert!(options.r111_not_basic_land || !options.text.contains("basic"));
                namers.push(decision.player.index());
                let name = names.next().unwrap();
                driver
                    .answer(Answer::ChooseName(NameChoice((*name).to_owned())))
                    .unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    panic!("the stack never emptied");
}

#[rstest]
fn r111_booby_trap_catches_the_named_draw(#[values(2, 3, 4)] seats: usize) {
    let victim = seats - 1;
    let mut scenario = libraries(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, vec![ready(mountain()); 6])
            .hand(0, [card("Booby Trap").unwrap()]),
        seats,
    );
    scenario = scenario.library(
        victim,
        vec![free_creature("Bait", 1, 1), forest(), forest()],
    );
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.cast("Booby Trap").unwrap();
    let namers = resolve(&mut driver, victim, &["Bait"]);
    assert_eq!(namers, vec![0]);
    let trap = on_battlefield(driver.state(), "Booby Trap").unwrap();
    let chosen = driver
        .state()
        .object(trap)
        .unwrap()
        .permanent()
        .unwrap()
        .chosen()
        .cloned();
    assert_eq!(
        chosen,
        Some(Chosen::R111PlayerAndName(pid(victim), "Bait".to_owned()))
    );
    let life = driver.state().player(pid(victim)).unwrap().life();
    let from = driver.engine().events().len();
    driver
        .idle_until(u32::try_from(victim).unwrap() + 1, Step::Main1)
        .unwrap();
    let revealed = driver.engine().events()[from..]
        .iter()
        .any(|event| matches!(event, Event::Revealed(shown) if shown.player == pid(victim)));
    assert!(revealed, "the draw is revealed");
    assert_eq!(
        driver.state().player(pid(victim)).unwrap().life(),
        life - 10
    );
    assert!(
        on_battlefield(driver.state(), "Booby Trap").is_none(),
        "sacrificed"
    );
    replays(&scenario, &driver);
}

#[rstest]
fn r111_booby_trap_refuses_a_basic_land_name(#[values(2, 3)] seats: usize) {
    let scenario = libraries(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, vec![ready(mountain()); 6])
            .hand(0, [card("Booby Trap").unwrap()]),
        seats,
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Booby Trap").unwrap();
    for _ in 0..100 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::ChooseName(_) => {
                let refused = driver.answer(Answer::ChooseName(NameChoice("Forest".to_owned())));
                assert!(refused.is_err(), "a basic land name");
                return;
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    panic!("no name was asked for");
}

#[rstest]
fn r111_null_chamber_bans_both_names(#[values(2, 3, 4)] seats: usize) {
    let opponent = seats - 1;
    let scenario = libraries(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(
                0,
                [
                    ready(plains()),
                    ready(plains()),
                    ready(plains()),
                    ready(plains()),
                ],
            )
            .hand(
                0,
                [
                    card("Null Chamber").unwrap(),
                    trick("Mine"),
                    trick("Theirs"),
                    trick("Free"),
                ],
            ),
        seats,
    );
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.cast("Null Chamber").unwrap();
    let namers = resolve(&mut driver, opponent, &["Mine", "Theirs"]);
    assert_eq!(
        namers,
        vec![0, opponent],
        "you, then the opponent you chose"
    );
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority");
    };
    let castable: Vec<String> = options
        .castable
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .map(|object| object.card().name.clone())
        .collect();
    assert_eq!(castable, vec!["Free".to_owned()]);
    replays(&scenario, &driver);
}

#[rstest]
fn r111_medomai_draws_for_the_first_named_spell(#[values(2, 3)] seats: usize) {
    let scenario = libraries(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, [ready(island()), ready(island()), ready(island())])
            .hand(
                0,
                [
                    card("Medomai's Prophecy").unwrap(),
                    trick("Omen"),
                    trick("Omen"),
                ],
            ),
        seats,
    );
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.cast("Medomai's Prophecy").unwrap();
    resolve(&mut driver, 1, &[]);
    let rounds = u32::try_from(seats).unwrap();
    driver.idle_until(rounds + 1, Step::Main1).unwrap();
    let namers = resolve(&mut driver, 1, &["Omen"]);
    assert_eq!(namers, vec![0]);
    let saga = on_battlefield(driver.state(), "Medomai's Prophecy").unwrap();
    let chosen = driver
        .state()
        .object(saga)
        .unwrap()
        .permanent()
        .unwrap()
        .chosen()
        .cloned();
    assert_eq!(chosen, Some(Chosen::Name("Omen".to_owned())));
    driver.idle_until(2 * rounds + 1, Step::Main1).unwrap();
    resolve(&mut driver, 1, &[]);
    let hand = |driver: &Driver| driver.state().zone(Zone::Hand(pid(0))).len();
    let before = hand(&driver);
    driver.cast("Omen").unwrap();
    resolve(&mut driver, 1, &[]);
    assert_eq!(hand(&driver), before - 1 + 2, "the first Omen draws two");
    driver.cast("Omen").unwrap();
    resolve(&mut driver, 1, &[]);
    assert_eq!(hand(&driver), before, "the second draws nothing");
    replays(&scenario, &driver);
}
