//! Goal: a payment of any amount is chosen by its payer as it resolves and the steps after it read the amount paid, at tables of two, three and four seats: energy spells deal, shrink and destroy by the energy paid (nothing when declined), Necrodominance draws the life paid, Liege of the Hollows gives each player the Squirrels of the mana that player paid, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{NumberChoice, Target, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::{forest, giant, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

fn energy() -> CounterKind {
    CounterKind::Other("energy".into())
}

/// Answers until nothing waits: each player pays the amount `plan` gives them (declines on `None`), the targets are
/// `target` or the first legal ones, X of a spell is `x`, and mana is paid automatically.
fn l31b_settle(
    driver: &mut Driver,
    x: u32,
    plan: &dyn Fn(PlayerId) -> Option<u32>,
    target: Option<Target>,
) -> Result<(), ScriptError> {
    for _ in 0..120 {
        let decision = driver.decision()?.clone();
        let state = driver.state();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && state.stack().is_empty()
            && state.resolving().is_none()
        {
            return Ok(());
        }
        let resolving = state.resolving().is_some();
        let answer = match &decision.kind {
            DecisionKind::ChooseNumber(_) if !resolving => {
                Answer::ChooseNumber(NumberChoice::Number(x))
            }
            DecisionKind::ChooseNumber(options) => {
                let paid = plan(decision.player).unwrap_or(0);
                assert!(
                    paid <= options.max.unwrap_or(0),
                    "{paid} within {options:?}"
                );
                Answer::ChooseNumber(NumberChoice::Number(paid))
            }
            DecisionKind::YesNo(_) if resolving => {
                Answer::YesNo(YesNoChoice(plan(decision.player).is_some()))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::ChooseTargets(options) => {
                let chosen = match target {
                    Some(target) => vec![target],
                    None => options.legal.iter().take(1).copied().collect(),
                };
                Answer::ChooseTargets(TargetChoice::Targets(chosen))
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the stack did not empty".into()))
}

fn l31b_replays(setup: &GameSetup, driver: &Driver) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn energy_of(driver: &Driver, seat: usize) -> u32 {
    driver
        .state()
        .player(pid(seat))
        .unwrap()
        .counters_of(&energy())
}

fn names_on_battlefield(driver: &Driver, seat: usize) -> Vec<String> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.controller() == pid(seat))
        .map(|object| object.card().name.clone())
        .collect()
}

#[rstest]
#[case::lightning("Harnessed Lightning", 3)]
#[case::discharge("Galvanic Discharge", 3)]
fn l31b_energy_bolts_deal_the_energy_paid(
    #[case] spell: &str,
    #[case] gained: u32,
    #[values(None, Some(0), Some(2), Some(3), Some(5))] pay: Option<u32>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let held = 2;
    let scenario = || {
        let last = seats - 1;
        Scenario::new(seats)
            .at(4, 0, Step::Main1)
            .hand(0, [card(spell).unwrap()])
            .battlefield(0, vec![ready(mountain()); 2])
            .counters(0, energy(), held)
            .battlefield(last, [ready(giant())])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let wurm = driver.target_named("Craw Wurm")?;
    driver.cast_at(spell, &[wurm])?;
    l31b_settle(&mut driver, 0, &|_| pay, None)?;
    let paid = pay.unwrap_or(0);
    // The Wurm is a 4/4: four or more damage destroys it.
    let alive = names_on_battlefield(&driver, seats - 1).contains(&"Craw Wurm".to_owned());
    assert_eq!(alive, paid < 4, "{pay:?}");
    assert_eq!(energy_of(&driver, 0), held + gained - paid);
    l31b_replays(&setup, &driver);
    Ok(())
}

#[rstest]
fn l31b_die_young_shrinks_by_the_energy_paid(
    #[values(None, Some(1), Some(2), Some(3))] pay: Option<u32>,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .at(4, 0, Step::Main1)
            .hand(0, [card("Die Young").unwrap()])
            .battlefield(0, vec![ready(swamp()); 2])
            .counters(0, energy(), 1)
            .battlefield(1, [ready(giant())])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let wurm = driver.target_named("Craw Wurm")?;
    driver.cast_at("Die Young", &[wurm])?;
    l31b_settle(&mut driver, 0, &|_| pay, None)?;
    let paid = pay.unwrap_or(0);
    let id = driver.id_of("Craw Wurm")?;
    let chars = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    let less = i32::try_from(paid).unwrap();
    assert_eq!(
        (chars.power, chars.toughness),
        (Some(4 - less), Some(4 - less))
    );
    l31b_replays(&setup, &driver);
    Ok(())
}

#[rstest]
#[case::declined(3, None)]
#[case::none(3, Some(0))]
#[case::two(3, Some(2))]
#[case::more_than_x(1, Some(4))]
fn l31b_wrath_of_the_skies_destroys_up_to_the_energy_paid(
    #[case] x: u32,
    #[case] pay: Option<u32>,
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let held = 3;
    let scenario = || {
        let mut lands = vec![ready(plains()); 2];
        lands.extend((0..x).map(|_| ready(island())));
        Scenario::new(seats)
            .at(4, 0, Step::Main1)
            .hand(0, [card("Wrath of the Skies").unwrap()])
            .battlefield(0, lands)
            .counters(0, energy(), held)
            .battlefield_named(1, ["Llanowar Elves", "Grizzly Bears", "Serra Angel"])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Wrath of the Skies")?;
    l31b_settle(&mut driver, x, &|_| pay, None)?;
    let paid = pay.unwrap_or(0);
    let left = names_on_battlefield(&driver, 1);
    for (name, value) in [
        ("Llanowar Elves", 1),
        ("Grizzly Bears", 2),
        ("Serra Angel", 5),
    ] {
        assert_eq!(
            left.contains(&name.to_owned()),
            value > paid,
            "{name} after {pay:?}"
        );
    }
    assert_eq!(energy_of(&driver, 0), held + x - paid);
    l31b_replays(&setup, &driver);
    Ok(())
}

#[rstest]
fn l31b_necrodominance_draws_the_life_paid(
    #[values(None, Some(0), Some(3), Some(5))] pay: Option<u32>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .at(4, 0, Step::Main1)
            .battlefield(0, [ready(card("Necrodominance").unwrap())])
            .library(0, vec![island(); 10])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let hand = |driver: &Driver| driver.state().zone(Zone::Hand(pid(0))).len();
    let life = |driver: &Driver| driver.state().player(pid(0)).unwrap().life();
    let (cards, total) = (hand(&driver), life(&driver));
    driver.pass_until(Step::End)?;
    l31b_settle(&mut driver, 0, &|_| pay, None)?;
    let paid = pay.unwrap_or(0);
    assert_eq!(hand(&driver), cards + paid as usize);
    assert_eq!(total - life(&driver), i32::try_from(paid).unwrap());
    l31b_replays(&setup, &driver);
    Ok(())
}

#[rstest]
#[case::two(2, [Some(2), Some(1), None, None])]
#[case::three(3, [None, Some(3), Some(0), None])]
#[case::four(4, [Some(1), None, Some(2), Some(3)])]
fn l31b_liege_gives_each_player_the_squirrels_of_their_payment(
    #[case] seats: usize,
    #[case] plan: [Option<u32>; 4],
) -> Result<(), ScriptError> {
    let scenario = || {
        let mut mine = vec![ready(card("Liege of the Hollows").unwrap())];
        mine.extend((0..2).map(|_| ready(swamp())));
        mine.extend((0..3).map(|_| ready(forest())));
        let mut table = Scenario::new(seats)
            .at(4, 0, Step::Main1)
            .hand(0, [card("Doom Blade").unwrap()])
            .battlefield(0, mine);
        for seat in 1..seats {
            table = table.battlefield(seat, vec![ready(forest()); 3]);
        }
        table
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    // Doom Blade is paid with the two Swamps, so each player still holds three Forests.
    let liege = driver.target_named("Liege of the Hollows")?;
    driver.begin_cast("Doom Blade")?;
    driver.choose_targets(&[liege])?;
    driver.pay_tapping(&["Swamp", "Swamp"], 0)?;
    l31b_settle(&mut driver, 0, &|player| plan[player.index()], None)?;
    for (seat, paid) in plan.iter().enumerate().take(seats) {
        let squirrels = names_on_battlefield(&driver, seat)
            .iter()
            .filter(|name| *name == "Squirrel")
            .count();
        assert_eq!(squirrels, paid.unwrap_or(0) as usize, "seat {seat}");
    }
    l31b_replays(&setup, &driver);
    Ok(())
}
