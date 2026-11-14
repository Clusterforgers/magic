//! Goal: a cost of cards alike as a set is paid only when the hand holds such a set and refuses a pick that is not
//! one, at two to four seats: Sphinx of the Chimes discards two nonland cards with the same name (two lands of one name
//! or two different names do not do), Illuminated Folio reveals two cards that share a color (colorless cards share
//! none), and a bot pick refused as not alike tries every set, those before it too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::error::{EngineError, IllegalAnswer};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 with `permanents` and `hand`, ten cards in every library, in seat 0's first main phase.
fn r5518_table(
    seats: usize,
    permanents: Vec<Arc<CardDefinition>>,
    hand: Vec<Arc<CardDefinition>>,
) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .battlefield(0, permanents.into_iter().map(ready).collect::<Vec<_>>())
        .hand(0, hand);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| forest()));
    }
    Driver::from_scenario(scenario.at(1, 0, Step::Main1))
}

/// The cards of seat 0's hand named `name`.
fn r5518_in_hand(driver: &Driver, name: &str) -> Vec<ObjectId> {
    driver
        .state()
        .zone(Zone::Hand(PlayerId::from_index(0)))
        .iter()
        .copied()
        .filter(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|o| o.card().name == name)
        })
        .collect()
}

/// Priority passes until the stack is empty.
fn r5518_settle(driver: &mut Driver) -> Result<(), ScriptError> {
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
    Ok(())
}

#[rstest]
#[case::same_name(&["Lightning Bolt", "Lightning Bolt", "Giant Growth"], true)]
#[case::different_names(&["Lightning Bolt", "Giant Growth", "Grizzly Bears"], false)]
#[case::lands_of_one_name(&["Forest", "Forest", "Lightning Bolt"], false)]
fn r5518_sphinx_discards_two_nonland_cards_with_the_same_name(
    #[values(2, 3, 4)] seats: usize,
    #[case] hand: &[&str],
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let cards = hand
        .iter()
        .map(|name| {
            if *name == "Forest" {
                forest()
            } else {
                card(name).unwrap()
            }
        })
        .collect();
    let mut driver = r5518_table(seats, vec![card("Sphinx of the Chimes").unwrap()], cards)?;
    let begun = driver.begin_activation("Sphinx of the Chimes");
    if !offered {
        // The activation never comes to a pick and nothing is put on the stack.
        if begun.is_ok() {
            assert!(!matches!(
                driver.decision()?.kind,
                DecisionKind::ChooseCards(_)
            ));
        }
        assert_eq!(driver.state().stack().len(), 0, "the stack is empty");
        return Ok(());
    }
    begun?;
    assert!(matches!(
        driver.decision()?.kind,
        DecisionKind::ChooseCards(_)
    ));
    let bolts = r5518_in_hand(&driver, "Lightning Bolt");
    let growth = r5518_in_hand(&driver, "Giant Growth");
    let refused = driver.answer(Answer::ChooseCards(CardsChoice(vec![bolts[0], growth[0]])));
    assert!(
        matches!(
            refused,
            Err(ScriptError::Engine(EngineError::Illegal(
                IllegalAnswer::WrongPick(_)
            )))
        ),
        "{refused:?}"
    );
    driver.answer(Answer::ChooseCards(CardsChoice(bolts)))?;
    r5518_settle(&mut driver)?;
    let me = PlayerId::from_index(0);
    assert_eq!(driver.state().zone(Zone::Hand(me)).len(), 1 + 4);
    assert_eq!(r5518_in_hand(&driver, "Lightning Bolt").len(), 0);
    Ok(())
}

#[rstest]
#[case::two_green(&["Llanowar Elves", "Grizzly Bears", "Lightning Bolt"], true)]
#[case::no_shared_color(&["Llanowar Elves", "Lightning Bolt", "Forest"], false)]
fn r5518_folio_reveals_two_cards_that_share_a_color(
    #[values(2, 3, 4)] seats: usize,
    #[case] hand: &[&str],
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let cards = hand
        .iter()
        .map(|name| {
            if *name == "Forest" {
                forest()
            } else {
                card(name).unwrap()
            }
        })
        .collect();
    let mut driver = r5518_table(
        seats,
        vec![card("Illuminated Folio").unwrap(), mountain()],
        cards,
    )?;
    let begun = driver.begin_activation("Illuminated Folio");
    if !offered {
        // Offered or not while mana is open, the activation never comes to a pick and nothing is put on the stack.
        if begun.is_ok() {
            assert!(!matches!(
                driver.decision()?.kind,
                DecisionKind::ChooseCards(_)
            ));
        }
        assert_eq!(driver.state().stack().len(), 0, "the stack is empty");
        return Ok(());
    }
    begun?;
    let elves = r5518_in_hand(&driver, "Llanowar Elves");
    let bolt = r5518_in_hand(&driver, "Lightning Bolt");
    let bears = r5518_in_hand(&driver, "Grizzly Bears");
    let refused = driver.answer(Answer::ChooseCards(CardsChoice(vec![elves[0], bolt[0]])));
    assert!(refused.is_err(), "{refused:?}");
    driver.answer(Answer::ChooseCards(CardsChoice(vec![elves[0], bears[0]])))?;
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.pay_tapping(&["Mountain"], 0)?;
    }
    r5518_settle(&mut driver)?;
    let me = PlayerId::from_index(0);
    assert_eq!(
        driver.state().zone(Zone::Hand(me)).len(),
        3 + 1,
        "the revealed cards stay"
    );
    Ok(())
}

#[rstest]
fn r5515_a_refused_pick_wraps_to_the_alike_set_before_it(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let hand = vec![
        card("Llanowar Elves").unwrap(),
        card("Grizzly Bears").unwrap(),
        card("Lightning Bolt").unwrap(),
        forest(),
    ];
    let mut driver = r5518_table(
        seats,
        vec![card("Illuminated Folio").unwrap(), mountain()],
        hand,
    )?;
    driver.begin_activation("Illuminated Folio")?;
    let decision = driver.decision()?.clone();
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        panic!("no pick: {decision:?}");
    };
    let alike = [
        r5518_in_hand(&driver, "Llanowar Elves")[0],
        r5518_in_hand(&driver, "Grizzly Bears")[0],
    ];
    // The last pair in offered order is not the green pair, so the only set that pays comes before the first pick.
    let last = options.from[options.from.len() - 2..].to_vec();
    assert!(!alike.iter().all(|id| last.contains(id)), "{options:?}");
    let mut engine = driver.engine().clone();
    let answer = Answer::ChooseCards(CardsChoice(last));
    mtg_testkit::bot::apply_answer(&mut engine, &decision, answer)?;
    assert!(
        !matches!(
            engine.decision().map(|d| &d.kind),
            Some(DecisionKind::ChooseCards(_))
        ),
        "the pick was taken"
    );
    Ok(())
}
