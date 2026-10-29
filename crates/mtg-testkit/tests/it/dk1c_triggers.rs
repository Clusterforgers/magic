//! Goal: the deck's targeted-spell triggers play as their cards say at two, three and four seats: Illuminator Virtuoso connives when a spell its controller controls targets it (not for an ability, an opponent's spell or another creature), and Hero of Iroas grows for each spell its controller casts at it and makes their Aura spells cost {1} less.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{name_of, pid};
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, island, mountain, pinger};
use mtg_testkit::corpus::card;
use mtg_testkit::pump_cards::grant;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const VIRTUOSO: &str = "Illuminator Virtuoso";
const HERO: &str = "Hero of Iroas";

fn counters(driver: &Driver, name: &str) -> u32 {
    // A creature that died has no counters.
    let Ok(id) = driver.id_of(name) else {
        return 0;
    };
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        })
}

fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Passes priority while the stack holds something; a card the creature connives away is `discard`.
fn settle(driver: &mut Driver, discard: &str) -> Result<bool, ScriptError> {
    let mut asked = false;
    for _ in 0..60 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseCards(options) => {
                let chosen = options
                    .from
                    .iter()
                    .copied()
                    .find(|id| name_of(driver.state(), *id) == discard)
                    .unwrap();
                asked = true;
                driver.answer(Answer::ChooseCards(CardsChoice(vec![chosen])))?;
            }
            DecisionKind::Priority(_) if !driver.state().stack().is_empty() => {
                driver.pass()?;
            }
            _ => return Ok(asked),
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// What targets a permanent.
#[derive(Debug, Clone, Copy)]
enum Act {
    /// Seat 0 casts its Rush A at the permanent.
    MySpell(&'static str),
    /// Seat 0 taps its Pinger at the permanent.
    MyAbility(&'static str),
    /// Seat 1 casts its Rush at the permanent once seat 0 passes priority.
    TheirSpell(&'static str),
}

/// Seat 0 controls `subject`, a Bear, a Pinger and three Mountains and holds two Rushes; seat 1 controls a Mountain and
/// holds a Rush. Every seat has `top` above its library.
fn table(
    seats: usize,
    subject: &str,
    top: &std::sync::Arc<mtg_cards::CardDefinition>,
) -> Result<Driver, ScriptError> {
    let rush = |name: &str| grant(name, Keyword::Haste);
    let mut scenario = Scenario::new(seats)
        .hand(0, [rush("Rush A"), rush("Rush B")])
        .hand(1, [rush("Their Rush")])
        .battlefield(
            0,
            [
                ready(card(subject).unwrap()),
                ready(free_creature("Bear", 2, 2)),
                ready(pinger("Pinger", 0)),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
            ],
        )
        .battlefield(1, [ready(mountain())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, [top.clone(), card("Serra Angel").unwrap()]);
    }
    Driver::from_scenario(scenario)
}

/// Does `act`; true when a creature asked which card to discard.
fn act(driver: &mut Driver, act: Act, discard: &str) -> Result<bool, ScriptError> {
    match act {
        Act::MySpell(at) => {
            let at = driver.target_named(at)?;
            driver.cast_at("Rush A", &[at])?;
        }
        Act::MyAbility(at) => {
            driver.begin_activation("Pinger")?;
            let at = driver.target_named(at)?;
            driver.choose_targets(&[at])?;
        }
        Act::TheirSpell(at) => {
            driver.pass()?;
            assert_eq!(driver.decision()?.player, pid(1), "seat 1 has priority");
            let at = driver.target_named(at)?;
            driver.cast_at("Their Rush", &[at])?;
        }
    }
    settle(driver, discard)
}

fn library_len(driver: &Driver) -> usize {
    driver.state().zone(Zone::Library(pid(0))).len()
}

#[rstest]
fn a_spell_its_controller_controls_that_targets_virtuoso_makes_it_connive(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] land_on_top: bool,
    #[values("drawn", "Rush B")] discarded: &str,
) -> Result<(), ScriptError> {
    let (top, drawn) = if land_on_top {
        (forest(), "Forest")
    } else {
        (card("Grizzly Bears").unwrap(), "Grizzly Bears")
    };
    let mut driver = table(seats, VIRTUOSO, &top)?;
    let discard = if discarded == "drawn" {
        drawn
    } else {
        discarded
    };
    let before = library_len(&driver);
    assert!(act(&mut driver, Act::MySpell(VIRTUOSO), discard)?);
    assert_eq!(library_len(&driver), before - 1, "it drew a card");
    let graveyard: Vec<String> = driver
        .state()
        .zone(Zone::Graveyard(pid(0)))
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect();
    assert!(graveyard.contains(&discard.to_owned()), "{graveyard:?}");
    let nonland = discard != "Forest";
    assert_eq!(counters(&driver, VIRTUOSO), u32::from(nonland));
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::another_creature(Act::MySpell("Bear"))]
#[case::its_own_controllers_ability(Act::MyAbility(VIRTUOSO))]
#[case::an_opponents_spell(Act::TheirSpell(VIRTUOSO))]
fn nothing_else_makes_virtuoso_connive(
    #[case] what: Act,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, VIRTUOSO, &forest())?;
    let before = library_len(&driver);
    assert!(!act(&mut driver, what, "Forest")?);
    assert_eq!(library_len(&driver), before);
    assert_eq!(counters(&driver, VIRTUOSO), 0);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn each_spell_cast_at_hero_of_iroas_gives_it_a_counter(
    #[values(1, 2)] spells: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, HERO, &forest())?;
    for number in 0..spells {
        let name = ["Rush A", "Rush B"][number];
        let at = driver.target_named(HERO)?;
        driver.cast_at(name, &[at])?;
        settle(&mut driver, "Forest")?;
    }
    assert_eq!(counters(&driver, HERO), u32::try_from(spells).unwrap());
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::a_spell_at_another_creature(Act::MySpell("Bear"))]
#[case::an_ability_at_it(Act::MyAbility(HERO))]
#[case::an_opponents_spell_at_it(Act::TheirSpell(HERO))]
fn only_a_spell_its_controller_casts_at_hero_of_iroas_counts(
    #[case] what: Act,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, HERO, &forest())?;
    act(&mut driver, what, "Forest")?;
    assert_eq!(counters(&driver, HERO), 0);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn hero_of_iroas_makes_aura_spells_cost_one_less(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] with_hero: bool,
) -> Result<(), ScriptError> {
    // Eel Umbra costs {1}{U}: one Island pays for it only under Hero of Iroas.
    let mut board = vec![ready(island()), ready(free_creature("Bear", 2, 2))];
    if with_hero {
        board.push(ready(card(HERO).unwrap()));
    }
    let scenario = Scenario::new(seats)
        .hand(0, [card("Eel Umbra").unwrap()])
        .battlefield(0, board);
    let mut driver = Driver::from_scenario(scenario)?;
    let at = driver.target_named("Bear")?;
    let cast = driver.cast_at("Eel Umbra", &[at]);
    assert_eq!(cast.is_ok(), with_hero, "{cast:?}");
    Ok(())
}
