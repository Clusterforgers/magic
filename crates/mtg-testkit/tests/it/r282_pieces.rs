//! Goal: the r28-2 pieces play at tables of two, three and four seats: suspend X asks X from the least it allows, costs X more and puts X time counters on the card; compleated takes two loyalty counters for each Phyrexian symbol paid with life and none when it is paid with mana; More Than Meets the Eye casts the back face from a hand for its cost and the card enters converted, or the front face for its mana cost when the caster picks it; cumulative upkeep with an action for its cost asks its controller to do the actions once for each age counter or sacrifice the permanent, offers the actions only while they can be done, and a spell that plots a card exiles it face up, plotted, so that its owner may cast it for free on a later turn and not on the turn it was plotted in; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::choose::OptionChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameSetup, replay};
use mtg_testkit::cards::{forest, free_creature, keyworded};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

/// The log replays to the live state, and an engine rebuilt from it stands at the same decision.
fn assert_replays(driver: &Driver, setup: &GameSetup) {
    let log = driver.engine().events().to_vec();
    let replayed = replay(setup, &log).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    let resumed = Engine::resume(setup.clone(), log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
}

/// A 2/2 with cumulative upkeep: "An opponent gains 1 life", offered while `payable` (JSON) holds.
fn ager(payable: Option<&str>) -> Arc<CardDefinition> {
    let payable = payable.map_or(String::new(), |p| format!(r#","payable":{p}"#));
    let keyword: Keyword = serde_json::from_str(&format!(
        r#"{{"CumulativeUpkeepActs":{{"acts":[{{"Choose":{{"from":"Opponents","as":"opponent"}}}},{{"GainLife":{{"who":{{"Bound":"opponent"}},"amount":1}}}}]{payable}}}}}"#
    ))
    .unwrap();
    keyworded("Ager", 2, 2, &[keyword])
}

/// Seat 0 controls the ager; each other seat controls a creature when `bystanders`.
fn age_table(seats: usize, ager: Arc<CardDefinition>, bystanders: bool) -> Scenario {
    let mut table = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(ager)]);
    for seat in 0..seats {
        table = table.library(seat, vec![forest(); 20]);
        if bystanders && seat > 0 {
            table = table.battlefield(seat, [ready(free_creature("Bystander", 1, 1))]);
        }
    }
    table
}

/// The life the opponents of seat 0 have gained in all.
fn opponents_gained(driver: &Driver, seats: usize, start: i32) -> i32 {
    (1..seats)
        .map(|seat| driver.state().player(pid(seat)).unwrap().life() - start)
        .sum()
}

/// Answers the questions of the actions (the opponent chosen among several is the first offered) until the stack is
/// empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..80 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseOption(_) => {
                driver.answer(Answer::ChooseOption(OptionChoice(0)))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("an empty stack".into()))
}

/// Answers until the choice between the actions and the sacrifice is asked (true) or the stack is empty (false).
fn until_choice(driver: &mut Driver) -> Result<bool, ScriptError> {
    for _ in 0..80 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseOption(_) => return Ok(true),
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(false),
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the choice or an empty stack".into()))
}

fn on_battlefield(driver: &Driver, name: &str) -> bool {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .any(|id| name_of(driver.state(), *id) == name)
}

#[rstest]
fn cumulative_upkeep_with_an_action_does_it_once_for_each_age_counter(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || age_table(seats, ager(None), true);
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    let start = driver.state().player(pid(1)).unwrap().life();
    let round = u32::try_from(seats).unwrap();
    let mut total = 0;
    for count in 1..=3 {
        driver.idle_until(1 + round * u32::try_from(count).unwrap(), Step::Upkeep)?;
        assert!(until_choice(&mut driver)?);
        assert_eq!(driver.decision()?.player, pid(0));
        driver.answer(Answer::ChooseOption(OptionChoice(0)))?;
        settle(&mut driver)?;
        total += count;
        assert_eq!(opponents_gained(&driver, seats, start), total);
        assert!(on_battlefield(&driver, "Ager"));
    }
    assert_replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn cumulative_upkeep_with_an_action_declined_sacrifices_the_permanent(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || age_table(seats, ager(None), true);
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    let start = driver.state().player(pid(1)).unwrap().life();
    driver.idle_until(1 + u32::try_from(seats).unwrap(), Step::Upkeep)?;
    assert!(until_choice(&mut driver)?);
    driver.answer(Answer::ChooseOption(OptionChoice(1)))?;
    assert!(!until_choice(&mut driver)?);
    assert!(!on_battlefield(&driver, "Ager"));
    assert_eq!(opponents_gained(&driver, seats, start), 0);
    assert_replays(&driver, &setup);
    Ok(())
}

/// The actions are offered only while they can be done: with no creature an opponent controls the permanent is
/// sacrificed without a question; with one, the choice is asked.
#[rstest]
fn cumulative_upkeep_with_an_action_that_cannot_be_done_sacrifices_without_asking(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] bystanders: bool,
) -> Result<(), ScriptError> {
    let payable = r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]},"op":"Ge","right":1}}"#;
    let build = || age_table(seats, ager(Some(payable)), bystanders);
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    driver.idle_until(1 + u32::try_from(seats).unwrap(), Step::Upkeep)?;
    assert_eq!(until_choice(&mut driver)?, bystanders);
    if bystanders {
        driver.answer(Answer::ChooseOption(OptionChoice(0)))?;
        settle(&mut driver)?;
    }
    assert_eq!(on_battlefield(&driver, "Ager"), bystanders);
    assert_replays(&driver, &setup);
    Ok(())
}

