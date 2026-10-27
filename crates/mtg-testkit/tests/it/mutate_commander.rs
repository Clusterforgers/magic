//! Goal: a commander card of a merged permanent (CR 702.140, 729.3, 903.9) is offered the command zone alone, whether it is the card the permanent was or the one that mutated onto it, and whether the permanent goes to a graveyard, exile, a hand or the top, bottom or shuffled into a library; a yes puts that card alone into the command zone, a no leaves it where the permanent went, the other card always goes where the permanent goes, two commander cards in one permanent are each asked, the commander is cast from the command zone again with its tax, and the game replays; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{choose_way, pay};
use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::{Destination, Effect, Place};
use mtg_cards::refs::{Pred, Ref, Selector, Targetable};
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::format::Commander;
use mtg_engine::zones::Zone;
use mtg_engine::{PresetPermanent, replay};
use mtg_testkit::cards::{bear, cost, forest, island, one_target, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, check_state, ready};
use rstest::rstest;

const GNARR: &str = "Trumpeting Gnarr";
const BEARS: &str = "Grizzly Bears";
const REMOVAL: &str = "Send Off";

/// Where the removal sends the merged permanent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum To {
    Graveyard,
    Exile,
    Hand,
    Top,
    Bottom,
    Shuffled,
}

impl To {
    fn effect(self) -> Effect {
        let what = Ref::Target(0);
        let library = |place| Effect::Move {
            what: what.clone(),
            to: Destination::Library { place, order: None },
        };
        match self {
            To::Graveyard => Effect::Destroy {
                what,
                regenerate: true,
            },
            To::Exile => Effect::Exile {
                what,
                bind: None,
                duration: None,
            },
            To::Hand => Effect::Move {
                what,
                to: Destination::Hand,
            },
            To::Top => library(Place::Top),
            To::Bottom => library(Place::Bottom),
            To::Shuffled => library(Place::Shuffled),
        }
    }

    fn zone(self) -> Zone {
        match self {
            To::Graveyard => Zone::Graveyard(pid(0)),
            To::Exile => Zone::Exile,
            To::Hand => Zone::Hand(pid(0)),
            To::Top | To::Bottom | To::Shuffled => Zone::Library(pid(0)),
        }
    }
}

/// An instant for `{B}` that sends target creature `to` its destination.
fn removal(to: To) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(REMOVAL)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Black]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(Targetable::Object(Selector(vec![Pred::Type(
                    CardType::Creature,
                )])))],
                effects: vec![to.effect()],
            }),
    )
}

/// Lands for the Bear, the mutate cost of the Gnarr and a second cast of either commander with its tax.
fn lands() -> Vec<PresetPermanent> {
    let mut lands: Vec<PresetPermanent> = (0..8).map(|_| ready(forest())).collect();
    lands.extend((0..6).map(|_| ready(island())));
    lands.push(ready(swamp()));
    lands
}

/// Which cards of the merged permanent are commanders: the Gnarr that mutates, the Bear it mutates onto, or both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Who {
    Spell,
    Host,
    Both,
}

/// A merged permanent of seat 0, the Gnarr mutated over a Bear, with `who` its commanders; seat 0 holds the removal
/// for `to`. Returns the driver, the scenario, the permanent (the Bear's id) and the Gnarr's card.
fn merged(
    seats: usize,
    who: Who,
    to: To,
) -> Result<(Driver, Scenario, ObjectId, ObjectId), ScriptError> {
    let mut board = lands();
    let mut hand = vec![removal(to)];
    let commanders = match who {
        Who::Spell => vec![card(GNARR).unwrap()],
        Who::Host => vec![bear()],
        Who::Both => vec![bear(), card(GNARR).unwrap()],
    };
    if who == Who::Spell {
        board.push(ready(bear()));
    }
    if who == Who::Host {
        hand.push(card(GNARR).unwrap());
    }
    let scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, commanders)
        .battlefield(0, board)
        .battlefield(1, vec![ready(swamp())])
        .hand(0, hand);
    let scenario = (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..8).map(|_| forest()))
    });
    let mut driver = Driver::from_scenario(scenario.clone())?;
    if who != Who::Spell {
        driver.begin_cast(BEARS)?;
        pay(&mut driver)?;
        settle(&mut driver, &[])?;
    }
    let target = driver.target_named(BEARS)?;
    let host = driver.id_of(BEARS)?;
    driver.begin_cast(GNARR)?;
    choose_way(&mut driver, "Mutate")?;
    driver.choose_targets(&[target])?;
    pay(&mut driver)?;
    settle(&mut driver, &[])?;
    let state = driver.state();
    let parts: Vec<ObjectId> = state
        .object(host)
        .unwrap()
        .merged()
        .unwrap()
        .from_the_top()
        .iter()
        .map(|part| part.id())
        .collect();
    let gnarr = parts[0];
    assert_eq!(parts, [gnarr, host]);
    Ok((driver, scenario, host, gnarr))
}

