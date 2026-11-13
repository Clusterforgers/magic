//! Goal: keywords that static abilities give to spells and cards with a cost read off each card play whole at two to four
//! seats: a replicate cost equal to the spell's mana cost is offered and copies once per payment (a printed replicate of
//! the same cost adds no second copy trigger), an emerge cost equal to a creature spell's mana cost casts it by
//! sacrifice, a miracle cost is the mana cost less its generic part, "Cascade, cascade." cascades twice only for a
//! colorless spell of mana value 7 or more, and exploiting a non-Human draws (with a Treasure for power 3 or more); the
//! log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{choose_way, lands, pay};
use crate::common::{name_of, names, pid};

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::Ref;
use mtg_cards::value::{Amount, CastWay};
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::{CardsChoice, NumberChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{cost, free_creature, island, plains, swamp};
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn card(name: &str) -> Arc<CardDefinition> {
    corpus::card(name).unwrap()
}

/// A sorcery for `{1}{U}` that gains its controller 2 life, with a printed replicate of `{1}{U}` when `printed`.
fn chant(printed: bool) -> Arc<CardDefinition> {
    let base = CardDefinition::new("Chant")
        .with_type(CardType::Sorcery)
        .with_mana_cost(cost(1, &[Color::Blue]));
    let base = if printed {
        base.with_rules(Ability::Keyword(Keyword::Replicate(Cost::mana(
            "{1}{U}".parse().unwrap(),
        ))))
    } else {
        base
    };
    Arc::new(base.with_rules(Ability::Spell {
        targets: Vec::new(),
        effects: vec![Effect::GainLife {
            who: Ref::You,
            amount: Amount::Fixed(2),
        }],
    }))
}

/// Seat 0 holds `hand` and controls `permanents` in the first main phase of turn 1; the others have a creature each.
fn table(
    seats: usize,
    hand: Vec<Arc<CardDefinition>>,
    permanents: Vec<Arc<CardDefinition>>,
    land: fn() -> Arc<CardDefinition>,
    count: usize,
) -> Scenario {
    let mut battlefield: Vec<_> = permanents.into_iter().map(ready).collect();
    battlefield.extend(lands(count, land));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, hand)
        .battlefield(0, battlefield);
    (0..seats).fold(scenario, |scenario, seat| {
        let scenario = scenario.library(seat, (0..8).map(|_| island()));
        if seat == 0 {
            scenario
        } else {
            scenario.battlefield(seat, [ready(free_creature("Foe", 2, 2))])
        }
    })
}

fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Passes priority until the stack is empty, keeping the targets of copies and taking every "may".
fn settle(driver: &mut Driver, picked: Option<mtg_core::ids::ObjectId>) -> Result<(), ScriptError> {
    for _ in 0..80 {
        match driver.decision()?.kind.clone() {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(picked.is_some())))?;
            }
            DecisionKind::ChooseCards(options) => {
                let cards = picked
                    .filter(|id| options.from.contains(id))
                    .map_or_else(Vec::new, |id| vec![id]);
                driver.answer(Answer::ChooseCards(CardsChoice(cards)))?;
            }
            DecisionKind::PayMana(_) => {
                pay(driver)?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
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

fn life(driver: &Driver) -> i32 {
    driver.state().player(pid(0)).unwrap().life()
}

#[rstest]
#[case::not_replicated(false, 0, 1)]
#[case::replicated_once(false, 1, 2)]
#[case::replicated_twice(false, 2, 3)]
// The printed replicate and the given one cost the same: two payments still copy the spell twice, not four times.
#[case::printed_and_given_alike(true, 2, 3)]
fn r5510_djinn_gives_replicate_equal_to_the_mana_cost(
    #[case] printed: bool,
    #[case] times: u32,
    #[case] resolved: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(
        seats,
        vec![chant(printed)],
        vec![card("Djinn Illuminatus")],
        island,
        8,
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Chant")?;
    let DecisionKind::ChooseNumber(options) = &driver.decision()?.kind else {
        panic!(
            "the replicate payments are asked: {}",
            driver.decision()?.kind.name()
        );
    };
    // Eight Islands pay {1}{U} and three more {1}{U}.
    assert_eq!(options.max, Some(3));
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(times)))?;
    pay(&mut driver)?;
    settle(&mut driver, None)?;
    assert_eq!(life(&driver), 20 + 2 * resolved);
    let copies = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::SpellCopied(_)))
        .count();
    assert_eq!(copies, usize::try_from(resolved - 1).unwrap());
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn r5510_without_djinn_no_replicate_is_asked(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(table(seats, vec![chant(false)], Vec::new(), island, 8))?;
    driver.begin_cast("Chant")?;
    assert!(!matches!(
        driver.decision()?.kind,
        DecisionKind::ChooseNumber(_)
    ));
    Ok(())
}

