//! Goal: the r30-6c cards play as printed at tables of two to four seats: Rose makes a Junk per opponent attacked at end of
//! combat; Acrobatic Cheerleader's survival trigger gives one flying counter only once; Mana-Charged Dragon gets +X/+0 for
//! the mana every player pays; Craig Boone's reflexive damage goes to the creature or, at its controller's choice, to
//! them; Earthbender Ascension pumps only from its fourth quest counter; The Fantastic Four's modes run once each turn
//! and trigger on spells with power, toughness or mana value 4; Brazen Cannonade's raid card stays playable until
//! combat ends on its controller's next turn.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::characteristics;
use mtg_engine::choose::{ModeChoice, NumberChoice, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice};
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, cost, forest, free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped, with_counters};
use rstest::rstest;

use crate::common::{names, pid};

/// Answers until seat 0 has priority with an empty stack: `custom` answers first when it returns an answer, mana is paid
/// automatically and anything else is answered passively.
fn r306c_settle(
    driver: &mut Driver,
    custom: &mut dyn FnMut(&Driver, &Decision) -> Option<Answer>,
) -> Result<(), ScriptError> {
    for _ in 0..120 {
        let decision = driver.decision()?.clone();
        if let Some(answer) = custom(driver, &decision) {
            driver.answer(answer)?;
            continue;
        }
        match &decision.kind {
            DecisionKind::Priority(_)
                if decision.player == pid(0)
                    && driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty()
                    && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

fn r306c_count(driver: &Driver, name: &str) -> usize {
    names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|n| *n == name)
        .count()
}

fn r306c_power(driver: &Driver, name: &str) -> i32 {
    let id = driver.id_of(name).unwrap();
    characteristics(driver.state(), id).unwrap().power.unwrap()
}

fn r306c_counters(driver: &Driver, name: &str, kind: &CounterKind) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| permanent.counters_of(kind))
}

fn r306c_life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn quest() -> CounterKind {
    CounterKind::Other("quest".to_owned())
}

/// Seat 0 attacks with `attacks` on turn 1 and the game goes on to its postcombat main phase.
fn r306c_attack(driver: &mut Driver, attacks: &[(&str, usize)]) -> Result<(), ScriptError> {
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(attacks)?;
    driver.pass_until(Step::Main2)?;
    r306c_settle(driver, &mut |_, _| None)
}

#[rstest]
fn rose_makes_a_junk_for_each_opponent_attacked(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3)] attacked: usize,
) -> Result<(), ScriptError> {
    let attacked = attacked.min(seats - 1);
    let mut attackers = vec![ready(card("Rose, Cutthroat Raider").unwrap())];
    attackers.extend((0..3).map(|i| ready(free_creature(&format!("Raider {i}"), 1, 1))));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, attackers);
    let mut driver = Driver::from_scenario(scenario)?;
    let names: Vec<String> = (0..attacked).map(|i| format!("Raider {i}")).collect();
    let attacks: Vec<(&str, usize)> = names
        .iter()
        .enumerate()
        .map(|(i, name)| (name.as_str(), i + 1))
        .collect();
    r306c_attack(&mut driver, &attacks)?;
    assert_eq!(r306c_count(&driver, "Junk"), attacked);
    Ok(())
}

#[rstest]
fn rose_counts_each_opponent_once(#[values(3, 4)] seats: usize) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(card("Rose, Cutthroat Raider").unwrap()),
            ready(free_creature("A", 1, 1)),
            ready(free_creature("B", 1, 1)),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    r306c_attack(
        &mut driver,
        &[("A", 1), ("B", 1), ("Rose, Cutthroat Raider", 2)],
    )?;
    assert_eq!(r306c_count(&driver, "Junk"), 2);
    Ok(())
}

#[rstest]
fn the_cheerleader_gets_one_flying_counter_only_once(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [tapped(card("Acrobatic Cheerleader").unwrap())]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::Main2)?;
    r306c_settle(&mut driver, &mut |_, _| None)?;
    let flying = CounterKind::Other("flying".to_owned());
    assert_eq!(r306c_counters(&driver, "Acrobatic Cheerleader", &flying), 1);
    let id = driver.id_of("Acrobatic Cheerleader")?;
    assert!(has_keyword(driver.state(), id, &Keyword::Flying));
    // Its next turn it attacks and is tapped again in the second main phase: no second counter.
    let next = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(next, Step::Main1)?;
    r306c_attack(&mut driver, &[("Acrobatic Cheerleader", 1)])?;
    assert_eq!(r306c_counters(&driver, "Acrobatic Cheerleader", &flying), 1);
    Ok(())
}

