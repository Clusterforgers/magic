//! Goal: the colors of mana spent to cast the spell of a cast trigger play as printed at tables of two, three and four
//! seats: Void Mirror counters a spell cast with colorless mana alone, whoever casts it, and lets one cast with colored
//! mana resolve; Magmablood Archaic gives its controller's creatures +1/+0 for each color spent on an instant; Rumbling
//! Aftershocks deals as much damage as its controller's spell was kicked and does not trigger on an unkicked one; Rage
//! Extractor deals damage equal to the mana value of its controller's spell with a Phyrexian symbol and ignores one
//! without; Nev, the Practical Dean grows by the X of its controller's first spell with {X} each turn and not by the
//! second; and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_cards::value::CounterKind;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{NumberChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, creature, forest, instant, mountain, phyrexian_bear, wastes};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Passes until the stack and the trigger queue are empty.
fn r407_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        match decision.kind {
            DecisionKind::ChooseTargets(_) => {
                let them = driver.target_player(1);
                driver.choose_targets(&[them])?;
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            _ => {
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

fn r407_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Seat 0's first main phase; every seat has a library.
fn r407_table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats).at(2, 0, Step::Main1);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| bear()));
    }
    scenario
}

#[rstest]
fn r407_void_mirror_counters_spells_cast_without_colored_mana(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] mirror_seat: usize,
    #[values(true, false)] colored: bool,
) -> Result<(), ScriptError> {
    let mirror = ready(card("Void Mirror").unwrap());
    let lands = [ready(forest()), ready(wastes())];
    let scenario = if mirror_seat == 0 {
        r407_table(seats).battlefield(0, [mirror, lands[0].clone(), lands[1].clone()])
    } else {
        r407_table(seats)
            .battlefield(mirror_seat, [mirror])
            .battlefield(0, lands)
    }
    .hand(0, [creature("Gray Golem", 1, &[], 2, 2)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Gray Golem")?;
    driver.pay_tapping(&[if colored { "Forest" } else { "Wastes" }], 0)?;
    r407_settle(&mut driver)?;
    let in_zone = |zone: Zone| {
        driver
            .state()
            .zone(zone)
            .iter()
            .any(|id| driver.state().object(*id).unwrap().card().name == "Gray Golem")
    };
    assert_eq!(in_zone(Zone::Battlefield), colored);
    assert_eq!(in_zone(Zone::Graveyard(pid(0))), !colored);
    r407_replays(&driver);
    Ok(())
}

#[rstest]
fn r407_magmablood_counts_the_colors_spent_on_an_instant(
    #[values(2, 3, 4)] seats: usize,
    #[values(("Forest", 2), ("Wastes", 1))] second: (&str, i32),
) -> Result<(), ScriptError> {
    let (land, colors) = second;
    let scenario = r407_table(seats)
        .battlefield(
            0,
            [
                ready(card("Magmablood Archaic").unwrap()),
                ready(bear()),
                ready(mountain()),
                ready(forest()),
                ready(wastes()),
            ],
        )
        .battlefield(1, [ready(bear())])
        .hand(0, [instant("Spark", 1, &[mtg_core::color::Color::Red])]);
    let mut driver = Driver::from_scenario(scenario)?;
    let bears: Vec<_> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| driver.state().object(*id).unwrap().card().name == "Grizzly Bears")
        .collect();
    driver.begin_cast("Spark")?;
    driver.pay_tapping(&["Mountain", land], 0)?;
    r407_settle(&mut driver)?;
    for bear in bears {
        let owner_seat = usize::from(
            mtg_engine::characteristics::controller_of(driver.state(), bear) != Some(pid(0)),
        );
        let power = characteristics(driver.state(), bear).unwrap().power;
        assert_eq!(power, Some(if owner_seat == 0 { 2 + colors } else { 2 }));
    }
    r407_replays(&driver);
    Ok(())
}

#[rstest]
fn r407_aftershocks_deal_damage_for_each_kick(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] kicks: u32,
) -> Result<(), ScriptError> {
    let mut permanents = vec![ready(card("Rumbling Aftershocks").unwrap())];
    permanents.extend((0..6).map(|_| ready(forest())));
    let scenario = r407_table(seats)
        .battlefield(0, permanents)
        .hand(0, [card("Wolfbriar Elemental").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Wolfbriar Elemental")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(kicks)))?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    let triggered = !driver.state().pending_triggers().is_empty()
        || driver.state().stack().len() > 1
        || matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_));
    assert_eq!(triggered, kicks > 0);
    r407_settle(&mut driver)?;
    let life = driver.state().player(pid(1)).unwrap().life();
    assert_eq!(life, 20 - i32::try_from(kicks).unwrap());
    r407_replays(&driver);
    Ok(())
}

#[rstest]
fn r407_rage_extractor_burns_for_spells_with_a_phyrexian_symbol(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] phyrexian: bool,
) -> Result<(), ScriptError> {
    let spell = if phyrexian { phyrexian_bear() } else { bear() };
    let name = spell.name.clone();
    let mut permanents = vec![ready(card("Rage Extractor").unwrap())];
    permanents.extend((0..3).map(|_| ready(forest())));
    let scenario = r407_table(seats)
        .battlefield(0, permanents)
        .hand(0, [spell]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast(&name)?;
    driver.pay_tapping(&["Forest", "Forest"], 0)?;
    r407_settle(&mut driver)?;
    let life = driver.state().player(pid(1)).unwrap().life();
    assert_eq!(life, if phyrexian { 18 } else { 20 });
    r407_replays(&driver);
    Ok(())
}

#[rstest]
fn r407_nev_counts_the_x_of_the_first_x_spell_each_turn(
    #[values(2, 3, 4)] seats: usize,
    #[values((2, 1), (1, 3))] xs: (u32, u32),
) -> Result<(), ScriptError> {
    let mut permanents = vec![ready(card("Nev, the Practical Dean").unwrap())];
    permanents.extend((0..4).map(|_| ready(forest())));
    let scenario = r407_table(seats).battlefield(0, permanents).hand(
        0,
        [
            card("Stonecoil Serpent").unwrap(),
            card("Stonecoil Serpent").unwrap(),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    for x in [xs.0, xs.1] {
        driver.begin_cast("Stonecoil Serpent")?;
        driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
        r407_settle(&mut driver)?;
    }
    let nev = driver.id_of("Nev, the Practical Dean")?;
    let counters = driver
        .state()
        .object(nev)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne);
    assert_eq!(counters, xs.0);
    r407_replays(&driver);
    Ok(())
}
