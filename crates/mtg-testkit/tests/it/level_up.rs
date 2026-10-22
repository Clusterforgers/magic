//! Goal: Level up, Outlast and Saddle play whole at tables of two, three and four seats, as their cards say: the abilities are offered at sorcery speed only while their costs can be paid, a level counter moves a creature through its brackets (power, toughness, keywords and abilities only while the counters are in range), outlast taps the creature for a +1/+1 counter that gives it the static ability of every creature with a counter, saddle taps other creatures for their power and makes the Mount saddled until end of turn (so its attack trigger fires only then), and the log of whole games with such cards replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::keywords::keywords_of;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{characteristics, replay};
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick, tapped, with_counters};
use rstest::rstest;

fn level() -> CounterKind {
    CounterKind::Other("level".into())
}

fn corpus(name: &str) -> Arc<CardDefinition> {
    card(name).unwrap()
}

/// Answers passively until the stack is empty and nothing resolves.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Activates the first ability of `name`, picks every creature it offers for a cost, pays whatever mana it asks for and
/// lets it resolve.
fn activate(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    if let DecisionKind::ChooseCards(options) = &driver.decision()?.kind {
        let all = CardsChoice(options.from.clone());
        driver.answer(Answer::ChooseCards(all))?;
    }
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    settle(driver)
}

fn stats(driver: &Driver, name: &str) -> (i32, i32) {
    let found = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (found.power.unwrap(), found.toughness.unwrap())
}

fn has(driver: &Driver, name: &str, keyword: &Keyword) -> bool {
    keywords_of(driver.state(), driver.id_of(name).unwrap())
        .into_iter()
        .any(|found| found == keyword)
}

fn counters(driver: &Driver, name: &str, kind: &CounterKind) -> u32 {
    driver
        .state()
        .object(driver.id_of(name).unwrap())
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(kind)
}

fn is_tapped(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped()
}

fn on_battlefield(driver: &Driver, name: &str) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| name_of(driver.state(), **id) == name)
        .count()
}

/// Seat 0 controls these permanents and `lands` Plains; seat 1 controls an Ogre.
fn table(seats: usize, permanents: Vec<PresetPermanent>, lands: usize) -> Scenario {
    let mut all = permanents;
    all.extend((0..lands).map(|_| ready(plains())));
    Scenario::new(seats)
        .battlefield(0, all)
        .battlefield(1, [ready(free_creature("Ogre", 3, 4))])
}

fn levelled(name: &str, levels: u32) -> PresetPermanent {
    with_counters(ready(corpus(name)), level(), levels)
}

// Level up

#[rstest]
#[case::with_the_mana(1, 1, 0, Step::Main1, true)]
#[case::without_the_mana(1, 0, 0, Step::Main1, false)]
#[case::in_the_other_players_turn(2, 1, 1, Step::Main1, false)]
#[case::in_combat(1, 1, 0, Step::BeginCombat, false)]
fn level_up_is_offered_at_sorcery_speed_while_its_cost_can_be_paid(
    #[values(2, 3, 4)] seats: usize,
    #[case] turn: u32,
    #[case] lands: usize,
    #[case] active: usize,
    #[case] step: Step,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let scenario =
        table(seats, vec![ready(corpus("Student of Warfare"))], lands).at(turn, active, step);
    let driver = Driver::from_scenario(scenario)?;
    let seen = !driver.offered_abilities("Student of Warfare")?.is_empty();
    assert_eq!(seen, offered);
    Ok(())
}

#[rstest]
#[case::no_level(0, (1, 1), false, false)]
#[case::one_level(1, (1, 1), false, false)]
#[case::the_first_bracket(2, (3, 3), true, false)]
#[case::inside_the_first_bracket(6, (3, 3), true, false)]
#[case::the_second_bracket(7, (4, 4), false, true)]
#[case::far_beyond(20, (4, 4), false, true)]
fn the_level_counters_decide_the_bracket_of_a_creature(
    #[values(2, 3, 4)] seats: usize,
    #[case] levels: u32,
    #[case] size: (i32, i32),
    #[case] first_strike: bool,
    #[case] double_strike: bool,
) -> Result<(), ScriptError> {
    let student = vec![levelled("Student of Warfare", levels)];
    let driver = Driver::from_scenario(table(seats, student, 0))?;
    assert_eq!(stats(&driver, "Student of Warfare"), size);
    let keyword = |keyword| has(&driver, "Student of Warfare", &keyword);
    assert_eq!(keyword(Keyword::FirstStrike), first_strike);
    assert_eq!(keyword(Keyword::DoubleStrike), double_strike);
    Ok(())
}

#[rstest]
fn each_level_up_puts_one_counter_and_moves_the_creature_into_the_bracket(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let student = vec![ready(corpus("Student of Warfare"))];
    let mut driver = Driver::from_scenario(table(seats, student, 3))?;
    activate(&mut driver, "Student of Warfare")?;
    assert_eq!(counters(&driver, "Student of Warfare", &level()), 1);
    assert_eq!(stats(&driver, "Student of Warfare"), (1, 1));
    activate(&mut driver, "Student of Warfare")?;
    assert_eq!(counters(&driver, "Student of Warfare", &level()), 2);
    assert_eq!(stats(&driver, "Student of Warfare"), (3, 3));
    assert!(has(&driver, "Student of Warfare", &Keyword::FirstStrike));
    Ok(())
}

