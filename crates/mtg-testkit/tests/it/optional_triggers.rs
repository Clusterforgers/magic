//! Goal: a triggered ability that says "you may pay {N}. If you do, ..." (read from Oracle text as a `Pay` in the trigger) plays as the rules say at tables of two, three and four seats for each kind of event that triggers it (casting a spell, a land entering, a creature dying, combat damage to a player, and a spell cast with a target chosen before the payment): its controller alone is asked, a yes pays and does the effect, a no pays nothing and does nothing, a controller who cannot pay is asked with No the only answer (a Yes is refused and changes nothing), and the log replays to the state the game is in.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{names, pid};
use mtg_cards::ability::Ability;
use mtg_cards::compile::compile;
use mtg_cards::effect::Effect;
use mtg_cards::support::{PrintedCard, PrintedFace};
use mtg_cards::value::CounterKind;
use mtg_engine::PresetPermanent;
use mtg_engine::characteristics::controller_of;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{EngineError, replay};
use mtg_testkit::cards::{forest, island, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::trigger_cards::creature_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// What makes the Watcher's trigger fire, and the Oracle line of the Watcher.
#[derive(Clone, Copy, Debug)]
enum Shape {
    /// Casting an instant: draw a card for {1}.
    Cast,
    /// A land entering: draw a card for {2}.
    Landfall,
    /// An opponent's creature dying: a Spirit token for {1}.
    Dies,
    /// Combat damage to a player: six counters for {2}{G}.
    Combat,
    /// Casting an instant: 2 damage to a target chosen when the trigger is put on the stack, for {2}.
    Targeted,
    /// Becoming tapped by attacking: a permanent returns to its owner's hand for {1}{U}.
    Taps,
    /// Combat damage to a player: up to two target lands are destroyed for {2}{R}.
    Destroys,
}

impl Shape {
    fn text(self) -> &'static str {
        match self {
            Shape::Cast => {
                "Whenever you cast an instant spell, you may pay {1}. If you do, draw a card."
            }
            Shape::Landfall => {
                "Landfall \u{2014} Whenever a land you control enters, you may pay {2}. If you do, draw a card."
            }
            Shape::Dies => {
                "Whenever a creature an opponent controls dies, you may pay {1}. If you do, create a 1/1 white Spirit creature token with flying."
            }
            Shape::Combat => {
                "Whenever ~ deals combat damage to a player, you may pay {2}{G}. If you do, put six +1/+1 counters on ~."
            }
            Shape::Targeted => {
                "Whenever you cast an instant spell, you may pay {2}. If you do, ~ deals 2 damage to any target."
            }
            Shape::Taps => {
                "Whenever ~ becomes tapped, you may pay {1}{U}. If you do, return target permanent to its owner's hand."
            }
            Shape::Destroys => {
                "Whenever ~ deals combat damage to a player, you may pay {2}{R}. If you do, destroy up to two target lands."
            }
        }
    }

    /// The question the payment asks.
    fn question(self) -> &'static str {
        match self {
            Shape::Cast | Shape::Dies => "Pay {1}?",
            Shape::Landfall | Shape::Targeted => "Pay {2}?",
            Shape::Combat => "Pay {2}{G}?",
            Shape::Taps => "Pay {1}{U}?",
            Shape::Destroys => "Pay {2}{R}?",
        }
    }

    /// The Watcher as the compiler reads its line.
    fn rules(self) -> Ability {
        let face = PrintedFace {
            name: "Watcher".to_owned(),
            type_line: "Creature \u{2014} Elf".to_owned(),
            oracle_text: self.text().to_owned(),
            power: Some("1".to_owned()),
            toughness: Some("1".to_owned()),
            ..PrintedFace::default()
        };
        let printed = PrintedCard {
            oracle_id: "12345678-1234-1234-1234-123456789abd".to_owned(),
            name: "Watcher".to_owned(),
            faces: vec![face],
            ..PrintedCard::default()
        };
        let compiled = compile(&printed).card.expect("the card compiles");
        let ability = compiled.faces[0].abilities[0].clone();
        assert!(
            matches!(&ability, Ability::Triggered { effects, .. } if matches!(effects.as_slice(), [Effect::Pay { .. }])),
            "{}: {ability:?}",
            self.text()
        );
        ability
    }

    /// The lands seat 0 has: the one that casts or supplies the color, and Islands enough to pay the price, or one short.
    fn lands(self, payable: bool) -> Vec<PresetPermanent> {
        let (first, islands) = match self {
            Shape::Cast | Shape::Dies => (Some(mountain()), 1),
            Shape::Targeted | Shape::Destroys => (Some(mountain()), 2),
            Shape::Landfall => (None, 1),
            Shape::Combat => (Some(forest()), 2),
            Shape::Taps => (None, 2),
        };
        let islands = if payable { islands } else { islands - 1 };
        first
            .into_iter()
            .chain((0..islands).map(|_| island()))
            .map(ready)
            .collect()
    }

    /// What seat 0 holds in hand.
    fn hand(self) -> Vec<std::sync::Arc<mtg_cards::CardDefinition>> {
        match self {
            Shape::Cast | Shape::Dies | Shape::Targeted => vec![card("Lightning Bolt").unwrap()],
            Shape::Landfall => vec![forest()],
            Shape::Combat | Shape::Taps | Shape::Destroys => Vec::new(),
        }
    }

    /// The number the effect changes, as the table reads it.
    fn measure(self, driver: &Driver) -> i64 {
        let state = driver.state();
        match self {
            Shape::Cast | Shape::Landfall => count(state.zone(Zone::Hand(pid(0))).len()),
            Shape::Dies => count(
                names(state, Zone::Battlefield)
                    .iter()
                    .filter(|name| *name == "Spirit")
                    .count(),
            ),
            Shape::Combat => {
                let id = driver.id_of("Watcher").unwrap();
                i64::from(
                    state
                        .object(id)
                        .and_then(|object| object.permanent())
                        .map_or(0, |permanent| {
                            permanent.counters_of(&CounterKind::PlusOnePlusOne)
                        }),
                )
            }
            Shape::Targeted => i64::from(state.player(pid(1)).unwrap().life()),
            Shape::Taps => count(state.zone(Zone::Hand(pid(1))).len()),
            Shape::Destroys => count(
                names(state, Zone::Battlefield)
                    .iter()
                    .filter(|name| ["Forest", "Plains"].contains(&name.as_str()))
                    .count(),
            ),
        }
    }

    /// The targets the trigger is given when it is put on the stack.
    fn targets(self, driver: &Driver) -> Result<Vec<mtg_engine::choose::Target>, ScriptError> {
        Ok(match self {
            Shape::Targeted => vec![driver.target_player(1)],
            Shape::Taps => vec![driver.target_named("Grizzly Bears")?],
            Shape::Destroys => vec![
                driver.target_named("Forest")?,
                driver.target_named("Plains")?,
            ],
            _ => Vec::new(),
        })
    }

    /// What a yes adds to the measure.
    fn gain(self) -> i64 {
        match self {
            Shape::Cast | Shape::Landfall | Shape::Dies | Shape::Taps => 1,
            Shape::Combat => 6,
            Shape::Targeted | Shape::Destroys => -2,
        }
    }
}