#[rstest]
fn an_untapped_cheerleader_gets_no_counter(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card("Acrobatic Cheerleader").unwrap())]);
    let mut driver = Driver::from_scenario(scenario)?;
    r306c_attack(&mut driver, &[])?;
    let flying = CounterKind::Other("flying".to_owned());
    assert_eq!(r306c_counters(&driver, "Acrobatic Cheerleader", &flying), 0);
    Ok(())
}

/// Each seat pays `pays[seat]` for the dragon's join forces (0 declines).
fn dragon_answers(pays: Vec<u32>) -> impl FnMut(&Driver, &Decision) -> Option<Answer> {
    move |_, decision| {
        let paying = pays[decision.player.index()];
        match &decision.kind {
            DecisionKind::YesNo(options) if options.text.starts_with("Pay") => {
                Some(Answer::YesNo(YesNoChoice(paying > 0)))
            }
            DecisionKind::ChooseNumber(_) => {
                Some(Answer::ChooseNumber(NumberChoice::Number(paying)))
            }
            _ => None,
        }
    }
}

#[rstest]
#[case::two_pay(2, vec![1, 2])]
#[case::one_declines(3, vec![2, 0, 1])]
#[case::nobody_pays(4, vec![0, 0, 0, 0])]
#[case::all_pay(4, vec![1, 1, 2, 3])]
fn the_dragon_gets_the_mana_every_player_paid(
    #[case] seats: usize,
    #[case] pays: Vec<u32>,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1);
    for seat in 0..seats {
        let mut permanents: Vec<_> = (0..3).map(|_| ready(mountain())).collect();
        if seat == 0 {
            permanents.push(ready(card("Mana-Charged Dragon").unwrap()));
        }
        scenario = scenario.battlefield(seat, permanents);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Mana-Charged Dragon", 1)])?;
    let total: u32 = pays.iter().sum();
    let mut answers = dragon_answers(pays);
    r306c_settle(&mut driver, &mut answers)?;
    assert_eq!(
        r306c_power(&driver, "Mana-Charged Dragon"),
        5 + i32::try_from(total).unwrap()
    );
    Ok(())
}

#[rstest]
fn craig_boone_damages_the_creature_or_its_controller(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] takes_it: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Craig Boone, Novac Guard").unwrap()),
                ready(bear()),
            ],
        )
        .battlefield(seats - 1, [ready(free_creature("Target", 2, 2))]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Craig Boone, Novac Guard", 1), ("Grizzly Bears", 1)])?;
    let target = driver.target_named("Target")?;
    let owner = seats - 1;
    r306c_settle(&mut driver, &mut |_, decision| match &decision.kind {
        DecisionKind::ChooseTargets(_) if decision.player == pid(0) => {
            Some(Answer::ChooseTargets(TargetChoice::Targets(vec![target])))
        }
        DecisionKind::YesNo(_) if decision.player == pid(owner) => {
            Some(Answer::YesNo(YesNoChoice(takes_it)))
        }
        _ => None,
    })?;
    assert_eq!(
        r306c_counters(&driver, "Craig Boone, Novac Guard", &quest()),
        2
    );
    if takes_it {
        assert_eq!(r306c_life(&driver, owner), 18);
        assert_eq!(r306c_count(&driver, "Target"), 1);
    } else {
        assert_eq!(r306c_life(&driver, owner), 20);
        assert_eq!(r306c_count(&driver, "Target"), 0);
    }
    Ok(())
}

#[rstest]
fn craig_boone_needs_two_attackers(#[values(2, 3)] seats: usize) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card("Craig Boone, Novac Guard").unwrap())]);
    let mut driver = Driver::from_scenario(scenario)?;
    r306c_attack(&mut driver, &[("Craig Boone, Novac Guard", 1)])?;
    assert_eq!(
        r306c_counters(&driver, "Craig Boone, Novac Guard", &quest()),
        0
    );
    Ok(())
}

#[rstest]
#[case::third_counter(2, false)]
#[case::fourth_counter(3, true)]
#[case::later_counter(6, true)]
fn earthbender_ascension_pumps_from_its_fourth_quest_counter(
    #[case] held: u32,
    #[case] pumps: bool,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                with_counters(ready(card("Earthbender Ascension").unwrap()), quest(), held),
                ready(bear()),
            ],
        )
        .hand(0, [forest()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.play_land("Forest")?;
    r306c_settle(&mut driver, &mut |_, _| None)?;
    assert_eq!(
        r306c_counters(&driver, "Earthbender Ascension", &quest()),
        held + 1
    );
    let bear_id = driver.id_of("Grizzly Bears")?;
    assert_eq!(
        r306c_power(&driver, "Grizzly Bears"),
        if pumps { 3 } else { 2 }
    );
    assert_eq!(
        has_keyword(driver.state(), bear_id, &Keyword::Trample),
        pumps
    );
    Ok(())
}

/// A creature card with this mana value, power and toughness.
fn r306c_creature(name: &str, value: u32, power: i32, toughness: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(cost(value, &[]))
            .with_power_toughness(power, toughness),
    )
}