#[rstest]
#[case::below_the_bracket(1, false, 0)]
#[case::the_first_bracket(2, true, 1)]
#[case::the_last_level_of_it(5, true, 1)]
#[case::the_second_bracket(6, true, 2)]
fn an_ability_under_a_header_works_only_while_the_levels_are_in_range(
    #[values(2, 3, 4)] seats: usize,
    #[case] levels: u32,
    #[case] offered: bool,
    #[case] tokens: usize,
) -> Result<(), ScriptError> {
    let tuskcaller = vec![levelled("Kazandu Tuskcaller", levels)];
    let mut driver = Driver::from_scenario(table(seats, tuskcaller, 0))?;
    let seen = !driver.offered_abilities("Kazandu Tuskcaller")?.is_empty();
    assert_eq!(seen, offered);
    if offered {
        activate(&mut driver, "Kazandu Tuskcaller")?;
    }
    assert_eq!(on_battlefield(&driver, "Elephant"), tokens);
    Ok(())
}

#[rstest]
fn a_lord_under_a_header_pumps_the_others_only_at_the_top_level(
    #[values(2, 3, 4)] seats: usize,
    #[values(3, 4, 9)] levels: u32,
) -> Result<(), ScriptError> {
    let commanders = vec![
        levelled("Coralhelm Commander", levels),
        ready(corpus("Coralhelm Commander")),
    ];
    let driver = Driver::from_scenario(table(seats, commanders, 0))?;
    // The second commander has no counters and is a 2/2 Merfolk: the levelled one pumps it from level 4 on.
    let unlevelled: Vec<_> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| name_of(driver.state(), *id) == "Coralhelm Commander")
        .filter(|id| {
            let permanent = driver.state().object(*id).unwrap().permanent().unwrap();
            permanent.counters_of(&level()) == 0
        })
        .filter_map(|id| characteristics(driver.state(), id).and_then(|found| found.power))
        .collect();
    assert_eq!(
        unlevelled,
        [if levels >= 4 { 3 } else { 2 }],
        "at level {levels}"
    );
    Ok(())
}

// Outlast

#[rstest]
#[case::ready(1, "ready", true)]
#[case::summoning_sick(1, "sick", false)]
#[case::tapped(1, "tapped", false)]
#[case::without_the_mana(0, "ready", false)]
fn outlast_is_offered_to_an_untapped_creature_that_can_pay(
    #[values(2, 3, 4)] seats: usize,
    #[case] lands: usize,
    #[case] how: &str,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let falconer = corpus("Abzan Falconer");
    let permanent = match how {
        "sick" => sick(falconer),
        "tapped" => tapped(falconer),
        _ => ready(falconer),
    };
    let driver = Driver::from_scenario(table(seats, vec![permanent], lands))?;
    let seen = !driver.offered_abilities("Abzan Falconer")?.is_empty();
    assert_eq!(seen, offered);
    Ok(())
}

#[rstest]
fn outlast_at_the_wrong_time_is_not_offered(
    #[values(2, 3, 4)] seats: usize,
    #[values((2, 1, Step::Main1), (1, 0, Step::End))] when: (u32, usize, Step),
) -> Result<(), ScriptError> {
    let scenario =
        table(seats, vec![ready(corpus("Abzan Falconer"))], 1).at(when.0, when.1, when.2);
    let driver = Driver::from_scenario(scenario)?;
    assert_eq!(
        driver.offered_abilities("Abzan Falconer")?,
        Vec::<usize>::new()
    );
    Ok(())
}

#[rstest]
fn outlast_taps_the_creature_for_a_counter_and_the_counter_gives_the_static_ability(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let bear = with_counters(
        ready(free_creature("Bear", 2, 2)),
        CounterKind::PlusOnePlusOne,
        1,
    );
    let permanents = vec![
        ready(corpus("Abzan Falconer")),
        bear,
        ready(free_creature("Cub", 1, 1)),
    ];
    let mut driver = Driver::from_scenario(table(seats, permanents, 1))?;
    assert!(
        has(&driver, "Bear", &Keyword::Flying),
        "a counter, so flying"
    );
    assert!(!has(&driver, "Cub", &Keyword::Flying));
    assert!(!has(&driver, "Abzan Falconer", &Keyword::Flying));
    assert_eq!(stats(&driver, "Abzan Falconer"), (2, 3));
    activate(&mut driver, "Abzan Falconer")?;
    let plus = CounterKind::PlusOnePlusOne;
    assert_eq!(counters(&driver, "Abzan Falconer", &plus), 1);
    assert_eq!(stats(&driver, "Abzan Falconer"), (3, 4));
    assert!(has(&driver, "Abzan Falconer", &Keyword::Flying));
    assert!(!has(&driver, "Cub", &Keyword::Flying));
    assert!(is_tapped(&driver, "Abzan Falconer"));
    Ok(())
}