fn count(n: usize) -> i64 {
    i64::try_from(n).unwrap()
}

fn table(seats: usize, shape: Shape, payable: bool) -> Result<Driver, ScriptError> {
    let mut board = vec![ready(creature_with("Watcher", vec![shape.rules()]))];
    board.extend(shape.lands(payable));
    let mut scenario = Scenario::new(seats)
        .hand(0, shape.hand())
        .battlefield(0, board);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..8).map(|_| forest()));
    }
    scenario = match shape {
        Shape::Dies | Shape::Taps => scenario.battlefield_named(1, ["Grizzly Bears"]),
        Shape::Destroys => scenario.battlefield(1, [ready(forest()), ready(plains())]),
        _ => scenario,
    };
    Driver::from_scenario(scenario)
}

/// Does what makes the trigger fire.
fn cause(driver: &mut Driver, shape: Shape) -> Result<(), ScriptError> {
    match shape {
        Shape::Cast => {
            let target = driver.target_player(1);
            driver.cast_at("Lightning Bolt", &[target])?;
        }
        Shape::Targeted => {
            // The Bolt is aimed at its caster so that only the trigger changes the life of seat 1.
            let own = driver.target_player(0);
            driver.cast_at("Lightning Bolt", &[own])?;
        }
        Shape::Dies => {
            let bears = driver.target_named("Grizzly Bears")?;
            driver.cast_at("Lightning Bolt", &[bears])?;
        }
        Shape::Landfall => {
            driver.play_land("Forest")?;
        }
        Shape::Combat | Shape::Taps | Shape::Destroys => {
            driver.idle_until(1, Step::DeclareAttackers)?;
            driver.attack(&[("Watcher", 1)])?;
        }
    }
    Ok(())
}

/// Answers passively until the payment is asked; the targets of the trigger are the shape's.
fn to_question(driver: &mut Driver, shape: Shape) -> Result<(), ScriptError> {
    for _ in 0..80 {
        match driver.decision()?.kind {
            DecisionKind::YesNo(_) => return Ok(()),
            DecisionKind::ChooseTargets(_) => {
                let targets = shape.targets(driver)?;
                driver.choose_targets(&targets)?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the payment question".to_owned()))
}

/// Settles whatever is on the stack and in combat.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..80 {
        let quiet = driver.state().stack().is_empty()
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_));
        if quiet {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("an empty stack".to_owned()))
}

