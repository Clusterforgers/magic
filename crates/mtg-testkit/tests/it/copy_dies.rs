//! Goal: a permanent that dies as it enters, or later, triggers from the abilities it had when it left, with and without a copy effect, at tables of two, three and four seats: a creature that is toughness 0 after the layers (an opponent's static ability) is put into the graveyard by state-based actions before any trigger goes on the stack, and its "when this dies" and "whenever a creature you control dies" abilities are read from the card it was (a plain card), from the creature a Clone copied and from the creature a Phantasmal Image copied, never from the card that lies in the graveyard; the same holds for a copy that is killed after it entered, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::{lands, life, run};
use common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::gain_life;
use mtg_testkit::corpus::card;
use mtg_testkit::trigger_cards::{artifact_with, sized_creature_with, trigger, when};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;
use std::sync::Arc;

/// The life the dies trigger gains.
const GAIN: i32 = 2;

/// What dies and how its trigger reads: the creature that dies with the trigger `when` writes, and is copied.
#[derive(Debug, Clone, Copy)]
enum Dying {
    /// "When this creature dies, you gain 2 life."
    ThisDies,
    /// "Whenever a creature you control dies, you gain 2 life."
    YourCreatureDies,
}

impl Dying {
    fn creature(self) -> Arc<CardDefinition> {
        let when = match self {
            Self::ThisDies => when::this_dies(),
            Self::YourCreatureDies => when::your_creature_dies(),
        };
        sized_creature_with(
            "Grave Sentinel",
            1,
            3,
            vec![trigger(when, vec![gain_life(GAIN)])],
        )
    }
}

/// `{2}` artifact: creatures the opponents of its controller control get -0/-3.
fn withering_banner() -> Arc<CardDefinition> {
    let json = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]},"mods":[{"ModifyPT":{"power":0,"toughness":-3}}]}}"#;
    artifact_with(
        "Withering Banner",
        vec![serde_json::from_str::<Ability>(json).unwrap()],
    )
}

/// Which card the dying permanent is, and so how it comes to have the trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Way {
    /// The creature itself is cast: the trigger is printed on its card.
    Plain,
    /// Clone copies the creature of seat 1.
    Clone,
    /// Phantasmal Image copies it (a copy with an exception of its own).
    PhantasmalImage,
}

impl Way {
    const fn cast(self) -> &'static str {
        match self {
            Self::Plain => "Grave Sentinel",
            Self::Clone => "Clone",
            Self::PhantasmalImage => "Phantasmal Image",
        }
    }

    const fn taps(self) -> &'static [&'static str] {
        match self {
            Self::Plain => &[],
            Self::Clone => &["Island", "Island", "Island", "Island"],
            Self::PhantasmalImage => &["Island", "Island"],
        }
    }

    /// The card that lies in the graveyard once the permanent has died.
    const fn card(self) -> &'static str {
        self.cast()
    }
}

/// Seat 0 holds the card for `way` over four Islands and a Mountain; seat 1 controls the creature to copy and, when
/// `banner`, the banner that makes seat 0's creatures toughness 0.
fn game(seats: usize, way: Way, dying: Dying, banner: bool) -> Scenario {
    let hand = match way {
        Way::Plain => dying.creature(),
        Way::Clone | Way::PhantasmalImage => card(way.cast()).unwrap(),
    };
    let foe = [ready(dying.creature())]
        .into_iter()
        .chain(banner.then(|| ready(withering_banner())));
    Scenario::new(seats)
        .hand(0, [hand])
        .battlefield(0, lands(4, 1))
        .battlefield(1, foe)
}

/// Casts the card and, for a copy, answers the question of what to copy with the creature of seat 1; returns once
/// nothing is left on the stack.
fn play_in(driver: &mut Driver, way: Way, seats: usize) -> Result<(), ScriptError> {
    driver.cast_tapping(way.cast(), way.taps())?;
    for _ in 0..50 {
        match driver.engine().decision().map(|d| d.kind.clone()) {
            Some(DecisionKind::ChooseCards(options)) => {
                assert_ne!(way, Way::Plain, "only a copy asks what to copy");
                assert_eq!(driver.decision()?.player, pid(0));
                driver.answer(Answer::ChooseCards(CardsChoice(options.from)))?;
            }
            _ if driver.state().stack().is_empty() && driver.state().resolving().is_none() => {
                return Ok(());
            }
            Some(DecisionKind::Priority(_)) => run(driver, seats)?,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the cast".into()))
}

/// The permanents named `name` that `seat` controls.
fn controlled(driver: &Driver, seat: usize, name: &str) -> Vec<ObjectId> {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            let object = driver.state().object(*id).unwrap();
            object.controller() == pid(seat) && object.card().name == name
        })
        .collect()
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

#[rstest]
fn a_permanent_that_dies_as_it_enters_triggers_from_the_abilities_it_had(
    #[values(2, 3, 4)] seats: usize,
    #[values(Way::Plain, Way::Clone, Way::PhantasmalImage)] way: Way,
    #[values(Dying::ThisDies, Dying::YourCreatureDies)] dying: Dying,
) -> Result<(), ScriptError> {
    let setup = game(seats, way, dying, true).setup().unwrap();
    let mut driver = Driver::from_scenario(game(seats, way, dying, true))?;
    let before = (life(&driver, 0), life(&driver, 1));
    play_in(&mut driver, way, seats)?;
    assert_eq!(
        (life(&driver, 0), life(&driver, 1)),
        (before.0 + GAIN, before.1),
        "the trigger of the permanent that died resolved once, for its controller"
    );
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        [way.card()],
        "the card is in its owner's graveyard"
    );
    assert_eq!(
        controlled(&driver, 0, "Grave Sentinel").len(),
        0,
        "nothing of seat 0 is left on the battlefield"
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_copy_that_is_destroyed_after_it_entered_triggers_from_the_abilities_it_had(
    #[values(2, 3, 4)] seats: usize,
    #[values(Way::Clone, Way::PhantasmalImage)] way: Way,
    #[values(Dying::ThisDies, Dying::YourCreatureDies)] dying: Dying,
) -> Result<(), ScriptError> {
    let scenario = || {
        game(seats, way, dying, false).hand(
            0,
            [card(way.cast()).unwrap(), card("Lightning Bolt").unwrap()],
        )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    play_in(&mut driver, way, seats)?;
    let copy = controlled(&driver, 0, "Grave Sentinel");
    assert_eq!(copy.len(), 1, "the copy entered and stayed");
    let before = life(&driver, 0);
    let target = Target::Object(driver.state().object(copy[0]).unwrap().reference());
    driver.cast_at("Lightning Bolt", &[target])?;
    run(&mut driver, seats)?;
    assert_eq!(life(&driver, 0), before + GAIN);
    assert!(controlled(&driver, 0, "Grave Sentinel").is_empty());
    replays(&driver, &setup);
    Ok(())
}
