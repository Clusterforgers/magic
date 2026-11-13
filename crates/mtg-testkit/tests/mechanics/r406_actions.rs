//! Goal: a replacement of a keyword action plays at tables of two to four seats: Topography Tracker has each creature
//! you control explore twice, Twists and Turns has you scry 1 before the exploration (so the exploration reveals the
//! card the scry leaves on top), the two together let the explorer's controller choose which applies first (one scry
//! or two), Tekuthal makes a proliferation happen twice and Leader, Super-Genius draws a card before each connive, and Ashes of the Abhorrent stop the abilities of cards in graveyards (unearth); without
//! the replacement the action happens once, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::{CardsChoice, OptionChoice, ScryChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, burn, cost, forest, free_creature, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

/// A sorcery for `{G}` with these effects.
fn r406_spell(name: &str, effects: Vec<Effect>) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[Color::Green]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects,
            }),
    )
}

/// "Each creature you control explores."
fn r406_survey() -> Arc<CardDefinition> {
    r406_spell(
        "Survey",
        vec![Effect::Explore {
            what: Ref::Each(Selector(vec![
                Pred::Type(CardType::Creature),
                Pred::ControlledBy(Ref::You),
            ])),
        }],
    )
}

/// Seat 0's first main phase with `battlefield`, three Forests and three Swamps ready, `hand` in hand and `top` on top of
/// its library; every seat has a library.
fn r406_table(
    seats: usize,
    battlefield: Vec<Arc<CardDefinition>>,
    hand: Vec<Arc<CardDefinition>>,
    top: &[Arc<CardDefinition>],
) -> Scenario {
    let lands = (0..3).flat_map(|_| [forest(), swamp()]);
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, hand)
        .battlefield(0, battlefield.into_iter().chain(lands).map(ready));
    for seat in 0..seats {
        let mut cards: Vec<_> = if seat == 0 { top.to_vec() } else { Vec::new() };
        cards.extend((0..12).map(|_| forest()));
        scenario = scenario.library(seat, cards);
    }
    scenario
}