fn question(driver: &Driver) -> (usize, String, bool) {
    let decision = driver.decision().unwrap();
    let DecisionKind::YesNo(options) = &decision.kind else {
        panic!(
            "a yes-or-no question was expected, not {}",
            decision.kind.name()
        );
    };
    (
        decision.player.index(),
        options.text.clone(),
        options.only_no,
    )
}

fn untapped_lands(driver: &Driver) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| controller_of(state, **id) == Some(pid(0)))
        .filter_map(|id| state.object(*id))
        .filter(|object| ["Forest", "Island", "Mountain"].contains(&object.card().name.as_str()))
        .filter(|object| {
            object
                .permanent()
                .is_some_and(|permanent| !permanent.tapped())
        })
        .count()
}

fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// A table of `seats` where the trigger has fired and its controller is asked about the payment.
fn asked(seats: usize, shape: Shape, payable: bool) -> Result<Driver, ScriptError> {
    let mut driver = table(seats, shape, payable)?;
    cause(&mut driver, shape)?;
    to_question(&mut driver, shape)?;
    Ok(driver)
}

#[rstest]
fn a_yes_pays_and_does_the_effect(
    #[values(2, 3, 4)] seats: usize,
    #[values(
        Shape::Cast,
        Shape::Landfall,
        Shape::Dies,
        Shape::Combat,
        Shape::Targeted,
        Shape::Taps,
        Shape::Destroys
    )]
    shape: Shape,
) -> Result<(), ScriptError> {
    let mut driver = asked(seats, shape, true)?;
    let (seat, text, only_no) = question(&driver);
    assert_eq!((seat, text.as_str(), only_no), (0, shape.question(), false));
    let (before, lands) = (shape.measure(&driver), untapped_lands(&driver));
    driver.answer(Answer::YesNo(YesNoChoice(true)))?;
    assert!(matches!(driver.decision()?.kind, DecisionKind::PayMana(_)));
    assert_eq!(driver.decision()?.player, pid(0));
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert_eq!(shape.measure(&driver), before + shape.gain());
    assert!(untapped_lands(&driver) < lands, "the price was paid");
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn a_no_pays_nothing_and_does_nothing(
    #[values(2, 3, 4)] seats: usize,
    #[values(
        Shape::Cast,
        Shape::Landfall,
        Shape::Dies,
        Shape::Combat,
        Shape::Targeted,
        Shape::Taps,
        Shape::Destroys
    )]
    shape: Shape,
) -> Result<(), ScriptError> {
    let mut driver = asked(seats, shape, true)?;
    let (before, lands) = (shape.measure(&driver), untapped_lands(&driver));
    driver.answer(Answer::YesNo(YesNoChoice(false)))?;
    assert!(!matches!(driver.decision()?.kind, DecisionKind::PayMana(_)));
    settle(&mut driver)?;
    assert_eq!(shape.measure(&driver), before);
    assert_eq!(untapped_lands(&driver), lands, "nothing was paid");
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn a_controller_who_cannot_pay_is_asked_with_no_the_only_answer(
    #[values(2, 3, 4)] seats: usize,
    #[values(
        Shape::Cast,
        Shape::Landfall,
        Shape::Dies,
        Shape::Combat,
        Shape::Targeted,
        Shape::Taps,
        Shape::Destroys
    )]
    shape: Shape,
) -> Result<(), ScriptError> {
    let mut driver = asked(seats, shape, false)?;
    let (seat, text, only_no) = question(&driver);
    assert_eq!((seat, text.as_str(), only_no), (0, shape.question(), true));
    let (before, lands, log, hash) = (
        shape.measure(&driver),
        untapped_lands(&driver),
        driver.engine().events().len(),
        driver.engine().state_hash(),
    );
    let refused = driver.answer(Answer::YesNo(YesNoChoice(true))).unwrap_err();
    assert!(
        matches!(
            refused,
            ScriptError::Engine(EngineError::Illegal(IllegalAnswer::NotOffered("yes")))
        ),
        "{refused:?}"
    );
    assert_eq!(driver.engine().events().len(), log, "nothing was logged");
    assert_eq!(driver.engine().state_hash(), hash, "nothing changed");
    driver.answer(Answer::YesNo(YesNoChoice(false)))?;
    settle(&mut driver)?;
    assert_eq!(shape.measure(&driver), before);
    assert_eq!(untapped_lands(&driver), lands);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn only_the_controller_of_the_trigger_is_asked(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] answer: bool,
) -> Result<(), ScriptError> {
    let mut driver = asked(seats, Shape::Cast, true)?;
    assert_eq!(driver.decision()?.player, pid(0));
    driver.answer(Answer::YesNo(YesNoChoice(answer)))?;
    if answer {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    for _ in 0..seats {
        assert!(
            !matches!(driver.decision()?.kind, DecisionKind::YesNo(_)),
            "no other seat is asked"
        );
        driver.answer_passively()?;
    }
    Ok(())
}
