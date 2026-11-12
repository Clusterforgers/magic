//! Goal: a permission to cast a card that adds costs (Demonic Embrace: "by paying 3 life and discarding a card in addition to
//! paying its other costs") offers the card from the graveyard only when the caster can pay the mana, the life and a discard,
//! takes all of them with the cast, leaves the cast from a hand at its mana cost, covers the owner's graveyard alone, and the
//! Aura plays as printed at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{CardsChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::{names, on_battlefield, pid};
use mtg_cards::keyword::Keyword;
use mtg_core::card::Subtype;

/// Seat 0 holds `hand` and `swamps` Swamps and controls a 2/2 Bear, at `life`; Demonic Embrace lies in the graveyard
/// of `grave_seat`.
fn dk1d_table(
    seats: usize,
    grave_seat: usize,
    hand: &[&str],
    swamps: usize,
    life: i32,
) -> Scenario {
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, hand.iter().map(|name| free_creature(name, 1, 1)))
        .graveyard(grave_seat, [card("Demonic Embrace").unwrap()])
        .battlefield(
            0,
            (0..swamps)
                .map(|_| ready(swamp()))
                .chain([ready(free_creature("Bear", 2, 2))]),
        )
        .life(0, life)
}

/// The life of seat 0.
fn dk1d_life(driver: &Driver) -> i32 {
    driver.state().player(pid(0)).unwrap().life()
}

/// Casts Demonic Embrace on the Bear: discards the first card asked, aims at the Bear and pays with the lands.
fn dk1d_cast(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.begin_cast("Demonic Embrace")?;
    for _ in 0..8 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseCards(options) => {
                let first = options.from[0];
                driver.answer(Answer::ChooseCards(CardsChoice(vec![first])))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let bear = on_battlefield(driver.state(), "Bear").unwrap();
                let target = Target::Object(driver.state().object(bear).unwrap().reference());
                driver.choose_targets(&[target])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
                for _ in 0..40 {
                    if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
                        break;
                    }
                    driver.answer_passively()?;
                }
                return Ok(());
            }
            other => panic!("unexpected {}", other.name()),
        }
    }
    Err(ScriptError::Stuck("the cast".into()))
}

#[rstest]
fn dk1d_the_aura_is_cast_from_the_graveyard_for_the_life_a_discard_and_its_mana(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = dk1d_table(seats, 0, &["Fodder"], 3, 20).setup().unwrap();
    let mut driver = Driver::from_scenario(dk1d_table(seats, 0, &["Fodder"], 3, 20))?;
    dk1d_cast(&mut driver)?;
    assert_eq!(dk1d_life(&driver), 17, "3 life paid");
    assert_eq!(
        names(driver.state(), Zone::Hand(pid(0))),
        Vec::<String>::new()
    );
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Fodder"],
        "the discarded card, and not the Aura"
    );
    let bear = on_battlefield(driver.state(), "Bear").unwrap();
    assert_eq!(
        driver
            .state()
            .host_of(on_battlefield(driver.state(), "Demonic Embrace").unwrap()),
        Some(bear)
    );
    let now = characteristics(driver.state(), bear).unwrap();
    assert_eq!((now.power, now.toughness), (Some(5), Some(3)));
    assert!(has_keyword(driver.state(), bear, &Keyword::Flying));
    assert!(now.has_subtype(&[], &Subtype("Demon".into())));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::paid_in_full(&["Fodder"], 3, 20, true)]
#[case::exactly_the_life(&["Fodder"], 3, 3, true)]
#[case::one_life_short(&["Fodder"], 3, 2, false)]
#[case::no_card_to_discard(&[], 3, 20, false)]
#[case::one_mana_short(&["Fodder"], 2, 20, false)]
fn dk1d_a_card_is_offered_only_when_every_added_cost_can_be_paid(
    #[values(2, 4)] seats: usize,
    #[case] hand: &[&str],
    #[case] swamps: usize,
    #[case] life: i32,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(dk1d_table(seats, 0, hand, swamps, life))?;
    assert_eq!(driver.begin_cast("Demonic Embrace").is_ok(), offered);
    if !offered {
        assert_eq!(dk1d_life(&driver), life, "a refused cast takes nothing");
        assert_eq!(
            names(driver.state(), Zone::Graveyard(pid(0))),
            ["Demonic Embrace"]
        );
    }
    Ok(())
}

#[test]
fn dk1d_from_a_hand_the_aura_costs_its_mana_alone() -> Result<(), ScriptError> {
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Demonic Embrace").unwrap()])
        .battlefield(
            0,
            (0..3)
                .map(|_| ready(swamp()))
                .chain([ready(free_creature("Bear", 2, 2))]),
        );
    let mut driver = Driver::from_scenario(scenario)?;
    dk1d_cast(&mut driver)?;
    assert_eq!(dk1d_life(&driver), 20, "no life was paid");
    assert!(on_battlefield(driver.state(), "Demonic Embrace").is_some());
    Ok(())
}

#[test]
fn dk1d_only_the_graveyard_of_the_owner_is_covered() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(dk1d_table(2, 1, &["Fodder"], 3, 20))?;
    assert!(
        driver.begin_cast("Demonic Embrace").is_err(),
        "the Aura of another player's graveyard is not cast by seat 0"
    );
    Ok(())
}
