//! Goal: mayhem is an alternative cost to cast a card from its owner's graveyard that is offered only in the turn its owner discarded it (by a spell or as the cost of an ability), paid with the mayhem cost in place of the mana cost, remembered by the spell, kept to the timing of the card, and never offered for a card that lay in the graveyard before, was discarded in an earlier turn or belongs to another seat; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CastWay;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, PresetPermanent, replay};
use mtg_testkit::cards::{forest, mountain, swamp};
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{lands, pay};
use crate::common::{name_of, names, on_battlefield, pid};

fn card(name: &str) -> Arc<CardDefinition> {
    corpus::card(name).unwrap()
}

/// Every seat has a library of Forests, and seat 1 a Grizzly Bears to aim at.
fn base(seats: usize) -> Scenario {
    (0..seats)
        .fold(Scenario::new(seats), |scenario, seat| {
            scenario.library(seat, (0..8).map(|_| forest()))
        })
        .battlefield(1, vec![ready(card("Grizzly Bears"))])
}

/// How the card is discarded in the first main phase of seat 0.
#[derive(Debug, Clone, Copy)]
enum Cause {
    /// Faithless Looting makes its controller discard two cards.
    Spell,
    /// Rummaging Goblin discards a card as the cost of its ability.
    Cost,
}

/// Settles the stack: priority passes until it is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            || driver.state().stack().is_empty()
        {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

/// Seat 0 holds `name`, has `count` Mountains to cast it with besides what the discard needs, and discards it by `cause`; the
/// stack is empty when this returns.
fn discarded(seats: usize, name: &str, cause: Cause, count: usize) -> Result<Driver, ScriptError> {
    let (hand, battlefield) = match cause {
        Cause::Spell => (vec!["Faithless Looting", name], lands(count + 1, mountain)),
        Cause::Cost => (
            vec![name],
            std::iter::once(ready(card("Rummaging Goblin")))
                .chain(lands(count, mountain))
                .collect(),
        ),
    };
    let mut driver =
        Driver::from_scenario(base(seats).hand_named(0, hand).battlefield(0, battlefield))?;
    match cause {
        Cause::Spell => {
            driver.cast("Faithless Looting")?;
            settle(&mut driver)?;
            let id = in_hand(&driver, name);
            let other = driver
                .state()
                .zone(Zone::Hand(pid(0)))
                .iter()
                .copied()
                .find(|other| *other != id)
                .expect("a second card to discard");
            driver.answer(Answer::ChooseCards(CardsChoice(vec![id, other])))?;
        }
        Cause::Cost => {
            driver.begin_activation("Rummaging Goblin")?;
        }
    }
    settle(&mut driver)?;
    Ok(driver)
}

/// The card called `name` in seat 0's hand.
fn in_hand(driver: &Driver, name: &str) -> mtg_core::ids::ObjectId {
    driver
        .state()
        .zone(Zone::Hand(pid(0)))
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == name)
        .expect("the card is in hand")
}

/// The way of every spell cast so far.
fn cast_ways(driver: &Driver) -> Vec<Option<CastWay>> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::SpellCast { choices, .. } => Some(choices.way),
            _ => None,
        })
        .collect()
}

/// Casts `name` as the holder of priority: the targets, if any are asked, are the Grizzly Bears.
fn cast(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_cast(name)?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
        driver.choose_targets(&[driver.target_named("Grizzly Bears")?])?;
    }
    pay(driver)?;
    settle(driver)
}

/// The log replays to the state the game is in.
fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
fn a_card_discarded_this_turn_is_cast_from_the_graveyard_for_its_mayhem_cost(
    #[values(Cause::Spell, Cause::Cost)] cause: Cause,
    #[values(2, 3, 4)] seats: usize,
    #[values("Electro's Bolt", "Spider-Islanders")] name: &str,
) -> Result<(), ScriptError> {
    // Two Mountains pay {1}{R}; the printed cost of either card is out of reach.
    let mut driver = discarded(seats, name, cause, 2)?;
    assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&name.to_owned()));
    cast(&mut driver, name)?;
    assert_eq!(cast_ways(&driver).last(), Some(&Some(CastWay::Mayhem)));
    if name == "Electro's Bolt" {
        assert!(on_battlefield(driver.state(), "Grizzly Bears").is_none());
        assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&name.to_owned()));
    } else {
        assert!(on_battlefield(driver.state(), name).is_some());
        assert!(!names(driver.state(), Zone::Graveyard(pid(0))).contains(&name.to_owned()));
    }
    assert_replays(&driver);
    Ok(())
}

