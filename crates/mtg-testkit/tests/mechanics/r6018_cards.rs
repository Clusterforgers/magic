//! Goal: the r60-18 pieces play whole at two to four seats: the number of permanent types among cards in a graveyard
//! (Korvold, Gleeful Glutton), a plot cost reduction of the plotter's permanents (Doc Aurlock), a bonus to the equipped
//! creature or a creature named Vecna chosen as the trigger resolves (Hand of Vecna), and point counters whose tenth
//! makes the damaged opponent lose (Strixhaven Stadium).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, free_creature, instant, keyworded_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::{attached_to, with_counters};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Seat 0 attacks `attacker` at seat 1 and the game goes on, passing, to the second main phase with an empty stack.
fn r6018_hit(driver: &mut Driver, attackers: &[&str]) -> Result<(), ScriptError> {
    driver.pass_until(Step::DeclareAttackers)?;
    let attacks: Vec<(&str, usize)> = attackers.iter().map(|name| (*name, 1)).collect();
    driver.attack(&attacks)?;
    for _ in 0..80 {
        // The game is over when the last opponent lost.
        let Ok(decision) = driver.decision().cloned() else {
            return Ok(());
        };
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().turn().step() == Step::Main2
                    && driver.state().stack().is_empty() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("combat".into()))
}

fn counters_on(driver: &Driver, name: &str, kind: &CounterKind) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |held| held.counters_of(kind))
}

/// An artifact creature card, for the graveyard.
fn golem() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Golem")
            .with_type(CardType::Artifact)
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1),
    )
}

#[rstest]
#[case::nothing(vec![], 0)]
#[case::an_instant_is_no_permanent_type(vec![instant("Shock", 1, &[])], 0)]
#[case::a_land(vec![forest()], 1)]
#[case::artifact_creature_counts_twice(vec![golem()], 2)]
#[case::three_types(vec![golem(), forest(), instant("Shock", 1, &[]), forest()], 3)]
fn r6018_korvold_counts_permanent_types_in_your_graveyard(
    #[values(2, 3, 4)] seats: usize,
    #[case] graveyard: Vec<Arc<CardDefinition>>,
    #[case] types: u32,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .library(0, vec![forest(); 10])
        .battlefield(0, [ready(card("Korvold, Gleeful Glutton").unwrap())])
        .graveyard(0, graveyard);
    // An opponent's graveyard does not count.
    scenario = scenario.graveyard(1, [golem(), forest()]);
    let mut driver = Driver::from_scenario(scenario)?;
    let hand = driver
        .state()
        .zone(mtg_engine::zones::Zone::Hand(pid(0)))
        .len();
    r6018_hit(&mut driver, &["Korvold, Gleeful Glutton"])?;
    assert_eq!(
        counters_on(
            &driver,
            "Korvold, Gleeful Glutton",
            &CounterKind::PlusOnePlusOne
        ),
        types
    );
    let now = driver
        .state()
        .zone(mtg_engine::zones::Zone::Hand(pid(0)))
        .len();
    assert_eq!(now - hand, types as usize);
    Ok(())
}

/// A creature for {4}{R} with plot {3}{R}.
fn plotter() -> Arc<CardDefinition> {
    keyworded_creature(
        "Plotter",
        4,
        &[Color::Red],
        3,
        3,
        &[Keyword::Plot(Cost::mana("{3}{R}".parse().unwrap()))],
    )
}

fn plottable(driver: &Driver) -> bool {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        return false;
    };
    options
        .plottable
        .iter()
        .any(|id| crate::common::name_of(driver.state(), *id) == "Plotter")
}

#[rstest]
fn r6018_doc_aurlock_takes_two_off_plotting(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] docs: usize,
    #[values(1, 2, 4)] mountains: usize,
    #[values(true, false)] doc_is_mine: bool,
) -> Result<(), ScriptError> {
    let mut mine: Vec<_> = (0..mountains).map(|_| ready(mountain())).collect();
    let docs_cards: Vec<_> = (0..docs)
        .map(|_| ready(card("Doc Aurlock, Grizzled Genius").unwrap()))
        .collect();
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [plotter()]);
    if doc_is_mine {
        mine.extend(docs_cards);
    } else {
        scenario = scenario.battlefield(1, docs_cards);
    }
    scenario = scenario.battlefield(0, mine);
    let driver = Driver::from_scenario(scenario)?;
    let less = if doc_is_mine { 2 * docs } else { 0 };
    let cost = 4usize.saturating_sub(less).max(1);
    assert_eq!(plottable(&driver), mountains >= cost);
    Ok(())
}

