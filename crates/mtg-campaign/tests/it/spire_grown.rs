//! Goal: the Drowned Spire's grown roster has a lieutenant for every colour and colour pair and two final bosses, every
//! one of the generation that grew it; and each grown boss's innate does its thing at level I and at level V against two
//! heroes in a real game.
use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_campaign::content;
use mtg_campaign::content::Boss;
use mtg_campaign::innate::innate_id;
use mtg_campaign::run::SPIRE_GROWN_FROM;
use mtg_campaign::scale::PHASE_ROUNDS;
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::phases::{BossPhase, PhaseStart};
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_engine::w6b_emblems::emblem_definition;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, creature, free_creature, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The boss's seat at the two-hero table.
const BOSS: usize = 2;

fn pid(seat: usize) -> PlayerId {
    PlayerId::from_index(seat)
}

fn spire_boss(id: &str) -> &'static Boss {
    content().unwrap().theme("spire").unwrap().boss(id).unwrap()
}

fn custom(id: &str) -> Arc<CardDefinition> {
    content().unwrap().card(id).unwrap().definition()
}

#[test]
fn the_grown_spire_covers_every_colour_and_pair_and_has_two_finals() {
    let spire = content().unwrap().theme("spire").unwrap();
    let grown: Vec<&Boss> = spire
        .bosses
        .iter()
        .filter(|b| b.since == SPIRE_GROWN_FROM)
        .collect();
    assert_eq!(grown.len(), 15);
    let sorted = |colors: &str| {
        let mut letters: Vec<char> = colors.chars().collect();
        letters.sort_unstable();
        letters.into_iter().collect::<String>()
    };
    let lieutenants: BTreeSet<String> = spire
        .bosses
        .iter()
        .filter(|b| !b.final_boss)
        .map(|b| sorted(&b.colors))
        .collect();
    let wubrg = ["W", "U", "B", "R", "G"];
    for (at, first) in wubrg.iter().enumerate() {
        assert!(lieutenants.contains(*first), "{first}");
        for second in &wubrg[at + 1..] {
            let pair = sorted(&format!("{first}{second}"));
            assert!(lieutenants.contains(&pair), "{pair}");
        }
    }
    let finals: Vec<&str> = spire
        .bosses
        .iter()
        .filter(|b| b.final_boss)
        .map(|b| b.id.as_str())
        .collect();
    assert_eq!(finals, ["leviathan", "drowned_admiral"]);
}

/// Two heroes (seats 0 and 1: a Spirit emblem, `heroes`, two untapped Mountains each, five Mountains in hand) against the boss (seat 2:
/// a ready 3/3 Brute and `board`, its innate at `level` and its round phases), all on libraries of Mountains; the heroes are one team.
fn table(
    id: &str,
    level: u8,
    heroes: &[(&str, u32, i32, i32)],
    board: Vec<PresetPermanent>,
) -> Scenario {
    let house = HouseRules {
        teams: Some(Teams {
            mode: TeamMode::Teams,
            seats: vec![0, 0, 1],
        }),
        ..HouseRules::default()
    };
    let mut scenario = Scenario::new(3).house_rules(house);
    for hero in 0..BOSS {
        let mut own: Vec<PresetPermanent> = heroes
            .iter()
            .map(|(name, mana, p, t)| ready(creature(name, *mana, &[], *p, *t)))
            .collect();
        own.extend((0..2).map(|_| ready(mountain())));
        scenario = scenario
            .life(hero, 20)
            .spirit(hero, Arc::new(emblem_definition("Haunt", &[])))
            .battlefield(hero, own)
            .hand(hero, (0..5).map(|_| mountain()));
    }
    for seat in 0..=BOSS {
        scenario = scenario.library(seat, (0..60).map(|_| mountain()));
    }
    let mut own = vec![ready(free_creature("Brute", 3, 3))];
    own.extend(board);
    let boss = spire_boss(id);
    let innate = custom(&innate_id(boss.innate.as_str(), level));
    // The round phases make the boss's seat the boss side, which the spotlight reads.
    let phases = boss
        .round_phases
        .iter()
        .zip(PHASE_ROUNDS)
        .map(|(card, round)| BossPhase {
            begins: PhaseStart::AtRound(round),
            emblem: custom(card),
        });
    scenario
        .life(BOSS, 40)
        .battlefield(BOSS, own)
        .emblems(BOSS, vec![innate])
        .phases(BOSS, phases)
}

