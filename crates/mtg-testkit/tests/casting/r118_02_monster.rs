//! Goal: an entering paid by exiling X cards from the graveyard, X being the X the spell was cast with ("As this creature enters, exile X creature cards from your graveyard. If you can't, put this creature into its owner's graveyard instead of onto the battlefield. For each creature card exiled this way, this creature enters with a +2/+0, +1/+1, or +0/+2 counter on it.", Frankenstein's Monster), exiles X creature cards (asked only when there is a choice), puts one counter of the kind chosen for each card on the creature as it enters, enters with nothing taken for X = 0, and goes to the graveyard when there are too few creature cards; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::choose::{CardsChoice, NumberChoice, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

/// Seat 0 holds Frankenstein's Monster, controls six Swamps and has `dead` creature cards and a Forest in its graveyard.
fn table(seats: usize, dead: usize) -> Scenario {
    let graveyard = (0..dead)
        .map(|at| free_creature(&format!("Dead {at}"), 1, 1))
        .chain(std::iter::once(forest()));
    (0..seats).fold(
        Scenario::new(seats)
            .hand(0, [card("Frankenstein's Monster").unwrap()])
            .battlefield(0, (0..6).map(|_| ready(swamp())))
            .graveyard(0, graveyard),
        |scenario, seat| scenario.library(seat, vec![forest(); 10]),
    )
}

/// Casts the Monster with X = `x`, choosing the counters `kinds` (by index) in order; how many picks and kinds were asked.
fn cast(driver: &mut Driver, x: u32, kinds: &[usize]) -> Result<(usize, usize), ScriptError> {
    driver.begin_cast("Frankenstein's Monster")?;
    let (mut picks, mut chosen) = (0, 0);
    for _ in 0..40 {
        let Ok(decision) = driver.decision() else {
            break;
        };
        match &decision.kind.clone() {
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseCards(options) => {
                picks += 1;
                let take = options.from[..options.min as usize].to_vec();
                driver.answer(Answer::ChooseCards(CardsChoice(take)))?;
            }
            DecisionKind::ChooseOption(options) => {
                assert_eq!(options.options.len(), 3, "{:?}", options.options);
                let pick = kinds[chosen];
                chosen += 1;
                driver.answer(Answer::ChooseOption(OptionChoice(pick)))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Ok((picks, chosen))
}

#[rstest]
#[case::two_of_three_cards(3, 2, &[0, 2][..], Some((2, 3)), 1)]
#[case::every_card(2, 2, &[1, 1][..], Some((2, 3)), 0)]
#[case::three_kinds(3, 3, &[0, 1, 2][..], Some((3, 4)), 0)]
#[case::x_zero(2, 0, &[][..], Some((0, 1)), 0)]
#[case::too_few_cards(1, 2, &[][..], None, 0)]
fn the_monster_exiles_x_creature_cards_and_takes_a_counter_for_each(
    #[case] dead: usize,
    #[case] x: u32,
    #[case] kinds: &[usize],
    #[case] stats: Option<(i32, i32)>,
    #[case] picks: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, dead))?;
    let asked = cast(&mut driver, x, kinds)?;
    assert_eq!(asked, (picks, kinds.len()));
    let graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    let exiled = names(driver.state(), Zone::Exile);
    if let Some((power, toughness)) = stats {
        let monster = driver.id_of("Frankenstein's Monster")?;
        let seen = mtg_engine::characteristics::characteristics(driver.state(), monster).unwrap();
        assert_eq!((seen.power, seen.toughness), (Some(power), Some(toughness)));
        let object = driver.state().object(monster).unwrap().permanent().unwrap();
        let named = |name: &str| object.counters_of(&CounterKind::Other(name.to_owned()));
        let expected = |index: usize| kinds.iter().filter(|kind| **kind == index).count();
        assert_eq!(named("+2/+0") as usize, expected(0));
        assert_eq!(
            object.counters_of(&CounterKind::PlusOnePlusOne) as usize,
            expected(1)
        );
        assert_eq!(named("+0/+2") as usize, expected(2));
        assert_eq!(exiled.len(), x as usize, "{exiled:?}");
        assert_eq!(graveyard.len(), dead - x as usize + 1, "{graveyard:?}");
    } else {
        assert!(graveyard.contains(&"Frankenstein's Monster".to_owned()));
        assert!(exiled.is_empty(), "{exiled:?}");
        assert!(
            !names(driver.state(), Zone::Battlefield)
                .contains(&"Frankenstein's Monster".to_owned())
        );
    }
    Ok(())
}