#[rstest]
fn the_static_ability_of_outlast_reaches_only_your_own_creatures_with_counters(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let ogre = with_counters(
        ready(free_creature("Ogre", 3, 4)),
        CounterKind::PlusOnePlusOne,
        2,
    );
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(corpus("Tuskguard Captain"))])
        .battlefield(1, [ogre]);
    let driver = Driver::from_scenario(scenario)?;
    assert!(!has(&driver, "Ogre", &Keyword::Trample));
    Ok(())
}

// Saddle

/// Seat 0 controls a Bridled Bighorn and `helpers`, each a name with its power and whether it is summoning sick; seat 1 an
/// Ogre.
fn mount_table(seats: usize, helpers: &[(&str, i32, bool)]) -> Scenario {
    let mut all = vec![ready(corpus("Bridled Bighorn"))];
    for (name, power, summoning_sick) in helpers {
        let card = free_creature(name, *power, 2);
        all.push(if *summoning_sick {
            sick(card)
        } else {
            ready(card)
        });
    }
    Scenario::new(seats)
        .battlefield(0, all)
        .battlefield(1, [ready(free_creature("Ogre", 3, 4))])
}

#[rstest]
#[case::exactly_the_power(&[("Bear", 2, false)], true)]
#[case::summoning_sick_creatures_may_saddle(&[("Cub", 2, true)], true)]
#[case::two_small_creatures(&[("Mouse", 1, false), ("Rat", 1, false)], true)]
#[case::too_little_power(&[("Mouse", 1, false)], false)]
#[case::no_other_creature(&[], false)]
fn saddle_is_offered_while_other_untapped_creatures_have_the_power(
    #[values(2, 3, 4)] seats: usize,
    #[case] helpers: &[(&str, i32, bool)],
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(mount_table(seats, helpers))?;
    let seen = !driver.offered_abilities("Bridled Bighorn")?.is_empty();
    assert_eq!(seen, offered);
    Ok(())
}

#[rstest]
fn the_mount_is_saddled_until_the_end_of_the_turn_and_not_after_it(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(mount_table(seats, &[("Bear", 2, false)]))?;
    assert!(!has(&driver, "Bridled Bighorn", &Keyword::Saddled));
    activate(&mut driver, "Bridled Bighorn")?;
    assert!(has(&driver, "Bridled Bighorn", &Keyword::Saddled));
    assert!(is_tapped(&driver, "Bear"));
    assert!(!is_tapped(&driver, "Bridled Bighorn"));
    driver.idle_until(2, Step::Main1)?;
    assert!(!has(&driver, "Bridled Bighorn", &Keyword::Saddled));
    Ok(())
}

#[rstest]
#[case::saddled(true, 1)]
#[case::not_saddled(false, 0)]
fn the_mount_makes_a_sheep_when_it_attacks_only_while_saddled(
    #[values(2, 3, 4)] seats: usize,
    #[case] saddled: bool,
    #[case] sheep: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(mount_table(seats, &[("Bear", 2, false)]))?;
    if saddled {
        activate(&mut driver, "Bridled Bighorn")?;
    }
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Bridled Bighorn", 1)])?;
    driver.idle_until(1, Step::Main2)?;
    assert_eq!(on_battlefield(&driver, "Sheep"), sheep);
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 17);
    Ok(())
}

// Whole games

#[test]
fn bot_games_with_level_up_outlast_and_saddle_keep_every_invariant_and_replay() {
    use std::sync::Mutex;

    use mtg_engine::Event;
    use mtg_testkit::{PlayOptions, level_fuzz_setup, play_game_with};

    use crate::common::seeds::{for_each_seed, fuzz_games};

    // Level counters put, +1/+1 counters put, Mounts saddled and Sheep made in all the games.
    let seen = Mutex::new([0u64; 4]);
    let games = fuzz_games(12);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                level_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let replayed = replay(&level_fuzz_setup(players, seed), &outcome.events).unwrap();
            assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
            let mut seen = seen.lock().unwrap();
            for event in &outcome.events {
                match event {
                    Event::CountersChanged(changed) if changed.delta > 0 => {
                        if changed.counter == level() {
                            seen[0] += 1;
                        } else if changed.counter == CounterKind::PlusOnePlusOne {
                            seen[1] += 1;
                        }
                    }
                    Event::AbilityActivated { source, .. } => {
                        let name = replayed.object(source.id).map(|o| o.card().name.clone());
                        if name.is_some_and(|name| name == "Bridled Bighorn") {
                            seen[2] += 1;
                        }
                    }
                    Event::TokenCreated(token) if token.name == "Sheep" => seen[3] += 1,
                    _ => {}
                }
            }
        }
    });
    let [levels, outlasts, saddles, sheep] = seen.into_inner().unwrap();
    assert!(levels > 0, "some creature levelled up");
    assert!(outlasts > 0, "some creature outlasted");
    assert!(saddles > 0, "some Mount was saddled");
    assert!(sheep > 0, "some saddled Mount attacked");
}
