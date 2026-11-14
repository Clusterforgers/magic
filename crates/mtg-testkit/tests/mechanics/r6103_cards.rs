//! Goal: the r61-03 pieces play whole at two to four seats: mana spendable only on power-up abilities (Quinjet
//! Technician), cards exiled from a library castable this turn only when they are exactly three colors, with mana
//! spendable only on such spells (Meeting of the Five), and each player exiling until a nonland card, an opponent
//! choosing one of those and the controller casting up to two of the others for free (Plargg and Nassari).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaType;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest, instant, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Passes priority until the stack is empty and nothing resolves, answering anything else passively.
fn r6103_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The object named `name` in any public zone or a hand of the first four seats.
fn r6103_find(driver: &Driver, name: &str) -> Result<mtg_core::ids::ObjectId, ScriptError> {
    let state = driver.state();
    let mut zones = vec![Zone::Battlefield, Zone::Exile, Zone::Stack];
    for seat in 0..4 {
        zones.push(Zone::Hand(PlayerId(seat)));
        zones.push(Zone::Graveyard(PlayerId(seat)));
    }
    zones
        .into_iter()
        .flat_map(|zone| state.zone(zone).to_vec())
        .find(|id| crate::common::name_of(state, *id) == name)
        .ok_or_else(|| ScriptError::NoSuchCard {
            name: name.to_owned(),
            place: "any zone",
        })
}

/// Seat 0 with `permanents` ready and `hand`, every seat with a library of Plains, the game at seat 0's main phase.
fn r6103_table(
    seats: usize,
    permanents: Vec<Arc<CardDefinition>>,
    hand: Vec<Arc<CardDefinition>>,
    library: Vec<Arc<CardDefinition>>,
) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .battlefield(0, permanents.into_iter().map(ready).collect::<Vec<_>>())
        .hand(0, hand)
        .library(0, library);
    for seat in 1..seats {
        scenario = scenario.library(seat, (0..10).map(|_| plains()).collect::<Vec<_>>());
    }
    Driver::from_scenario(scenario.at(1, 0, Step::Main1))
}

/// Activates Quinjet Technician's mana ability that adds {R}{R}.
fn r6103_tap_two(driver: &mut Driver) -> Result<(), ScriptError> {
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        return Err(ScriptError::Stuck("no priority".into()));
    };
    let source = options
        .mana_abilities
        .iter()
        .find(|option| option.produces == ManaType::Red && option.count == 2)
        .map(|option| option.source)
        .ok_or_else(|| ScriptError::Stuck("no {R}{R} ability".into()))?;
    driver.answer(Answer::Priority(PriorityAction::ActivateMana(source)))?;
    Ok(())
}

fn r6103_red_in_pool(driver: &Driver) -> u32 {
    driver.state().player(PlayerId(0)).unwrap().mana().total()
}

#[rstest]
fn r6103_quinjet_mana_pays_a_power_up(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let jack = "Jack of Hearts, Volatile Hero";
    let mut permanents = vec![card("Quinjet Technician").unwrap(), card(jack).unwrap()];
    permanents.extend((0..4).map(|_| mountain()));
    let mut driver = r6103_table(seats, permanents, Vec::new(), vec![plains(); 10])?;
    r6103_tap_two(&mut driver)?;
    assert_eq!(r6103_red_in_pool(&driver), 2);
    // {4}{R}{R}: four Mountains and the restricted {R}{R}.
    driver.begin_activation(jack)?;
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    assert_eq!(
        r6103_red_in_pool(&driver),
        0,
        "the power-up spent the {{R}}{{R}}"
    );
    r6103_settle(&mut driver)?;
    let id = driver.id_of(jack)?;
    let counters = driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |held| held.counters_of(&CounterKind::PlusOnePlusOne));
    assert_eq!(counters, 2);
    Ok(())
}

#[rstest]
fn r6103_quinjet_mana_pays_no_spell(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = r6103_table(
        seats,
        vec![card("Quinjet Technician").unwrap()],
        vec![instant("Two Red", 0, &[Color::Red, Color::Red])],
        vec![plains(); 10],
    )?;
    r6103_tap_two(&mut driver)?;
    let id = r6103_find(&driver, "Two Red")?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority");
    };
    let castable = options.castable.contains(&id);
    if castable {
        driver.begin_cast("Two Red")?;
        let paid = driver.answer(Answer::PayMana(PayChoice::Auto));
        assert!(
            paid.is_err() || driver.state().stack().is_empty(),
            "the restricted mana paid a spell"
        );
    }
    assert_eq!(r6103_red_in_pool(&driver), 2);
    Ok(())
}

