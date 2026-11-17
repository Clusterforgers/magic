//! Goal: an Aura spell announces what it will enchant as it is cast (CR 303.4a, 601.2c) from hand, from face-down exile (a card Gonti's trigger hid) and from face-up exile (Nashi), paid automatically or by hand, at tables of two to four seats: the target decision comes before the payment and offers the host, the cast carries the target and the Aura resolves attached to it; an Aura with nothing to enchant cannot be cast.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::Event;
use mtg_engine::choose::{Target, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::attach_cards::canny_thief;
use mtg_testkit::cards::{bear, forest, free_creature, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Where the Aura is cast from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum From {
    Hand,
    /// Exiled face down from an opponent's library by a Canny Thief (Gonti's trigger).
    Hidden,
    /// Exiled face up from an opponent's library by Nashi, cast for life.
    Nashi,
}

/// What each Aura enchants here and the lands that pay for it by hand.
fn plan(aura: &str) -> (&'static str, &'static [&'static str]) {
    match aura {
        "Control Magic" => ("Ogre", &["Island", "Island", "Island", "Island"]),
        "Pacifism" => ("Ogre", &["Plains", "Plains"]),
        "Rancor" => ("Grizzly Bears", &["Forest"]),
        "Animate Dead" => ("Dead Bear", &["Swamp", "Swamp"]),
        "Curse of Chaos" => ("seat 1", &["Mountain", "Mountain", "Mountain"]),
        other => panic!("no plan for {other}"),
    }
}

/// Answers until a player has priority with an empty stack: yes to each question, the first legal targets, an automatic
/// payment and the rest passively.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..64 {
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) if driver.state().zone(Zone::Stack).is_empty() => {
                return Ok(());
            }
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(true)),
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::ChooseTargets(options) => Answer::ChooseTargets(TargetChoice::Targets(
                options.legal.iter().copied().take(1).collect(),
            )),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the stack does not empty".into()))
}

/// The newest object of that name in any zone, as a target, or seat 1 for "seat 1".
fn host(driver: &Driver, name: &str) -> Target {
    if name == "seat 1" {
        return driver.target_player(1);
    }
    let object = driver
        .state()
        .objects()
        .filter(|object| object.card().name == name)
        .max_by_key(|object| object.id())
        .unwrap();
    Target::Object(object.reference())
}

/// Seat 0's postcombat main phase with `aura` castable from `from`: seat 0 has a Grizzly Bears and four of each basic
/// land, seat 1 an Ogre and a creature card in its graveyard (none with `bare`). For an exile cast seat 0's Canny Thief
/// or Nashi has hit seat 1, whose top card was the Aura.
fn table(aura: &str, from: From, seats: usize, bare: bool) -> Result<Driver, ScriptError> {
    let mut mine = vec![ready(bear())];
    for land in [island(), plains(), forest(), swamp(), mountain()] {
        mine.extend((0..4).map(|_| ready(land.clone())));
    }
    match from {
        From::Hand => {}
        From::Hidden => mine.push(ready(canny_thief("Canny Thief").unwrap())),
        From::Nashi => mine.push(ready(card("Nashi, Moon Sage's Scion").unwrap())),
    }
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1);
    if !bare {
        scenario = scenario
            .battlefield(1, [ready(free_creature("Ogre", 3, 3))])
            .graveyard(1, [free_creature("Dead Bear", 2, 2)]);
    }
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![island(), island()]);
    }
    if from == From::Hand {
        scenario = scenario.hand(0, [card(aura).unwrap()]);
    } else {
        scenario = scenario.library(1, vec![card(aura).unwrap(), island()]);
    }
    let mut driver = Driver::from_scenario(scenario.battlefield(0, mine))?;
    let attacker = match from {
        From::Hand => return Ok(driver),
        From::Hidden => "Canny Thief",
        From::Nashi => "Nashi, Moon Sage's Scion",
    };
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[(attacker, 1)])?;
    driver.idle_until(1, Step::Main2)?;
    settle(&mut driver)?;
    Ok(driver)
}

#[rstest]
fn an_aura_announces_its_host_as_it_is_cast(
    #[values(
        "Control Magic",
        "Pacifism",
        "Rancor",
        "Animate Dead",
        "Curse of Chaos"
    )]
    aura: &str,
    #[values(From::Hand, From::Hidden, From::Nashi)] from: From,
    #[values(false, true)] by_hand: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Nashi's spell is paid with life, not mana: there is nothing to tap by hand.
    if from == From::Nashi && by_hand {
        return Ok(());
    }
    let (host_name, lands) = plan(aura);
    let mut driver = table(aura, from, seats, false)?;
    let wanted = host(&driver, host_name);
    driver.begin_cast(aura)?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!(
            "{aura} from {from:?} asks no target: {:?}",
            driver.decision()?.kind
        );
    };
    assert!(options.legal.contains(&wanted), "{aura}: {options:?}");
    assert_eq!((options.min, options.max), (1, 1));
    driver.choose_targets(&[wanted])?;
    let events = if by_hand {
        driver.pay_tapping(lands, 0)?
    } else {
        driver.answer(Answer::PayMana(PayChoice::Auto))?
    };
    let announced = events.iter().find_map(|event| match event {
        Event::SpellCast { choices, .. } => Some(choices.targets.clone()),
        _ => None,
    });
    assert_eq!(announced, Some(vec![vec![wanted]]), "{aura} from {from:?}");
    settle(&mut driver)?;
    let state = driver.state();
    let permanent = state
        .object(driver.id_of(aura)?)
        .and_then(|object| object.permanent().cloned())
        .unwrap();
    match wanted {
        Target::Player(player) => assert_eq!(permanent.attached_player(), Some(player)),
        Target::Object(object) => {
            assert_eq!(permanent.attached_to().map(|r| r.id), Some(object.id));
        }
    }
    if aura == "Control Magic" {
        assert_eq!(
            mtg_engine::characteristics::controller_of(state, driver.id_of("Ogre")?),
            Some(pid(0))
        );
    }
    Ok(())
}

#[rstest]
#[case::control_magic("Control Magic", From::Hand)]
#[case::pacifism("Pacifism", From::Hand)]
#[case::rancor("Rancor", From::Hand)]
#[case::animate_dead("Animate Dead", From::Hand)]
#[case::animate_dead_hidden("Animate Dead", From::Hidden)]
#[case::animate_dead_nashi("Animate Dead", From::Nashi)]
fn an_aura_with_nothing_to_enchant_cannot_be_cast(
    #[case] aura: &str,
    #[case] from: From,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = if from == From::Hand {
        // No creature on the battlefield, none in a graveyard.
        let mut lands = Vec::new();
        for land in [island(), plains(), forest(), swamp()] {
            lands.extend((0..4).map(|_| ready(land.clone())));
        }
        Driver::from_scenario(
            Scenario::new(seats)
                .at(1, 0, Step::Main1)
                .hand(0, [card(aura).unwrap()])
                .battlefield(0, lands),
        )?
    } else {
        table(aura, from, seats, true)?
    };
    assert!(driver.begin_cast(aura).is_err(), "{aura} from {from:?}");
    assert_eq!(driver.state().zone(Zone::Stack), []);
    Ok(())
}