/// Passes and answers passively until the stack is empty and no question is open, answering the command zone question
/// about each card of `answers` with its answer (and the question of a mutating spell with yes); the cards asked about,
/// in order.
fn settle(driver: &mut Driver, answers: &[(ObjectId, bool)]) -> Result<Vec<ObjectId>, ScriptError> {
    let mut asked = Vec::new();
    for _ in 0..300 {
        let decision = driver.decision()?;
        let quiet = driver.state().stack().is_empty() && driver.state().resolving().is_none();
        match &decision.kind {
            DecisionKind::YesNo(options) if options.text.contains("command zone") => {
                let (card, yes) = *answers
                    .iter()
                    .find(|(card, _)| *card == options.source)
                    .unwrap_or_else(|| panic!("asked about another card: {}", options.text));
                assert_eq!(decision.player, pid(0));
                asked.push(card);
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            DecisionKind::Priority(_) if quiet => return Ok(asked),
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Sends the merged permanent `to` its destination with the removal, answering as `answers` says, and checks where each
/// card lies: an accepted commander card in the command zone, every other card where the permanent went, the cards on
/// top or on the bottom together at that end, the invariants and the replay.
fn send_off(
    driver: &mut Driver,
    scenario: &Scenario,
    (host, gnarr): (ObjectId, ObjectId),
    to: To,
    answers: &[(ObjectId, bool)],
) -> Result<(), ScriptError> {
    let name = driver.state().object(host).unwrap().card().name.clone();
    let target = driver.target_named(&name)?;
    driver.cast_at(REMOVAL, &[target])?;
    let mut asked = settle(driver, answers)?;
    asked.sort();
    let mut expected: Vec<ObjectId> = answers.iter().map(|(card, _)| *card).collect();
    expected.sort();
    assert_eq!(asked, expected, "each commander card is asked about once");
    let state = driver.state();
    let mut in_library = 0;
    for card in [host, gnarr] {
        let object = state.object(card).unwrap();
        assert!(object.merged().is_none());
        let accepted = answers.iter().any(|(id, yes)| *id == card && *yes);
        let zone = if accepted {
            Zone::Command(pid(0))
        } else {
            to.zone()
        };
        assert_eq!(object.zone(), zone, "{card:?}");
        in_library += usize::from(zone == Zone::Library(pid(0)));
    }
    // The cards put on top or on the bottom lie together at that end (the last id is the top card).
    let library = state.zone(Zone::Library(pid(0)));
    let ends = match to {
        To::Top => &library[library.len() - in_library..],
        To::Bottom => &library[..in_library],
        _ => &[][..],
    };
    assert!(ends.iter().all(|id| [host, gnarr].contains(id)), "{to:?}");
    check_state(state).unwrap();
    let setup = scenario.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn a_commander_card_of_a_merged_permanent_is_asked_alone_and_the_other_card_goes_on(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] commander_is_the_spell: bool,
    #[values(To::Graveyard, To::Exile, To::Hand, To::Top, To::Bottom, To::Shuffled)] to: To,
    #[values(true, false)] accept: bool,
) -> Result<(), ScriptError> {
    let who = if commander_is_the_spell {
        Who::Spell
    } else {
        Who::Host
    };
    let (mut driver, scenario, host, gnarr) = merged(seats, who, to)?;
    let commander = if commander_is_the_spell { gnarr } else { host };
    let casts = driver.state().commander_casts(commander);
    send_off(
        &mut driver,
        &scenario,
        (host, gnarr),
        to,
        &[(commander, accept)],
    )?;
    if accept {
        // The commander is cast from the command zone again, for its tax, and the cast is counted.
        let state = driver.state();
        assert_eq!(state.commander_tax(commander), 2 * casts);
        let name = state.object(commander).unwrap().card().name.clone();
        driver.begin_cast(&name)?;
        if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
            choose_way(&mut driver, "Cast")?;
        }
        pay(&mut driver)?;
        assert_eq!(driver.state().commander_casts(commander), casts + 1);
        assert_eq!(
            driver.state().object(commander).unwrap().zone(),
            Zone::Stack
        );
    }
    Ok(())
}

#[rstest]
fn two_commander_cards_of_one_merged_permanent_are_each_asked(
    #[values(2, 4)] seats: usize,
    #[values(To::Graveyard, To::Exile, To::Hand, To::Top, To::Bottom, To::Shuffled)] to: To,
    #[values(true, false)] bear_accepts: bool,
    #[values(true, false)] gnarr_accepts: bool,
) -> Result<(), ScriptError> {
    let (mut driver, scenario, host, gnarr) = merged(seats, Who::Both, to)?;
    send_off(
        &mut driver,
        &scenario,
        (host, gnarr),
        to,
        &[(host, bear_accepts), (gnarr, gnarr_accepts)],
    )
}