/// A free sorcery: "Exile a creature card from your hand. It becomes plotted."
fn schemer() -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(
        r#"{"Spell":{"effects":[{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"},{"Type":"Creature"}]},"as":"card"}},{"Plot":{"what":{"Bound":"card"}}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Schemer")
            .with_type(CardType::Sorcery)
            .with_mana_cost(mtg_core::mana::ManaCost::parse("{0}").unwrap())
            .with_rules(rules),
    )
}

fn castable(driver: &Driver) -> Vec<String> {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        return Vec::new();
    };
    options
        .castable
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect()
}

#[rstest]
fn a_plotted_card_is_cast_for_free_by_its_owner_on_a_later_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let bear = keyworded("Pricey Bear", 3, 3, &[]);
    let build = || {
        let mut table = Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, [schemer(), bear.clone()]);
        for seat in 0..seats {
            table = table.library(seat, vec![forest(); 20]);
        }
        table
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    driver.cast("Schemer")?;
    while !driver.state().stack().is_empty() {
        driver.answer_passively()?;
    }
    let exiled = driver.state().zone(Zone::Exile).to_vec();
    assert_eq!(exiled.len(), 1);
    let card = driver.state().object(exiled[0]).unwrap();
    assert_eq!(name_of(driver.state(), exiled[0]), "Pricey Bear");
    assert_eq!(card.plotted(), Some(1));
    assert!(!castable(&driver).contains(&"Pricey Bear".to_owned()));
    let next = 1 + u32::try_from(seats).unwrap();
    driver.idle_until(next, Step::Main1)?;
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    assert!(castable(&driver).contains(&"Pricey Bear".to_owned()));
    assert_replays(&driver, &setup);
    Ok(())
}

/// Seat 0 holds Starscream, Power Hungry ({3}{B}, More Than Meets the Eye {2}{B}) and controls `swamps` Swamps.
fn starscream_table(seats: usize, swamps: usize) -> Scenario {
    let starscream =
        mtg_testkit::corpus::card("Starscream, Power Hungry // Starscream, Seeker Leader").unwrap();
    let lands: Vec<_> = (0..swamps)
        .map(|_| ready(mtg_testkit::cards::swamp()))
        .collect();
    let mut table = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [starscream])
        .battlefield(0, lands);
    for seat in 0..seats {
        table = table.library(seat, vec![forest(); 20]);
    }
    table
}

/// With mana for the converted cost alone, the card is cast as its back face for {2}{B} and enters converted.
#[rstest]
fn more_than_meets_the_eye_casts_the_back_face_from_a_hand(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = starscream_table(seats, 3).setup()?;
    let mut driver = Driver::from_scenario(starscream_table(seats, 3))?;
    driver.begin_cast("Starscream, Power Hungry")?;
    driver.expect_decision(0, "pay-mana")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{2}{B}");
    driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
    while !driver.state().stack().is_empty() {
        driver.answer_passively()?;
    }
    assert!(on_battlefield(&driver, "Starscream, Seeker Leader"));
    assert!(!on_battlefield(&driver, "Starscream, Power Hungry"));
    assert_replays(&driver, &setup);
    Ok(())
}

/// With mana for both, the caster picks the face, and the front face costs its mana cost.
#[rstest]
fn more_than_meets_the_eye_offers_both_faces_when_both_can_be_paid(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(starscream_table(seats, 4))?;
    driver.begin_cast("Starscream, Power Hungry")?;
    let DecisionKind::ChooseOption(options) = &driver.decision()?.kind else {
        panic!("the face is asked");
    };
    assert_eq!(options.options.len(), 2);
    driver.answer(Answer::ChooseOption(OptionChoice(0)))?;
    driver.expect_decision(0, "pay-mana")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{3}{B}");
    Ok(())
}