/// Why a card lies in the graveyard without having been discarded this turn.
#[derive(Debug, Clone, Copy)]
enum Past {
    /// It was there when the game began.
    LayThere,
    /// Its owner discarded it in their first turn, and the game went on to their next.
    EarlierTurn,
}

#[rstest]
fn a_card_that_was_not_discarded_this_turn_is_not_cast_for_any_cost(
    #[values(Past::LayThere, Past::EarlierTurn)] past: Past,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let name = "Electro's Bolt";
    let mut driver = match past {
        Past::LayThere => Driver::from_scenario(
            base(seats)
                .graveyard_named(0, [name])
                .battlefield(0, lands(4, mountain)),
        )?,
        Past::EarlierTurn => {
            let mut driver = discarded(seats, name, Cause::Cost, 4)?;
            driver.idle_until(1 + u32::try_from(seats).unwrap(), Step::Main1)?;
            driver
        }
    };
    assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&name.to_owned()));
    assert!(driver.begin_cast(name).is_err(), "{past:?}");
    Ok(())
}

#[rstest]
#[case::an_instant("Sadistic Slash", true)]
#[case::a_creature_with_flash("Swarm, Being of Bees", true)]
#[case::a_sorcery("Electro's Bolt", false)]
#[case::a_creature("Spider-Islanders", false)]
fn mayhem_keeps_the_timing_of_the_card(
    #[case] name: &str,
    #[case] castable: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // In the turn of seat 1, seat 0 discards the card at instant speed and has the ability on the stack.
    let mut permanents = vec![ready(card("Rummaging Goblin"))];
    permanents.extend(lands(2, swamp));
    permanents.extend(lands(2, mountain));
    let mut driver = Driver::from_scenario(
        base(seats)
            .at(1, 1, Step::Main1)
            .hand_named(0, [name])
            .battlefield(0, permanents),
    )?;
    for _ in 1..seats {
        driver.pass()?;
    }
    driver.begin_activation("Rummaging Goblin")?;
    assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&name.to_owned()));
    if castable {
        cast(&mut driver, name)?;
        assert_eq!(cast_ways(&driver), [Some(CastWay::Mayhem)]);
        assert_replays(&driver);
    } else {
        assert!(driver.begin_cast(name).is_err());
    }
    Ok(())
}

#[rstest]
fn only_the_seat_that_discarded_the_card_may_cast_it(
    #[values(true, false)] owner_acts: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Seat 1 holds the instant and discards it in its own turn; every seat has the Swamps to pay for it.
    let mut driver = Driver::from_scenario(
        (2..seats).fold(
            base(seats)
                .at(1, 1, Step::Main1)
                .hand_named(1, ["Sadistic Slash"])
                .battlefield(
                    1,
                    std::iter::once(ready(card("Rummaging Goblin")))
                        .chain(std::iter::once(ready(card("Grizzly Bears"))))
                        .chain(lands(2, swamp))
                        .collect::<Vec<PresetPermanent>>(),
                )
                .battlefield(0, lands(2, swamp)),
            |scenario, seat| scenario.battlefield(seat, lands(2, swamp)),
        ),
    )?;
    driver.begin_activation("Rummaging Goblin")?;
    assert!(names(driver.state(), Zone::Graveyard(pid(1))).contains(&"Sadistic Slash".to_owned()));
    if !owner_acts {
        driver.pass()?;
    }
    assert_eq!(driver.begin_cast("Sadistic Slash").is_ok(), owner_acts);
    Ok(())
}

#[rstest]
fn the_discard_of_a_whole_hand_makes_every_such_card_castable_once_each(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Faithless Looting discards two cards of seat 0 at once; with the mana for one mayhem cost, the first cast spends it.
    let mut driver = Driver::from_scenario(
        base(seats)
            .hand_named(
                0,
                ["Faithless Looting", "Spider-Islanders", "Electro's Bolt"],
            )
            .battlefield(0, lands(3, mountain)),
    )?;
    driver.cast("Faithless Looting")?;
    settle(&mut driver)?;
    let both: Vec<_> = ["Spider-Islanders", "Electro's Bolt"]
        .iter()
        .map(|name| in_hand(&driver, name))
        .collect();
    driver.answer(Answer::ChooseCards(CardsChoice(both)))?;
    settle(&mut driver)?;
    let graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    assert!(graveyard.contains(&"Spider-Islanders".to_owned()));
    assert!(graveyard.contains(&"Electro's Bolt".to_owned()));
    cast(&mut driver, "Spider-Islanders")?;
    assert!(driver.begin_cast("Electro's Bolt").is_err(), "no mana left");
    assert_eq!(cast_ways(&driver).last(), Some(&Some(CastWay::Mayhem)));
    Ok(())
}