/// Seat 0 casts Meeting of the Five from a hand over a library whose top holds `top`, and the spell resolves.
fn r6103_meeting(seats: usize, top: Vec<Arc<CardDefinition>>) -> Result<Driver, ScriptError> {
    let lands = vec![
        plains(),
        island(),
        swamp(),
        mountain(),
        forest(),
        forest(),
        forest(),
        forest(),
    ];
    let mut library = top;
    library.extend((0..12).map(|_| plains()));
    let mut driver = r6103_table(
        seats,
        lands,
        vec![card("Meeting of the Five").unwrap()],
        library,
    )?;
    driver.cast("Meeting of the Five")?;
    r6103_settle(&mut driver)?;
    Ok(driver)
}

fn r6103_castable(driver: &Driver, name: &str) -> bool {
    let Ok(id) = r6103_find(driver, name) else {
        return false;
    };
    matches!(&driver.decision().unwrap().kind, DecisionKind::Priority(options) if options.castable.contains(&id))
}

#[rstest]
fn r6103_meeting_casts_only_three_colored_spells_from_among_them(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let three = creature(
        "Shard Beast",
        0,
        &[Color::White, Color::Blue, Color::Black],
        3,
        3,
    );
    let two = creature("Guild Beast", 0, &[Color::Red, Color::Green], 2, 2);
    let four = creature(
        "Nephilim",
        0,
        &[Color::White, Color::Blue, Color::Black, Color::Red],
        4,
        4,
    );
    let mut driver = r6103_meeting(seats, vec![two, three, four])?;
    for name in ["Shard Beast", "Guild Beast", "Nephilim"] {
        let id = r6103_find(&driver, name)?;
        assert_eq!(
            driver.state().object(id).unwrap().zone(),
            Zone::Exile,
            "{name}"
        );
    }
    let pool = driver.state().player(PlayerId(0)).unwrap().mana().total();
    assert_eq!(pool, 10);
    assert!(r6103_castable(&driver, "Shard Beast"));
    assert!(!r6103_castable(&driver, "Guild Beast"));
    assert!(!r6103_castable(&driver, "Nephilim"));
    driver.cast("Shard Beast")?;
    r6103_settle(&mut driver)?;
    let id = r6103_find(&driver, "Shard Beast")?;
    assert_eq!(driver.state().object(id).unwrap().zone(), Zone::Battlefield);
    assert_eq!(
        driver.state().player(PlayerId(0)).unwrap().mana().total(),
        7
    );
    Ok(())
}

#[rstest]
fn r6103_meeting_mana_pays_no_other_spell(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                plains(),
                island(),
                swamp(),
                mountain(),
                forest(),
                forest(),
                forest(),
                forest(),
            ]
            .into_iter()
            .map(ready)
            .collect::<Vec<_>>(),
        )
        .hand(
            0,
            [
                card("Meeting of the Five").unwrap(),
                creature("Guild Beast", 0, &[Color::Red, Color::Green], 2, 2),
            ],
        )
        .library(0, (0..12).map(|_| plains()).collect::<Vec<_>>());
    for seat in 1..seats {
        scenario = scenario.library(seat, (0..10).map(|_| plains()).collect::<Vec<_>>());
    }
    let mut driver = Driver::from_scenario(scenario.at(1, 0, Step::Main1))?;
    driver.cast("Meeting of the Five")?;
    r6103_settle(&mut driver)?;
    assert_eq!(
        driver.state().player(PlayerId(0)).unwrap().mana().total(),
        10
    );
    // The lands are tapped and the pool's mana is only for spells of exactly three colors.
    assert!(!r6103_castable(&driver, "Guild Beast"));
    Ok(())
}