#[rstest]
// Serra Angel's emerge cost {3}{W}{W} less Grizzly Bears' mana value 2 leaves {1}{W}{W}.
#[case::with_herigast(true, 3)]
#[case::without_herigast(false, 3)]
fn r5510_herigast_gives_emerge_equal_to_the_mana_cost(
    #[case] herigast: bool,
    #[case] plains_count: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut permanents = vec![card("Grizzly Bears")];
    if herigast {
        permanents.push(card("Herigast, Erupting Nullkite"));
    }
    let scenario = table(
        seats,
        vec![card("Serra Angel")],
        permanents,
        plains,
        plains_count,
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let castable = match &driver.decision()?.kind {
        DecisionKind::Priority(options) => options
            .castable
            .iter()
            .any(|id| name_of(driver.state(), *id) == "Serra Angel"),
        _ => false,
    };
    assert_eq!(castable, herigast);
    if !herigast {
        return Ok(());
    }
    let bears = driver.id_of("Grizzly Bears")?;
    driver.begin_cast("Serra Angel")?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        choose_way(&mut driver, "Emerge")?;
    }
    settle(&mut driver, Some(bears))?;
    let ways: Vec<_> = driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::SpellCast { choices, .. } => Some(choices.way),
            _ => None,
        })
        .collect();
    assert_eq!(ways, [Some(CastWay::Emerge)]);
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Serra Angel".to_owned()));
    assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Grizzly Bears".to_owned()));
    assert_replays(&driver);
    Ok(())
}

#[rstest]
// Pacifism {1}{W}: the {1} goes, {W} stays.
#[case::pacifism("Pacifism", Some("{W}"))]
// A creature card is no enchantment: no miracle.
#[case::not_an_enchantment("Serra Angel", None)]
fn r5510_aminatou_gives_miracle_reduced_by_four(
    #[case] name: &str,
    #[case] miracle: Option<&str>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(
        seats,
        vec![card(name)],
        vec![card("Aminatou, Veil Piercer")],
        plains,
        2,
    );
    let driver = Driver::from_scenario(scenario)?;
    let id = driver.state().zone(Zone::Hand(pid(0)))[0];
    let found: Vec<Keyword> = mtg_engine::keywords::keywords_of(driver.state(), id)
        .into_iter()
        .filter(|keyword| matches!(keyword, Keyword::Miracle(_)))
        .cloned()
        .collect();
    let wanted: Vec<Keyword> = miracle
        .map(|mana| Keyword::Miracle(Cost::mana(mana.parse().unwrap())))
        .into_iter()
        .collect();
    assert_eq!(found, wanted);
    Ok(())
}

#[rstest]
// Ulamog's Crusher is colorless with mana value 8: two cascades. Serra Angel is white: none.
#[case::colorless_eight("Ulamog's Crusher", 8, island, 2)]
#[case::white_five("Serra Angel", 5, plains, 0)]
fn r5510_zhulodok_gives_cascade_twice(
    #[case] name: &str,
    #[case] count: usize,
    #[case] land: fn() -> Arc<CardDefinition>,
    #[case] cascades: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(
        seats,
        vec![card(name)],
        vec![card("Zhulodok, Void Gorger")],
        land,
        count,
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast(name)?;
    pay(&mut driver)?;
    let triggered = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::AbilityTriggered { .. }))
        .count();
    assert_eq!(triggered, cascades);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
// Serra Angel is a non-Human of power 4: a card and a Treasure. Grizzly Bears has power 2: a card. Akrasan Squire is a
// Human: nothing.
#[case::non_human_power_four("Serra Angel", 1, 1)]
#[case::non_human_power_two("Grizzly Bears", 1, 0)]
#[case::human("Akrasan Squire", 0, 0)]
fn r5510_henry_wu_draws_for_an_exploited_non_human(
    #[case] sacrificed: &str,
    #[case] drawn: usize,
    #[case] treasures: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(
        seats,
        vec![card("Henry Wu, InGen Geneticist")],
        vec![card(sacrificed)],
        island,
        0,
    )
    .battlefield(
        0,
        [
            ready(swamp()),
            ready(mtg_testkit::cards::forest()),
            ready(island()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let picked = driver.id_of(sacrificed)?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.begin_cast("Henry Wu, InGen Geneticist")?;
    settle(&mut driver, Some(picked))?;
    assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&sacrificed.to_owned()));
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        hand - 1 + drawn
    );
    let treasure_count = names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|name| name.as_str() == "Treasure")
        .count();
    assert_eq!(treasure_count, treasures);
    assert_replays(&driver);
    Ok(())
}