/// A vanilla creature named Vecna.
fn vecna() -> Arc<CardDefinition> {
    free_creature("Vecna", 2, 2)
}

#[rstest]
fn r6018_hand_of_vecna_pumps_the_creature_chosen(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 3)] in_hand: usize,
    #[values(true, false)] equipped: bool,
    #[values(true, false)] has_vecna: bool,
) -> Result<(), ScriptError> {
    let hand = ready(card("Hand of Vecna").unwrap());
    let mut mine = vec![
        ready(free_creature("Bear", 2, 2)),
        if equipped {
            attached_to(hand, 0, 0)
        } else {
            hand
        },
    ];
    if has_vecna {
        mine.push(ready(vecna()));
    }
    // An opponent's Vecna is not "a creature you control named Vecna".
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, vec![forest(); in_hand])
        .battlefield(0, mine)
        .battlefield(1, [ready(vecna())]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::BeginCombat)?;
    for _ in 0..20 {
        if driver.state().turn().step() != Step::BeginCombat {
            break;
        }
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let x = i32::try_from(in_hand).unwrap();
    let bonus = |id| {
        let chars = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
        (chars.power.unwrap() - 2, chars.toughness.unwrap() - 2)
    };
    let bear = bonus(driver.id_of("Bear")?);
    let vecnas: Vec<_> = driver
        .state()
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| crate::common::name_of(driver.state(), *id) == "Vecna")
        .map(|id| {
            (
                mtg_engine::characteristics::controller_of(driver.state(), id),
                bonus(id),
            )
        })
        .collect();
    let theirs = vecnas
        .iter()
        .find(|(owner, _)| *owner == Some(pid(1)))
        .unwrap()
        .1;
    assert_eq!(theirs, (0, 0));
    let my_vecna = vecnas
        .iter()
        .find(|(owner, _)| *owner == Some(pid(0)))
        .map(|(_, b)| *b);
    let total = bear.0 + my_vecna.map_or(0, |b| b.0);
    let candidates = usize::from(equipped) + usize::from(has_vecna);
    assert_eq!(total, if candidates > 0 { x } else { 0 });
    if !equipped {
        assert_eq!(bear, (0, 0));
    }
    Ok(())
}

#[rstest]
fn r6018_strixhaven_stadium_tenth_point_makes_that_player_lose(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 8, 9)] points: u32,
) -> Result<(), ScriptError> {
    let point = CounterKind::Other("point".into());
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            with_counters(
                ready(card("Strixhaven Stadium").unwrap()),
                point.clone(),
                points,
            ),
            ready(free_creature("Hitter", 1, 1)),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    r6018_hit(&mut driver, &["Hitter"])?;
    let lost = points + 1 >= 10;
    assert_eq!(
        counters_on(&driver, "Strixhaven Stadium", &point),
        if lost { 0 } else { points + 1 }
    );
    assert_eq!(driver.state().is_alive(pid(1)), !lost);
    Ok(())
}

/// Answers passively until the turn is at `step` of the `combat`th combat with an empty stack, or at the second main
/// phase.
fn r6018_until(driver: &mut Driver, step: Step, combat: u32) -> Result<(), ScriptError> {
    for _ in 0..400 {
        let state = driver.state();
        let here = state.turn().step() == step && state.turn().combats() == combat;
        if here || state.turn().step() == Step::Main2 {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the combat".into()))
}

fn tapped(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .is_some_and(mtg_engine::object::PermanentState::tapped)
}

#[rstest]
fn r6018_raphael_untaps_the_attackers_and_adds_one_combat_a_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    const RAPHAEL: &str = "Raphael, Tag Team Tough";
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(card(RAPHAEL).unwrap()),
            ready(free_creature("Buddy", 1, 1)),
            ready(free_creature("Homebody", 1, 1)),
        ],
    );
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let before = driver.state().player(pid(1)).unwrap().life();
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[(RAPHAEL, 1), ("Buddy", 1)])?;
    r6018_until(&mut driver, Step::DeclareAttackers, 2)?;
    // The second combat begins with the attackers untapped.
    assert_eq!(driver.state().turn().combats(), 2);
    assert!(!tapped(&driver, RAPHAEL));
    assert!(!tapped(&driver, "Buddy"));
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), before - 6);
    driver.attack(&[(RAPHAEL, 1)])?;
    r6018_until(&mut driver, Step::Main2, 3)?;
    // The second hit is not the first this turn: no third combat, and Raphael stays tapped.
    assert_eq!(driver.state().turn().step(), Step::Main2);
    assert_eq!(driver.state().turn().combats(), 2);
    assert!(tapped(&driver, RAPHAEL));
    let replayed = mtg_engine::replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