/// Answers the upkeep of seat 0 under Plargg and Nassari: the opponent chooses `chosen` and seat 0 casts `cast`, in
/// order, saying yes to each cast.
fn r6103_plargg_upkeep(
    driver: &mut Driver,
    chosen: &str,
    cast: &[&str],
) -> Result<(), ScriptError> {
    let mut wanted: Vec<&str> = cast.to_vec();
    wanted.reverse();
    let mut chose_opponent = false;
    for _ in 0..300 {
        if driver.state().turn().step() == Step::Draw
            || (driver.state().turn().step() != Step::Upkeep
                && driver.state().turn().step() != Step::Untap)
        {
            return Ok(());
        }
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseCards(options)
                if !chose_opponent && decision.player != PlayerId(0) =>
            {
                let id = r6103_find(driver, chosen)?;
                assert!(options.from.contains(&id));
                chose_opponent = true;
                driver.answer(Answer::ChooseCards(CardsChoice(vec![id])))?;
            }
            DecisionKind::ChooseCards(options) if decision.player == PlayerId(0) => {
                let pick = wanted
                    .pop()
                    .map(|name| r6103_find(driver, name))
                    .transpose()?
                    .filter(|id| options.from.contains(id));
                driver.answer(Answer::ChooseCards(CardsChoice(pick.into_iter().collect())))?;
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("upkeep".into()))
}

#[rstest]
fn r6103_plargg_casts_up_to_two_of_the_others(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] casts: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .battlefield(0, [ready(card("Plargg and Nassari").unwrap())])
        .library(
            0,
            [mountain(), creature("Mine", 5, &[Color::Red], 2, 2)]
                .into_iter()
                .chain((0..10).map(|_| mountain())),
        );
    for seat in 1..seats {
        let name = format!("Theirs {seat}");
        scenario = scenario.library(
            seat,
            [creature(&name, 6, &[Color::Blue], 1, 1)]
                .into_iter()
                .chain((0..10).map(|_| plains())),
        );
    }
    let mut driver = Driver::from_scenario(scenario.at(1, 0, Step::Untap))?;
    let others: Vec<String> = (1..seats).map(|seat| format!("Theirs {seat}")).collect();
    let mut cast: Vec<&str> = others.iter().map(String::as_str).collect();
    cast.truncate(casts);
    r6103_plargg_upkeep(&mut driver, "Mine", &cast)?;
    r6103_settle(&mut driver)?;
    let zone = |name: &str| {
        driver
            .state()
            .object(r6103_find(&driver, name).unwrap())
            .unwrap()
            .zone()
    };
    assert_eq!(zone("Mine"), Zone::Exile, "the chosen card is not cast");
    for (index, name) in others.iter().enumerate() {
        let expected = if index < casts {
            Zone::Battlefield
        } else {
            Zone::Exile
        };
        assert_eq!(zone(name), expected, "{name}");
    }
    // The Mountain seat 0 exiled first stays in exile.
    let exiled_lands = driver
        .state()
        .zone(Zone::Exile)
        .iter()
        .filter(|id| crate::common::name_of(driver.state(), **id) == "Mountain")
        .count();
    assert_eq!(exiled_lands, 1);
    Ok(())
}

#[rstest]
fn r6103_sharkey_spends_any_type_on_its_own_abilities(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] on_sharkey: bool,
) -> Result<(), ScriptError> {
    let sharkey = "Sharkey, Tyrant of the Shire";
    let mut driver = r6103_table(
        seats,
        vec![
            card(sharkey).unwrap(),
            mtg_testkit::cards::bear(),
            mountain(),
        ],
        Vec::new(),
        vec![plains(); 10],
    )?;
    let holder = if on_sharkey { sharkey } else { "Grizzly Bears" };
    let ability: mtg_cards::ability::Ability = serde_json::from_str(
        r#"{"Activated":{"cost":"{U}","effects":[{"GainLife":{"amount":1}}]}}"#,
    )
    .unwrap();
    let id = driver.id_of(holder)?;
    let on = driver.on(id);
    driver.add_effect(
        on,
        mtg_engine::effects::Modification::GrantAbility(Arc::new(ability)),
        mtg_cards::modify::Duration::UntilEndOfTurn,
    )?;
    let life = driver.state().player(PlayerId(0)).unwrap().life();
    let activated = driver.begin_activation(holder).is_ok()
        && (!matches!(driver.decision()?.kind, DecisionKind::PayMana(_))
            || driver.answer(Answer::PayMana(PayChoice::Auto)).is_ok());
    if activated {
        r6103_settle(&mut driver)?;
    }
    let gained = driver.state().player(PlayerId(0)).unwrap().life() - life;
    // The Mountain's red mana pays {U} only for Sharkey's own ability.
    assert_eq!(gained, i32::from(on_sharkey));
    Ok(())
}