/// Seat 0 holds Nissa, Ascended Animist ({3}{G}{G}{G/P}{G/P}, loyalty 7, compleated) and controls `forests` Forests.
fn nissa_table(seats: usize, forests: usize) -> Scenario {
    let nissa = mtg_testkit::corpus::card("Nissa, Ascended Animist").unwrap();
    let lands: Vec<_> = (0..forests).map(|_| ready(forest())).collect();
    let mut table = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [nissa])
        .battlefield(0, lands);
    for seat in 0..seats {
        table = table.library(seat, vec![forest(); 20]);
    }
    table
}

/// Compleated: paid with seven Forests Nissa enters with her printed 7 loyalty; with fewer each Phyrexian symbol paid
/// with 2 life takes two loyalty counters.
#[rstest]
fn compleated_takes_two_loyalty_for_each_phyrexian_symbol_paid_with_life(
    #[values(2, 3, 4)] seats: usize,
    #[values((7, 0, 7), (6, 2, 5), (5, 4, 3))] case: (usize, i32, u32),
) -> Result<(), ScriptError> {
    let (forests, life, loyalty) = case;
    let setup = nissa_table(seats, forests).setup()?;
    let mut driver = Driver::from_scenario(nissa_table(seats, forests))?;
    let before = driver.state().player(pid(0)).unwrap().life();
    driver.cast("Nissa, Ascended Animist")?;
    while !driver.state().stack().is_empty() {
        driver.answer_passively()?;
    }
    assert_eq!(before - driver.state().player(pid(0)).unwrap().life(), life);
    let nissa = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == "Nissa, Ascended Animist")
        .unwrap();
    let counters = driver
        .state()
        .object(nissa)
        .and_then(|object| object.permanent())
        .map_or(0, |held| {
            held.counters_of(&mtg_cards::value::CounterKind::Loyalty)
        });
    assert_eq!(counters, loyalty);
    assert_replays(&driver, &setup);
    Ok(())
}

/// A 5-mana creature with "Suspend X—{X}{W}. X can't be 0."
fn x_suspender() -> Arc<CardDefinition> {
    let keyword: Keyword =
        serde_json::from_str(r#"{"Suspend":{"time":0,"cost":[{"Mana":"{X}{W}"},{"XAtLeast":1}]}}"#)
            .unwrap();
    Arc::new(
        CardDefinition::new("Waiting Soldier")
            .with_type(CardType::Creature)
            .with_power_toughness(3, 3)
            .with_mana_cost(mtg_testkit::cards::cost(5, &[]))
            .with_rules(Ability::Keyword(keyword)),
    )
}

/// Suspend X asks X from 1 up, costs X more, and puts X time counters on the card in exile.
#[rstest]
fn suspend_x_asks_x_and_puts_that_many_time_counters(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 3)] x: u32,
) -> Result<(), ScriptError> {
    let build = || {
        let plains: Vec<_> = (0..4)
            .map(|_| ready(mtg_testkit::cards::plains()))
            .collect();
        let mut table = Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, [x_suspender()])
            .battlefield(0, plains);
        for seat in 0..seats {
            table = table.library(seat, vec![forest(); 20]);
        }
        table
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    let card = driver.state().zone(Zone::Hand(pid(0)))[0];
    driver.answer(Answer::Priority(
        mtg_engine::decision::PriorityAction::Suspend(card),
    ))?;
    let DecisionKind::ChooseNumber(options) = &driver.decision()?.kind else {
        panic!("X is asked");
    };
    assert_eq!(options.min, 1, "X can't be 0");
    driver.answer(Answer::ChooseNumber(
        mtg_engine::choose::NumberChoice::Number(x),
    ))?;
    driver.expect_decision(0, "pay-mana")?;
    assert_eq!(
        driver.pay_options()?.cost.to_string(),
        format!("{{{x}}}{{W}}")
    );
    driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
    let exiled = driver.state().zone(Zone::Exile).to_vec();
    assert_eq!(exiled.len(), 1);
    let time = driver
        .state()
        .object(exiled[0])
        .unwrap()
        .card_counters_of(&mtg_cards::value::CounterKind::Other("time".to_owned()));
    assert_eq!(time, x);
    assert_replays(&driver, &setup);
    Ok(())
}