/// Chooses the first mode offered and records how many modes were offered each time.
fn first_mode(offered: &mut Vec<usize>) -> impl FnMut(&Driver, &Decision) -> Option<Answer> + '_ {
    move |_, decision| match &decision.kind {
        DecisionKind::ChooseMode(options) => {
            offered.push(options.modes.len());
            Some(Answer::ChooseMode(ModeChoice::Modes(vec![
                options.modes[0].index,
            ])))
        }
        _ => None,
    }
}

#[rstest]
#[case::mana_value_four("Value Four", 4, 1, 1, true)]
#[case::power_four("Power Four", 1, 4, 1, true)]
#[case::toughness_four("Toughness Four", 1, 1, 4, true)]
#[case::none_four("Three Three", 3, 3, 3, false)]
fn the_fantastic_four_triggers_on_a_spell_with_a_four(
    #[case] name: &str,
    #[case] value: u32,
    #[case] power: i32,
    #[case] toughness: i32,
    #[case] triggers: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut permanents: Vec<_> = (0..4).map(|_| ready(mountain())).collect();
    permanents.push(ready(card("The Fantastic Four").unwrap()));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, permanents)
        .hand(0, [r306c_creature(name, value, power, toughness)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast(name)?;
    let mut offered = Vec::new();
    r306c_settle(&mut driver, &mut first_mode(&mut offered))?;
    assert_eq!(offered.len(), usize::from(triggers));
    // The first mode makes a 0/4 Wall with defender.
    assert_eq!(r306c_count(&driver, "Wall"), usize::from(triggers));
    Ok(())
}

#[rstest]
fn the_fantastic_four_modes_run_once_each_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut permanents: Vec<_> = (0..8).map(|_| ready(mountain())).collect();
    permanents.push(ready(card("The Fantastic Four").unwrap()));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, permanents)
        .hand(
            0,
            [
                r306c_creature("First", 4, 1, 1),
                r306c_creature("Second", 4, 1, 1),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    let mut offered = Vec::new();
    driver.cast("First")?;
    r306c_settle(&mut driver, &mut first_mode(&mut offered))?;
    driver.cast("Second")?;
    r306c_settle(&mut driver, &mut first_mode(&mut offered))?;
    assert_eq!(offered, vec![4, 3]);
    // The Wall, then 3 damage to each opponent.
    assert_eq!(r306c_count(&driver, "Wall"), 1);
    for seat in 1..seats {
        assert_eq!(r306c_life(&driver, seat), 17);
    }
    assert_eq!(r306c_life(&driver, 0), 20);
    Ok(())
}

/// Seat 0 has Brazen Cannonade and a creature; its library is Forests.
fn cannonade(seats: usize) -> Scenario {
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card("Brazen Cannonade").unwrap()), ready(bear())])
        .library(0, (0..6).map(|_| forest()))
}

fn exiled(driver: &Driver) -> usize {
    driver.state().zone(Zone::Exile).len()
}

#[rstest]
fn brazen_cannonade_exiles_after_an_attack_and_lets_it_be_played(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] attacks: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(cannonade(seats))?;
    let attackers: &[(&str, usize)] = if attacks {
        &[("Grizzly Bears", 1)]
    } else {
        &[]
    };
    r306c_attack(&mut driver, attackers)?;
    assert_eq!(exiled(&driver), usize::from(attacks));
    if attacks {
        driver.play_land("Forest")?;
        assert_eq!(exiled(&driver), 0);
        assert_eq!(r306c_count(&driver, "Forest"), 1);
    }
    Ok(())
}

#[rstest]
#[case::before_combat_next_turn(Step::Main1, true)]
#[case::after_combat_next_turn(Step::Main2, false)]
fn brazen_cannonade_lets_it_be_played_until_combat_on_the_next_turn(
    #[values(2, 3)] seats: usize,
    #[case] then: Step,
    #[case] playable: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(cannonade(seats))?;
    r306c_attack(&mut driver, &[("Grizzly Bears", 1)])?;
    assert_eq!(exiled(&driver), 1);
    let next = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(next, then)?;
    r306c_settle(&mut driver, &mut |_, _| None)?;
    let id = driver.state().zone(Zone::Exile)[0];
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("seat 0 has priority");
    };
    assert_eq!(options.playable_lands.contains(&id), playable);
    Ok(())
}
