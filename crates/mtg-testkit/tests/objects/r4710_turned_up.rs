//! Goal: "a creature you control that was turned face up this turn" plays at two to four seats: Kaust draws a card when a
//! creature its controller turned face up this turn deals combat damage to a player, and none for one turned face up on
//! an earlier turn.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, pay};

/// A disguise creature: a 3/3 for {1}{G} that is turned face up for {G}.
fn r4710_masked_ape() -> Arc<CardDefinition> {
    let mut def = CardDefinition::new("Masked Ape")
        .with_type(CardType::Creature)
        .with_power_toughness(3, 3);
    def.mana_cost = Some(mtg_core::mana::ManaCost::parse("{1}{G}").unwrap());
    def.rules = vec![Arc::new(Ability::Keyword(Keyword::Disguise(Cost::mana(
        "{G}".parse().unwrap(),
    ))))];
    Arc::new(def)
}

/// Passes priority until the stack is empty.
fn r4710_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            || driver.state().stack().is_empty()
        {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

/// Turns seat 0's only face-down permanent face up and pays for it.
fn r4710_turn_up(driver: &mut Driver) -> Result<(), ScriptError> {
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority to turn the Ape face up");
    };
    let id = *options.turnable.first().expect("a face-down Ape");
    driver.answer(Answer::Priority(PriorityAction::TurnFaceUp(id)))?;
    pay(driver).map(|_| ())
}

/// Seat 0 controls Kaust and casts the Ape face down on turn 1, turning it face up then (`early`) or on its next turn, and
/// attacks with it; the cards seat 0 draws from the combat damage.
#[rstest]
#[case::up_this_turn_two_seats(2, false, 1)]
#[case::up_this_turn_four_seats(4, false, 1)]
#[case::up_last_turn(3, true, 0)]
fn r4710_kaust_draws_for_a_creature_turned_up_this_turn(
    #[case] seats: usize,
    #[case] early: bool,
    #[case] drawn: usize,
) -> Result<(), ScriptError> {
    let mut permanents: Vec<_> = (0..5).map(|_| ready(forest())).collect();
    permanents.push(ready(mountain()));
    permanents.push(ready(card("Kaust, Eyes of the Glade").unwrap()));
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .hand(0, [r4710_masked_ape()])
            .battlefield(0, permanents)
            .at(1, 0, Step::Main1),
        |scenario, seat| scenario.library(seat, vec![forest(); 12]),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Masked Ape")?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        choose_way(&mut driver, "Cast face down")?;
    }
    pay(&mut driver)?;
    r4710_settle(&mut driver)?;
    if early {
        r4710_turn_up(&mut driver)?;
    }
    let next = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(next, Step::Main1)?;
    if !early {
        r4710_turn_up(&mut driver)?;
    }
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Masked Ape", 1)])?;
    let held = driver
        .state()
        .zone(Zone::Hand(PlayerId::from_index(0)))
        .len();
    while driver.state().turn().step() != Step::Main2 {
        let decision = driver.decision()?.clone();
        driver.answer(Driver::passive_answer(&decision))?;
    }
    let now = driver
        .state()
        .zone(Zone::Hand(PlayerId::from_index(0)))
        .len();
    assert_eq!(now - held, drawn);
    Ok(())
}
