//! Goal: replicate plays whole at tables of two, three and four seats: the number of payments is asked after the costs, never above what the lands pay for, a spell paid 0, 1 and many times is cast once and copied once for each payment (the copies are not cast and do not replicate), the cost made of tapped Daleks asks a creature for each payment and taps each only once, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::casting::{lands, pay};
use crate::common::pid;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::Ref;
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, NumberChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{cost, free_creature, island, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A sorcery for `{1}{U}` with replicate `{1}` whose only effect is that its controller gains 2 life.
fn chant() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Chant")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(1, &[Color::Blue]))
            .with_rules(Ability::Keyword(Keyword::Replicate(Cost::mana(
                "{1}".parse().unwrap(),
            ))))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![Effect::GainLife {
                    who: Ref::You,
                    amount: Amount::Fixed(2),
                }],
            }),
    )
}

/// Seat 0 holds the cards and controls `permanents` in the first main phase of turn 1.
fn table(
    seats: usize,
    hand: Vec<Arc<CardDefinition>>,
    permanents: Vec<mtg_engine::PresetPermanent>,
) -> Scenario {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, hand)
        .battlefield(0, permanents);
    (1..seats).fold(scenario, |scenario, seat| {
        scenario.battlefield(seat, [ready(free_creature("Foe", 2, 2))])
    })
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// The bound of the question how many times the replicate cost is paid.
fn most_payments(driver: &Driver) -> Option<u32> {
    let DecisionKind::ChooseNumber(options) = &driver.decision().unwrap().kind else {
        panic!(
            "the payments are asked: {}",
            driver.decision().unwrap().kind.name()
        );
    };
    options.max
}

/// Casts the spell paying replicate `times` times and lets the stack empty.
fn cast_chant(driver: &mut Driver, times: u32) -> Result<(), ScriptError> {
    driver.begin_cast("Chant")?;
    driver.expect_decision(0, "choose-number")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(times)))?;
    pay(driver)?;
    settle(driver).map(|_| ())
}

/// Passes priority until the stack is empty, keeping the targets of each copy (the question is asked once for each copy);
/// returns how many times it was asked.
fn settle(driver: &mut Driver) -> Result<usize, ScriptError> {
    let mut asked = 0;
    for _ in 0..80 {
        if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            asked += 1;
            driver.answer(Answer::YesNo(YesNoChoice(false)))?;
        } else if driver.state().stack().is_empty() {
            break;
        } else {
            driver.pass()?;
        }
    }
    Ok(asked)
}

#[rstest]
#[case::paid_zero_times(0, 1)]
#[case::paid_once(1, 2)]
#[case::paid_three_times(3, 4)]
fn a_spell_is_copied_once_for_each_payment_and_the_copies_are_not_cast(
    #[case] times: u32,
    #[case] resolved: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, vec![chant()], lands(5, island)))?;
    cast_chant(&mut driver, times)?;
    assert_eq!(life(&driver, 0), 20 + 2 * i32::try_from(resolved).unwrap());
    let events = driver.engine().events();
    let copies = events
        .iter()
        .filter(|event| matches!(event, Event::SpellCopied(_)))
        .count();
    let casts = events
        .iter()
        .filter(|event| matches!(event, Event::SpellCast { .. }))
        .count();
    assert_eq!((copies, casts), (resolved - 1, 1));
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::cost_only(2, 0)]
#[case::one_spare(3, 1)]
#[case::four_spare(6, 4)]
fn the_payments_asked_are_no_more_than_the_lands_pay_for(
    #[case] islands: usize,
    #[case] most: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, vec![chant()], lands(islands, island)))?;
    driver.begin_cast("Chant")?;
    if most == 0 {
        assert!(!matches!(
            driver.decision()?.kind,
            DecisionKind::ChooseNumber(_)
        ));
    } else {
        assert_eq!(most_payments(&driver), Some(most));
    }
    Ok(())
}

#[test]
fn a_cast_backed_out_of_at_the_question_leaves_no_trace() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, vec![chant()], lands(4, island)))?;
    let before = driver.engine().events().len();
    driver.begin_cast("Chant")?;
    driver.expect_decision(0, "choose-number")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Cancel))?;
    assert!(driver.state().stack().is_empty());
    assert_eq!(life(&driver, 0), 20);
    assert!(
        driver.engine().events()[before..]
            .iter()
            .all(|event| !matches!(event, Event::SpellCast { .. } | Event::SpellCopied(_)))
    );
    Ok(())
}

/// A free Dalek creature.
fn dalek(name: &str) -> Arc<CardDefinition> {
    let card = free_creature(name, 1, 1);
    Arc::new((*card).clone().with_subtype("Dalek"))
}

/// Exterminate! with `daleks` untapped Daleks and lands for its cost; the foes are the Foe of each other seat.
fn exterminate_table(seats: usize, daleks: usize) -> Result<Driver, ScriptError> {
    let mut permanents = lands(3, swamp);
    permanents.extend((0..daleks).map(|n| ready(dalek(&format!("Dalek {n}")))));
    Driver::from_scenario(table(
        seats,
        vec![mtg_testkit::corpus::card("Exterminate!").unwrap()],
        permanents,
    ))
}

fn tapped_daleks(driver: &Driver) -> usize {
    let state = driver.state();
    state
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .filter(|id| {
            state.object(**id).is_some_and(|object| {
                object.card().name.starts_with("Dalek")
                    && object
                        .permanent()
                        .is_some_and(|permanent| permanent.tapped())
            })
        })
        .count()
}

#[rstest]
#[case::no_dalek_tapped(0, 1)]
#[case::one(1, 2)]
#[case::two(2, 3)]
fn tapping_untapped_daleks_pays_one_replicate_for_each_and_never_the_same_twice(
    #[case] times: u32,
    #[case] resolved: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = exterminate_table(seats, 2)?;
    driver.begin_cast("Exterminate!")?;
    driver.expect_decision(0, "choose-number")?;
    assert_eq!(most_payments(&driver), Some(2));
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(times)))?;
    let mut picked: Vec<ObjectId> = Vec::new();
    // The last Dalek left is taken without a question.
    for asked in 0..times.min(1) {
        let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
            panic!("Dalek {asked} is asked: {}", driver.decision()?.kind.name());
        };
        // A Dalek picked for an earlier payment is not offered again.
        assert_eq!(options.from.len(), 2 - asked as usize);
        assert!(options.from.iter().all(|id| !picked.contains(id)));
        picked.push(options.from[0]);
        driver.answer(Answer::ChooseCards(CardsChoice(vec![options.from[0]])))?;
    }
    driver.expect_decision(0, "choose-targets")?;
    let foe = driver.target_named("Foe")?;
    driver.choose_targets(&[foe])?;
    pay(&mut driver)?;
    assert_eq!(tapped_daleks(&driver), times as usize);
    assert_eq!(settle(&mut driver)?, resolved - 1);
    let events = driver.engine().events();
    let casts = events
        .iter()
        .filter(|event| matches!(event, Event::SpellCast { .. }))
        .count();
    let copies = events
        .iter()
        .filter(|event| matches!(event, Event::SpellCopied(_)))
        .count();
    assert_eq!(casts, 1);
    assert_eq!(copies, resolved - 1);
    Ok(())
}