fn r6018_agents_toolkit_enters_with_four_kinds_of_counters(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    const TOOLKIT: &str = "Agent's Toolkit";
    let mut lands = vec![ready(forest()), ready(forest())];
    lands.push(ready(mtg_testkit::cards::island()));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card(TOOLKIT).unwrap()])
        .battlefield(0, lands);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast(TOOLKIT)?;
    for _ in 0..40 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    for kind in [
        CounterKind::PlusOnePlusOne,
        CounterKind::Other("flying".into()),
        CounterKind::Other("deathtouch".into()),
        CounterKind::Other("shield".into()),
    ] {
        assert_eq!(counters_on(&driver, TOOLKIT, &kind), 1, "{kind:?}");
    }
    Ok(())
}

/// The rulings of the Conspiracy cards: a game played without a draft has no notes, so no creature card was removed from
/// the draft with cards named Animus of Predation and it has none of the listed keywords.
#[rstest]
fn r6018_animus_of_predation_has_no_keyword_without_a_draft(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    const ANIMUS: &str = "Animus of Predation";
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card(ANIMUS).unwrap())])
        .graveyard(
            0,
            [keyworded_creature(
                "Flier",
                0,
                &[],
                1,
                1,
                &[Keyword::Flying],
            )],
        );
    let driver = Driver::from_scenario(scenario)?;
    let id = driver.id_of(ANIMUS)?;
    for keyword in [
        Keyword::Flying,
        Keyword::FirstStrike,
        Keyword::Deathtouch,
        Keyword::Haste,
        Keyword::Menace,
        Keyword::Vigilance,
    ] {
        assert!(!mtg_engine::keywords::has_keyword(
            driver.state(),
            id,
            &keyword
        ));
    }
    Ok(())
}

/// O-Kagachi Made Manifest, the back face of The Kami War, as a card of its own.
fn o_kagachi() -> Arc<CardDefinition> {
    let saga = card("The Kami War // O-Kagachi Made Manifest").unwrap();
    Arc::new(saga.faces[1].clone())
}

#[rstest]
#[case::a_creature_card(vec![mtg_testkit::cards::creature("Ogre", 4, &[Color::Red], 3, 3)], 5)]
#[case::the_defender_picks_one_of_two(
    vec![
        mtg_testkit::cards::creature("Ogre", 4, &[Color::Red], 3, 3),
        mtg_testkit::cards::creature("Imp", 1, &[Color::Red], 1, 1),
    ],
    -1
)]
#[case::lands_are_not_chosen(vec![forest()], 0)]
#[case::nothing(vec![], 0)]
fn r6018_o_kagachi_returns_the_card_the_defender_chooses(
    #[values(2, 3, 4)] seats: usize,
    #[case] graveyard: Vec<Arc<CardDefinition>>,
    #[case] mana_value: i32,
) -> Result<(), ScriptError> {
    const NAME: &str = "O-Kagachi Made Manifest";
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(o_kagachi())])
        .graveyard(0, graveyard.clone());
    let mut driver = Driver::from_scenario(scenario)?;
    let base = mtg_engine::characteristics::characteristics(driver.state(), driver.id_of(NAME)?)
        .unwrap()
        .power
        .unwrap();
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[(NAME, 1)])?;
    let mut chooser = None;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if driver.state().turn().step() != Step::DeclareAttackers {
            break;
        }
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::ChooseCards(_) => {
                chooser = Some(decision.player);
                driver.answer_passively()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let power = mtg_engine::characteristics::characteristics(driver.state(), driver.id_of(NAME)?)
        .unwrap()
        .power
        .unwrap();
    let hand = driver
        .state()
        .zone(mtg_engine::zones::Zone::Hand(pid(0)))
        .to_vec();
    assert_eq!(hand.len(), usize::from(mana_value != 0));
    if mana_value < 0 {
        // The defending player chooses between the two, and the bonus is the mana value of the one returned.
        assert_eq!(chooser, Some(pid(1)));
        let returned = crate::common::name_of(driver.state(), hand[0]);
        assert_eq!(power - base, if returned == "Ogre" { 5 } else { 2 });
    } else {
        assert_eq!(power - base, mana_value);
    }
    Ok(())
}