/// Answers until `stop` holds: the boss takes as many legal targets as it may and says yes; a hero pays what it is asked
/// when `pays`, else answers passively.
fn drive(
    driver: &mut Driver,
    pays: bool,
    stop: impl Fn(&Driver) -> bool,
) -> Result<(), ScriptError> {
    for _ in 0..3_000 {
        if stop(driver) {
            return Ok(());
        }
        let decision = driver.decision()?.clone();
        let boss = decision.player == pid(BOSS);
        let answer = match &decision.kind {
            DecisionKind::ChooseTargets(o) if boss && !o.legal.is_empty() => {
                let most = usize::try_from(o.max).unwrap_or(usize::MAX);
                Answer::ChooseTargets(TargetChoice::Targets(
                    o.legal.iter().copied().take(most).collect(),
                ))
            }
            DecisionKind::YesNo(_) if boss || pays => Answer::YesNo(YesNoChoice(true)),
            DecisionKind::PayMana(_) if boss || pays => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        // A payment the hero cannot make is not offered: it answers passively instead.
        if driver.answer(answer).is_err() {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("drive".into()))
}

/// Whether the stack is empty and `seat` holds priority.
fn settled(driver: &Driver, seat: usize) -> bool {
    driver.state().stack().is_empty()
        && driver
            .decision()
            .is_ok_and(|d| d.player == pid(seat) && matches!(d.kind, DecisionKind::Priority(_)))
}

fn at(turn: u32, step: Step) -> impl Fn(&Driver) -> bool {
    move |d: &Driver| d.state().turn().number() == turn && d.state().turn().step() == step
}

/// The battlefield permanents named `name` that `seat` controls.
fn count(driver: &Driver, seat: usize, name: &str) -> i32 {
    let state = driver.state();
    let n = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.controller() == pid(seat) && o.card().name == name)
        .count();
    i32::try_from(n).unwrap()
}

/// How many of the heroes' creatures are on the battlefield, and how many of them are tapped.
fn hero_creatures(driver: &Driver) -> (i32, i32) {
    let state = driver.state();
    let mut all = 0;
    let mut tapped = 0;
    for id in state.zone(Zone::Battlefield) {
        let Some(object) = state.object(*id) else {
            continue;
        };
        let creature = characteristics(state, *id)
            .is_some_and(|c| c.types.contains(mtg_core::card::CardType::Creature));
        if object.controller().index() < BOSS && creature {
            all += 1;
            tapped += i32::from(
                object
                    .permanent()
                    .is_some_and(mtg_engine::object::PermanentState::tapped),
            );
        }
    }
    (all, tapped)
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn library(driver: &Driver, seat: usize) -> i32 {
    i32::try_from(driver.state().zone(Zone::Library(pid(seat))).len()).unwrap()
}

/// The +1/+1 counters on the boss's creatures.
fn boss_counters(driver: &Driver) -> i32 {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.controller() == pid(BOSS))
        .filter_map(|o| o.permanent())
        .map(|p| i32::try_from(p.counters_of(&CounterKind::PlusOnePlusOne)).unwrap())
        .sum()
}

/// What the innate of `id` at `level` has done, in two numbers the case names.
fn innate_effect(id: &str, level: u8) -> Result<(i32, i32), ScriptError> {
    let bears = [("Bear", 0, 2, 2), ("Wolf", 0, 2, 2)];
    let golem = [("Golem", 4, 4, 4)];
    let ox = [("Ox", 0, 3, 3)];
    Ok(match id {
        // The boss's first combat: tapped hero creatures.
        "lamplighter" | "storm_herald" => {
            let heroes: &[_] = if id == "lamplighter" { &bears } else { &golem };
            let scenario = table(id, level, heroes, Vec::new()).at(3, BOSS, Step::Main1);
            let mut driver = Driver::from_scenario(scenario)?;
            drive(&mut driver, false, at(3, Step::Main2))?;
            if id == "lamplighter" {
                (hero_creatures(&driver).1, 0)
            } else {
                (count(&driver, 0, "Golem") + count(&driver, 1, "Golem"), 0)
            }
        }
        // The boss's end step: what it put back, made or grew.
        "tidecaller" | "kelp_shepherd" | "coral_mother" => {
            let board = (0..3).map(|_| ready(free_creature("Imp", 1, 1))).collect();
            let scenario = table(id, level, &golem, board).at(3, BOSS, Step::Main1);
            let mut driver = Driver::from_scenario(scenario)?;
            drive(&mut driver, false, at(4, Step::Upkeep))?;
            match id {
                "tidecaller" => (hero_creatures(&driver).0, 0),
                "kelp_shepherd" => (count(&driver, BOSS, "Crab"), 0),
                _ => (boss_counters(&driver), 0),
            }
        }
        // The boss's upkeep: what it made, milled, burnt or took.
        "drowned_abbess" | "spawning_shoal" | "silt_crawler" | "brine_hag" | "drowned_admiral" => {
            let mut driver = Driver::from_scenario(table(id, level, &bears, Vec::new()))?;
            drive(&mut driver, false, at(2, Step::End))?;
            let before = (
                library(&driver, 0) + library(&driver, 1),
                life(&driver, 0) + life(&driver, 1),
            );
            drive(&mut driver, true, at(3, Step::Main1))?;
            match id {
                "drowned_abbess" => (4 - hero_creatures(&driver).0, 0),
                "spawning_shoal" => (count(&driver, BOSS, "Fish"), 0),
                "silt_crawler" => (before.0 - library(&driver, 0) - library(&driver, 1), 0),
                "brine_hag" => (before.1 - life(&driver, 0) - life(&driver, 1), 0),
                _ => (count(&driver, BOSS, "Zombie Pirate"), 0),
            }
        }
        // A spell the boss casts, a land it plays and a Treasure it spends each answer a hero's creature.
        "maelstrom" | "magma_vent" | "wreck_forger" => {
            let heroes: &[_] = if id == "maelstrom" { &golem } else { &ox };
            let start = if id == "wreck_forger" {
                (2, 1)
            } else {
                (3, BOSS)
            };
            let scenario = table(id, level, heroes, vec![ready(mountain())])
                .at(start.0, start.1, Step::Main1)
                .hand(BOSS, [burn("Jolt", 1), mountain()]);
            let mut driver = Driver::from_scenario(scenario)?;
            match id {
                "maelstrom" => {
                    let brute = driver.target_named("Brute")?;
                    driver.cast_at("Jolt", &[brute])?;
                }
                "magma_vent" => {
                    driver.play_land("Mountain")?;
                }
                _ => {
                    drive(&mut driver, false, at(3, Step::Main1))?;
                    driver.tap_for_mana("Treasure")?;
                }
            }
            drive(&mut driver, false, |d| settled(d, BOSS))?;
            let name = if id == "maelstrom" { "Golem" } else { "Ox" };
            (count(&driver, 0, name) + count(&driver, 1, name), 0)
        }
        // A hero's attack into the fog: the attacker's power.
        "fog_bell" => {
            let scenario = table(id, level, &bears, Vec::new());
            let mut driver = Driver::from_scenario(scenario)?;
            driver.idle_until(1, Step::DeclareAttackers)?;
            driver.attack(&[("Bear", BOSS)])?;
            let state = driver.state();
            let bear = state
                .zone(Zone::Battlefield)
                .iter()
                .copied()
                .find(|o| state.object(*o).is_some_and(|o| o.card().name == "Bear"))
                .unwrap();
            (
                characteristics(state, bear)
                    .and_then(|c| c.power)
                    .unwrap_or(0),
                0,
            )
        }
        // A hero's noncreature spell: the Inklings it gave the boss.
        "salt_magistrate" => {
            let scenario = table(id, level, &bears, Vec::new()).hand(0, [burn("Jolt", 1)]);
            let mut driver = Driver::from_scenario(scenario)?;
            let brute = driver.target_named("Brute")?;
            driver.cast_at("Jolt", &[brute])?;
            drive(&mut driver, true, |d| settled(d, 0))?;
            (count(&driver, BOSS, "Inkling"), 0)
        }
        other => panic!("no innate check for {other}"),
    })
}

#[rstest]
#[case::lamplighter_taps_one("lamplighter", 1, (1, 0))]
#[case::lamplighter_taps_three("lamplighter", 5, (3, 0))]
#[case::tidecaller_spares_mana_value_four("tidecaller", 1, (2, 0))]
#[case::tidecaller_returns_mana_value_four("tidecaller", 5, (1, 0))]
#[case::abbess_paid_off("drowned_abbess", 1, (0, 0))]
#[case::abbess_takes_a_creature("drowned_abbess", 5, (1, 0))]
#[case::wreck_forger_one_damage("wreck_forger", 1, (2, 0))]
#[case::wreck_forger_three_damage("wreck_forger", 5, (1, 0))]
#[case::coral_one_counter("coral_mother", 1, (1, 0))]
#[case::coral_three_counters("coral_mother", 5, (3, 0))]
#[case::fog_minus_one("fog_bell", 1, (1, 0))]
#[case::fog_minus_three("fog_bell", 5, (-1, 0))]
#[case::salt_tithe_paid("salt_magistrate", 1, (0, 0))]
#[case::salt_tithe_unpaid("salt_magistrate", 5, (1, 0))]
#[case::storm_herald_one_damage("storm_herald", 1, (2, 0))]
#[case::storm_herald_four_damage("storm_herald", 5, (1, 0))]
#[case::kelp_one_crab("kelp_shepherd", 1, (1, 0))]
#[case::kelp_three_crabs("kelp_shepherd", 5, (3, 0))]
#[case::maelstrom_spares_mana_value_four("maelstrom", 1, (2, 0))]
#[case::maelstrom_returns_mana_value_four("maelstrom", 5, (1, 0))]
#[case::shoal_one_fish("spawning_shoal", 1, (1, 0))]
#[case::shoal_three_fish("spawning_shoal", 5, (3, 0))]
#[case::brine_one_damage("brine_hag", 1, (1, 0))]
#[case::brine_three_damage("brine_hag", 5, (3, 0))]
#[case::silt_mills_one_each("silt_crawler", 1, (2, 0))]
#[case::silt_mills_five_each("silt_crawler", 5, (10, 0))]
#[case::vent_one_damage("magma_vent", 1, (2, 0))]
#[case::vent_three_damage("magma_vent", 5, (1, 0))]
#[case::admiral_one_pirate("drowned_admiral", 1, (1, 0))]
#[case::admiral_two_pirates("drowned_admiral", 5, (2, 0))]
fn each_grown_innate_does_its_thing_at_its_level(
    #[case] id: &str,
    #[case] level: u8,
    #[case] want: (i32, i32),
) -> Result<(), ScriptError> {
    assert_eq!(innate_effect(id, level)?, want, "{id} at level {level}");
    Ok(())
}