/// Answers until the stack is empty and seat 0 has priority: `pick` answers what it knows (a `None` is answered
/// passively), mana is paid automatically. Returns the decisions seen, by kind name.
fn r406_settle(
    driver: &mut Driver,
    mut pick: impl FnMut(&Driver, &Decision) -> Option<Answer>,
) -> Result<Vec<String>, ScriptError> {
    let mut seen = Vec::new();
    for _ in 0..120 {
        let decision = driver.decision()?.clone();
        let kind = decision.kind.name().to_owned();
        match kind.as_str() {
            "pay-mana" => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            "priority"
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(seen);
            }
            "priority" => {
                driver.pass()?;
            }
            _ => {
                seen.push(kind);
                match pick(driver, &decision) {
                    Some(answer) => driver.answer(answer)?,
                    None => driver.answer_passively()?,
                };
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

/// The +1/+1 counters on the permanent called `name`.
fn r406_plus(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne)
}

fn r406_replays(driver: &Driver, scenario: &Scenario) {
    let setup = scenario.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
#[case::with_the_tracker(true, 2)]
#[case::without_it(false, 1)]
fn r406_topography_tracker_explores_twice(
    #[values(2, 3, 4)] seats: usize,
    #[case] tracker: bool,
    #[case] each: u32,
) -> Result<(), ScriptError> {
    let mut battlefield = vec![bear()];
    if tracker {
        battlefield.push(card("Topography Tracker").unwrap());
    }
    let top: Vec<_> = (0..4).map(|_| burn("Shock", 2)).collect();
    let scenario = r406_table(seats, battlefield, vec![r406_survey()], &top);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.cast("Survey")?;
    let seen = r406_settle(&mut driver, |_, _| None)?;
    assert_eq!(r406_plus(&driver, "Grizzly Bears"), each);
    if tracker {
        assert_eq!(r406_plus(&driver, "Topography Tracker"), each);
    }
    let explored = seen.iter().filter(|kind| *kind == "choose-cards").count();
    assert_eq!(explored, if tracker { 4 } else { 1 }, "{seen:?}");
    assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Survey".to_owned()));
    r406_replays(&driver, &scenario);
    Ok(())
}

#[rstest]
#[case::the_card_bottomed_reveals_a_land(true, 0, true)]
#[case::the_card_kept_is_revealed(false, 1, false)]
fn r406_twists_and_turns_scries_before_the_exploration(
    #[values(2, 3, 4)] seats: usize,
    #[case] bottom: bool,
    #[case] counters: u32,
    #[case] land_in_hand: bool,
) -> Result<(), ScriptError> {
    let battlefield = vec![bear(), card("Twists and Turns // Mycoid Maze").unwrap()];
    let scenario = r406_table(seats, battlefield, vec![r406_survey()], &[burn("Shock", 2)]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.cast("Survey")?;
    let seen = r406_settle(&mut driver, |_, decision| match &decision.kind {
        DecisionKind::Scry(scry) if bottom => Some(Answer::Scry(ScryChoice {
            top: Vec::new(),
            bottom: scry.cards.clone(),
        })),
        _ => None,
    })?;
    assert_eq!(seen.first().map(String::as_str), Some("scry"), "{seen:?}");
    assert_eq!(r406_plus(&driver, "Grizzly Bears"), counters);
    let hand = names(driver.state(), Zone::Hand(pid(0)));
    assert_eq!(
        hand.contains(&"Forest".to_owned()),
        land_in_hand,
        "{hand:?}"
    );
    r406_replays(&driver, &scenario);
    Ok(())
}

#[rstest]
#[case::twists_first("Twists and Turns", 1)]
#[case::tracker_first("Topography Tracker", 2)]
fn r406_the_explorer_orders_two_replacements(
    #[values(2, 3, 4)] seats: usize,
    #[case] first: &str,
    #[case] scries: usize,
) -> Result<(), ScriptError> {
    let battlefield = vec![
        card("Topography Tracker").unwrap(),
        card("Twists and Turns // Mycoid Maze").unwrap(),
    ];
    let top: Vec<_> = (0..6).map(|_| burn("Shock", 2)).collect();
    let scenario = r406_table(seats, battlefield, vec![r406_survey()], &top);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.cast("Survey")?;
    let seen = r406_settle(&mut driver, |_, decision| match &decision.kind {
        DecisionKind::ChooseOption(options) => {
            let at = options
                .options
                .iter()
                .position(|option| format!("{option:?}").contains(first))
                .unwrap();
            Some(Answer::ChooseOption(OptionChoice(at)))
        }
        _ => None,
    })?;
    assert_eq!(seen.first().map(String::as_str), Some("choose-option"));
    assert_eq!(
        seen.iter().filter(|kind| *kind == "scry").count(),
        scries,
        "{seen:?}"
    );
    assert_eq!(r406_plus(&driver, "Topography Tracker"), 2);
    r406_replays(&driver, &scenario);
    Ok(())
}

#[rstest]
#[case::with_tekuthal(true, 3)]
#[case::without_him(false, 2)]
fn r406_tekuthal_proliferates_twice(
    #[values(2, 3, 4)] seats: usize,
    #[case] tekuthal: bool,
    #[case] counters: u32,
) -> Result<(), ScriptError> {
    let mut battlefield = vec![free_creature("Wall", 0, 8)];
    if tekuthal {
        battlefield.push(card("Tekuthal, Inquiry Dominus").unwrap());
    }
    let scenario = r406_table(
        seats,
        battlefield,
        vec![card("Grim Affliction").unwrap()],
        &[],
    );
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let wall = driver.target_named("Wall")?;
    driver.cast_at("Grim Affliction", &[wall])?;
    let seen = r406_settle(&mut driver, |_, decision| match &decision.kind {
        DecisionKind::ChooseCards(cards) => {
            Some(Answer::ChooseCards(CardsChoice(cards.from.clone())))
        }
        _ => None,
    })?;
    let id = driver.id_of("Wall")?;
    let held = driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .unwrap()
        .counters_of(&CounterKind::MinusOneMinusOne);
    assert_eq!(held, counters, "{seen:?}");
    r406_replays(&driver, &scenario);
    Ok(())
}

#[rstest]
#[case::with_leader(true, 2)]
#[case::without_the_replacement(false, 0)]
fn r406_leader_draws_before_the_connive(
    #[values(2, 3, 4)] seats: usize,
    #[case] leader: bool,
    #[case] kept: usize,
) -> Result<(), ScriptError> {
    let connive = r406_spell(
        "Scheme",
        vec![Effect::Connive {
            what: Ref::Each(Selector(vec![
                Pred::Type(CardType::Creature),
                Pred::ControlledBy(Ref::You),
            ])),
            count: mtg_cards::value::Amount::one(),
        }],
    );
    let mut battlefield = vec![bear()];
    if leader {
        battlefield.push(card("Leader, Super-Genius").unwrap());
    }
    let top: Vec<_> = (0..4).map(|_| burn("Shock", 2)).collect();
    let scenario = r406_table(seats, battlefield, vec![connive], &top);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.cast("Scheme")?;
    r406_settle(&mut driver, |_, _| None)?;
    let hand = names(driver.state(), Zone::Hand(pid(0)));
    assert_eq!(hand.len(), kept, "{hand:?}");
    assert_eq!(r406_plus(&driver, "Grizzly Bears"), 1);
    let graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    assert_eq!(
        graveyard.iter().filter(|name| *name == "Shock").count(),
        if leader { 2 } else { 1 },
        "{graveyard:?}"
    );
    r406_replays(&driver, &scenario);
    Ok(())
}

#[rstest]
#[case::with_ashes(true)]
#[case::without_them(false)]
fn r406_ashes_of_the_abhorrent_stop_graveyard_abilities(
    #[values(2, 3, 4)] seats: usize,
    #[case] ashes: bool,
) -> Result<(), ScriptError> {
    let battlefield = if ashes {
        vec![card("Ashes of the Abhorrent").unwrap()]
    } else {
        Vec::new()
    };
    let scenario = r406_table(seats, battlefield, Vec::new(), &[])
        .graveyard(0, [card("Dregscape Zombie").unwrap()]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let offered = driver.offered_abilities("Dregscape Zombie")?;
    assert_eq!(offered.is_empty(), ashes, "{offered:?}");
    if !ashes {
        driver.begin_activation("Dregscape Zombie")?;
        r406_settle(&mut driver, |_, _| None)?;
        assert!(names(driver.state(), Zone::Battlefield).contains(&"Dregscape Zombie".to_owned()));
    }
    r406_replays(&driver, &scenario);
    Ok(())
}
