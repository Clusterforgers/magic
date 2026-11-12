//! Goal: the changes of control the compiler reads (Mass Mutiny, Hideous Taskmaster, Blatant Thievery, Frenzied Fugue,
//! a Threaten as an instant, Djinn of Infinite Deceits, Switcheroo, Vedalken Plotter) play through the engine at
//! tables of two, three and four seats: a creature that changes controller leaves combat, an Aura or Equipment stays
//! attached under its own controller, a creature exchanged twice ends with the right player, an exchange with an
//! illegal target or one controller does nothing, a change for the turn ends with it (untapped and hasty while it
//! lasts), a player who leaves takes their cards and their control effects with them, a stolen commander deals
//! commander damage and goes home to its owner; every answer keeps the invariants and the log replays to the state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::VecDeque;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::assemble::assemble;
use mtg_cards::compile::compile;
use mtg_cards::keyword::Keyword;
use mtg_cards::support::{PrintedCard, PrintedFace, Support};
use mtg_engine::characteristics::{characteristics, controller_of};
use mtg_engine::choose::{Target, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::format::Commander;
use mtg_engine::state::GameResult;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, free_creature, island, keyworded, mountain, plains};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::scenario::attached_to;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::pid;

type W7dResult = Result<(), ScriptError>;

/// A card assembled from its Oracle text as the server assembles a compiled card; it must be `Verified`.
pub fn w7d_oracle(
    name: &str,
    mana_cost: &str,
    type_line: &str,
    pt: Option<(i32, i32)>,
    text: &str,
) -> Arc<CardDefinition> {
    let printed = PrintedCard {
        oracle_id: format!(
            "00000000-0000-0000-0000-{:012}",
            name.len() * 7919 + text.len()
        ),
        name: name.to_owned(),
        faces: vec![PrintedFace {
            name: name.to_owned(),
            mana_cost: mana_cost.to_owned(),
            type_line: type_line.to_owned(),
            oracle_text: text.to_owned(),
            power: pt.map(|(p, _)| p.to_string()),
            toughness: pt.map(|(_, t)| t.to_string()),
            ..PrintedFace::default()
        }],
        ..PrintedCard::default()
    };
    let compiled = compile(&printed);
    let assembled = assemble(&printed, compiled.card.as_ref(), &mtg_engine::caps());
    assert_eq!(
        assembled.support.support,
        Support::Verified,
        "{name}: {:?}",
        assembled.support.clauses
    );
    Arc::new(assembled.definition)
}

/// A Threaten that is an instant, so it can be cast in the middle of combat.
fn w7d_treason() -> Arc<CardDefinition> {
    w7d_oracle(
        "Sudden Treason",
        "{R}",
        "Instant",
        None,
        "Gain control of target creature until end of turn. Untap that creature. It gains haste until end of turn.",
    )
}

fn w7d_switcheroo() -> Arc<CardDefinition> {
    w7d_oracle(
        "Switcheroo",
        "{U}",
        "Sorcery",
        None,
        "Exchange control of two target creatures.",
    )
}

fn w7d_thievery() -> Arc<CardDefinition> {
    w7d_oracle(
        "Blatant Thievery",
        "{U}",
        "Sorcery",
        None,
        "For each opponent, gain control of target permanent that player controls.",
    )
}

/// Answers until the stack is empty, nothing resolves and no trigger waits: each target decision takes up to its most
/// of `picks` (in order), mana is paid automatically, anything else is answered passively.
fn w7d_settle(driver: &mut Driver, picks: &mut VecDeque<Target>) -> W7dResult {
    w7d_answer_until(driver, picks, |driver| {
        let state = driver.state();
        state.stack().is_empty()
            && state.resolving().is_none()
            && state.pending_triggers().is_empty()
    })
}

/// Answers as [`w7d_settle`] does until a priority decision is pending where `done` holds.
fn w7d_answer_until(
    driver: &mut Driver,
    picks: &mut VecDeque<Target>,
    done: impl Fn(&Driver) -> bool,
) -> W7dResult {
    for _ in 0..300 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && done(driver) {
            return Ok(());
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseTargets(options) if !picks.is_empty() => {
                let take = (options.max as usize).min(picks.len());
                Answer::ChooseTargets(TargetChoice::Targets(picks.drain(..take).collect()))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The deciding player casts `name` at `picks` and every seat answers until the stack is empty.
fn w7d_cast(driver: &mut Driver, name: &str, picks: Vec<Target>) -> W7dResult {
    driver.begin_cast(name)?;
    w7d_settle(driver, &mut picks.into())
}

/// The deciding player casts `name` at `picks` and keeps priority with the spell on the stack.
fn w7d_announce(driver: &mut Driver, name: &str, picks: Vec<Target>) -> W7dResult {
    driver.begin_cast(name)?;
    let caster = driver.decision()?.player;
    w7d_answer_until(driver, &mut picks.into(), |driver| {
        driver.decision().is_ok_and(|d| d.player == caster)
    })
}

/// Answers passively until `seat` has a decision of the kind called `kind`.
fn w7d_until(driver: &mut Driver, seat: usize, kind: &str) -> W7dResult {
    for _ in 0..300 {
        let decision = driver.decision()?.clone();
        if decision.player == pid(seat) && decision.kind.name() == kind {
            return Ok(());
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    Err(ScriptError::Stuck(kind.into()))
}

/// The seat that controls the battlefield permanent called `name`.
fn w7d_holds(driver: &Driver, name: &str) -> usize {
    let id = driver.id_of(name).unwrap();
    controller_of(driver.state(), id).unwrap().index()
}

/// Whether the permanent called `name` is on the battlefield.
fn w7d_present(driver: &Driver, name: &str) -> bool {
    driver.id_of(name).is_ok()
}

/// Whether the permanent called `name` attacks or blocks.
fn w7d_in_combat(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .combat()
        .attackers()
        .iter()
        .any(|attacker| attacker.object() == id || attacker.blockers().contains(&id))
}

fn w7d_life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn w7d_tapped(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped()
}

/// The permanent the one called `name` is attached to, by name.
fn w7d_host(driver: &Driver, name: &str) -> Option<String> {
    let id = driver.id_of(name).unwrap();
    let host = driver.state().object(id)?.permanent()?.attached_to()?;
    Some(driver.state().object(host.id)?.card().name.clone())
}

/// The Elf of the last seat, beside the Ogre when that seat is seat 1 (a battlefield given twice keeps the last).
fn w7d_elf_beside_ogre(last: usize) -> Vec<mtg_engine::PresetPermanent> {
    let mut permanents = vec![ready(free_creature("Elf", 1, 1))];
    if last == 1 {
        permanents.push(ready(free_creature("Ogre", 3, 3)));
    }
    permanents
}

fn w7d_lands(count: usize, land: fn() -> Arc<CardDefinition>) -> Vec<mtg_engine::PresetPermanent> {
    (0..count).map(|_| ready(land())).collect()
}

#[rstest]
fn w7d_a_stolen_blocker_leaves_combat(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] trample: bool,
) -> W7dResult {
    let raider = if trample {
        keyworded("Raider", 5, 5, &[Keyword::Trample])
    } else {
        free_creature("Raider", 5, 5)
    };
    let mut mine = vec![ready(raider)];
    mine.extend(w7d_lands(1, mountain));
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .hand(0, [w7d_treason()])
        .battlefield(0, mine)
        .battlefield(1, [ready(free_creature("Wall", 2, 2))]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Raider", 1)])?;
    w7d_until(&mut driver, 1, "declare-blockers")?;
    driver.block(&[("Wall", "Raider")])?;
    w7d_until(&mut driver, 0, "priority")?;
    let wall = driver.target_named("Wall")?;
    w7d_cast(&mut driver, "Sudden Treason", vec![wall])?;
    assert_eq!(w7d_holds(&driver, "Wall"), 0);
    assert!(!w7d_in_combat(&driver, "Wall"), "it left combat (CR 506.4)");
    assert!(w7d_in_combat(&driver, "Raider"));
    driver.idle_until(2, Step::Main2)?;
    // A blocked creature stays blocked; with trample and no blocker left it assigns all its damage to the player.
    let dealt = if trample { 5 } else { 0 };
    assert_eq!(w7d_life(&driver, 1), 20 - dealt);
    let wall = driver.id_of("Wall")?;
    assert_eq!(
        driver
            .state()
            .object(wall)
            .unwrap()
            .permanent()
            .unwrap()
            .damage(),
        0
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w7d_a_stolen_attacker_leaves_combat(#[values(2, 3, 4)] seats: usize) -> W7dResult {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(free_creature("Raider", 3, 3))])
        .hand(1, [w7d_treason()])
        .battlefield(1, w7d_lands(1, mountain));
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Raider", 1)])?;
    w7d_until(&mut driver, 1, "priority")?;
    let raider = driver.target_named("Raider")?;
    w7d_cast(&mut driver, "Sudden Treason", vec![raider])?;
    assert_eq!(w7d_holds(&driver, "Raider"), 1);
    assert!(
        !w7d_in_combat(&driver, "Raider"),
        "it left combat (CR 506.4)"
    );
    assert!(!w7d_tapped(&driver, "Raider"));
    driver.idle_until(2, Step::Main2)?;
    assert_eq!(w7d_life(&driver, 1), 20);
    driver.idle_until(3, Step::Main1)?;
    assert_eq!(
        w7d_holds(&driver, "Raider"),
        0,
        "the change ended with the turn"
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w7d_a_stolen_creature_keeps_its_aura_and_equipment(
    #[values(2, 3, 4)] seats: usize,
) -> W7dResult {
    let boon = w7d_oracle(
        "Boon",
        "{W}",
        "Enchantment — Aura",
        None,
        "Enchant creature\nEnchanted creature gets +2/+2.",
    );
    let blade = w7d_oracle(
        "Blade",
        "{1}",
        "Artifact — Equipment",
        None,
        "Equipped creature gets +1/+0.\nEquip {1}",
    );
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [w7d_thievery()])
        .battlefield(0, [ready(island()), ready(free_creature("Scout", 1, 1))])
        .battlefield(
            1,
            [
                ready(free_creature("Ogre", 3, 3)),
                attached_to(ready(boon), 1, 0),
                attached_to(ready(blade), 1, 0),
                ready(plains()),
            ],
        );
    let mut picks = Vec::new();
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature(&format!("Cub {seat}"), 1, 1))]);
    }
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    picks.push(driver.target_named("Ogre")?);
    for seat in 2..seats {
        picks.push(driver.target_named(&format!("Cub {seat}"))?);
    }
    w7d_cast(&mut driver, "Blatant Thievery", picks)?;
    assert_eq!(w7d_holds(&driver, "Ogre"), 0);
    for seat in 2..seats {
        assert_eq!(w7d_holds(&driver, &format!("Cub {seat}")), 0);
    }
    for gear in ["Boon", "Blade"] {
        assert_eq!(w7d_holds(&driver, gear), 1, "{gear} keeps its controller");
        assert_eq!(w7d_host(&driver, gear).as_deref(), Some("Ogre"));
    }
    let ogre = characteristics(driver.state(), driver.id_of("Ogre")?).unwrap();
    assert_eq!((ogre.power, ogre.toughness), (Some(6), Some(5)));
    w7d_until(&mut driver, 0, "declare-attackers")?;
    assert!(
        driver.attack(&[("Ogre", 1)]).is_err(),
        "it is new to seat 0"
    );
    driver.attack(&[("Scout", 1)])?;
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::mass_mutiny("Mass Mutiny")]
#[case::hideous_taskmaster("Hideous Taskmaster")]
fn w7d_creatures_stolen_for_the_turn_attack_and_go_home(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
) -> W7dResult {
    let card = if spell == "Mass Mutiny" {
        w7d_oracle(
            spell,
            "{R}",
            "Sorcery",
            None,
            "For each opponent, gain control of up to one target creature that player controls until end of turn. Untap those creatures. They gain haste until end of turn.",
        )
    } else {
        w7d_oracle(
            spell,
            "{R}",
            "Creature — Eldrazi",
            Some((7, 2)),
            "Devoid (This card has no color.)\nWhen you cast this spell, for each opponent, gain control of up to one target creature that player controls until end of turn. Untap those creatures. They gain trample, haste, and annihilator 1 until end of turn.\nTrample, haste, annihilator 1 (Whenever this creature attacks, defending player sacrifices a permanent of their choice.)",
        )
    };
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card])
        .battlefield(0, w7d_lands(1, mountain));
    for seat in 1..seats {
        let spoil = tapped(free_creature(&format!("Spoil {seat}"), 2, 2));
        scenario = scenario.battlefield(seat, [spoil, ready(plains())]);
    }
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let names: Vec<String> = (1..seats).map(|seat| format!("Spoil {seat}")).collect();
    let picks = names
        .iter()
        .map(|name| driver.target_named(name))
        .collect::<Result<_, _>>()?;
    w7d_cast(&mut driver, spell, picks)?;
    for name in &names {
        assert_eq!(w7d_holds(&driver, name), 0, "{name}");
        assert!(!w7d_tapped(&driver, name), "{name} is untapped");
    }
    w7d_until(&mut driver, 0, "declare-attackers")?;
    let attacks: Vec<(&str, usize)> = names.iter().map(|name| (name.as_str(), 1)).collect();
    driver.attack(&attacks)?;
    driver.idle_until(1, Step::Main2)?;
    let taskmaster = spell != "Mass Mutiny";
    assert_eq!(
        w7d_life(&driver, 1),
        20 - 2 * i32::try_from(seats - 1).unwrap()
    );
    // Annihilator 1 of each stolen attacker makes seat 1 sacrifice its only permanent, its Plains.
    assert_eq!(
        driver.state().zone(Zone::Battlefield).iter().any(|id| {
            let object = driver.state().object(*id).unwrap();
            object.card().name == "Plains" && object.owner() == pid(1)
        }),
        !taskmaster
    );
    driver.idle_until(2, Step::Main1)?;
    for (seat, name) in (1..seats).zip(&names) {
        assert_eq!(w7d_holds(&driver, name), seat, "{name} went home");
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w7d_frenzied_fugue_steals_each_of_its_controllers_turns(
    #[values(2, 3, 4)] seats: usize,
) -> W7dResult {
    let fugue = w7d_oracle(
        "Frenzied Fugue",
        "{R}",
        "Enchantment — Aura",
        None,
        "Enchant permanent\nWhen this Aura enters and at the beginning of your upkeep, gain control of enchanted permanent until end of turn. Untap that permanent. It gains haste until end of turn.",
    );
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [fugue])
        .battlefield(0, w7d_lands(1, mountain))
        .battlefield(1, [tapped(free_creature("Ogre", 3, 3))]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let ogre = driver.target_named("Ogre")?;
    w7d_cast(&mut driver, "Frenzied Fugue", vec![ogre])?;
    assert_eq!(w7d_holds(&driver, "Ogre"), 0);
    assert!(!w7d_tapped(&driver, "Ogre"));
    w7d_until(&mut driver, 0, "declare-attackers")?;
    driver.attack(&[("Ogre", 1)])?;
    driver.idle_until(1, Step::Main2)?;
    assert_eq!(w7d_life(&driver, 1), 17);
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(
        w7d_holds(&driver, "Ogre"),
        1,
        "the change ended with the turn"
    );
    let next = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(next, Step::Main1)?;
    assert_eq!(
        w7d_holds(&driver, "Ogre"),
        0,
        "the upkeep trigger took it again"
    );
    assert_eq!(w7d_holds(&driver, "Frenzied Fugue"), 0);
    assert_eq!(w7d_host(&driver, "Frenzied Fugue").as_deref(), Some("Ogre"));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w7d_a_creature_exchanged_twice(#[values(2, 3, 4)] seats: usize) -> W7dResult {
    let last = seats - 1;
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [w7d_switcheroo(), w7d_switcheroo()])
        .battlefield(
            0,
            [
                ready(free_creature("Cub", 1, 1)),
                ready(island()),
                ready(island()),
            ],
        )
        .battlefield(1, [ready(free_creature("Ogre", 3, 3))])
        .battlefield(last, w7d_elf_beside_ogre(last));
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let pair = vec![driver.target_named("Cub")?, driver.target_named("Ogre")?];
    w7d_cast(&mut driver, "Switcheroo", pair)?;
    assert_eq!(
        (w7d_holds(&driver, "Cub"), w7d_holds(&driver, "Ogre")),
        (1, 0)
    );
    let pair = vec![driver.target_named("Ogre")?, driver.target_named("Elf")?];
    w7d_cast(&mut driver, "Switcheroo", pair)?;
    assert_eq!(w7d_holds(&driver, "Ogre"), last);
    assert_eq!(w7d_holds(&driver, "Elf"), 0);
    assert_eq!(w7d_holds(&driver, "Cub"), 1);
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(w7d_holds(&driver, "Ogre"), last, "an exchange lasts");
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w7d_djinn_exchanges_outside_combat_only(#[values(2, 3, 4)] seats: usize) -> W7dResult {
    let djinn = w7d_oracle(
        "Djinn of Infinite Deceits",
        "{4}{U}{U}",
        "Creature — Djinn",
        Some((2, 7)),
        "Flying\n{T}: Exchange control of two target nonlegendary creatures. You can't activate this ability during combat.",
    );
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(djinn), ready(free_creature("Cub", 1, 1))])
        .battlefield(1, [ready(free_creature("Ogre", 3, 3))]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Cub", 1)])?;
    w7d_until(&mut driver, 0, "priority")?;
    let offered = driver.offered_abilities("Djinn of Infinite Deceits")?;
    assert_eq!(offered, Vec::<usize>::new(), "not during combat");
    driver.idle_until(2, Step::Main2)?;
    let pair = vec![driver.target_named("Cub")?, driver.target_named("Ogre")?];
    driver.begin_activation("Djinn of Infinite Deceits")?;
    w7d_settle(&mut driver, &mut pair.into())?;
    assert_eq!(
        (w7d_holds(&driver, "Cub"), w7d_holds(&driver, "Ogre")),
        (1, 0)
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::a_target_dies(true)]
#[case::one_player_controls_both(false)]
fn w7d_an_exchange_that_cannot_happen_does_nothing(
    #[values(2, 3, 4)] seats: usize,
    #[case] dies: bool,
) -> W7dResult {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [w7d_switcheroo(), w7d_treason(), burn("Shock", 2)])
        .battlefield(
            0,
            [
                ready(free_creature("Cub", 1, 1)),
                ready(island()),
                ready(mountain()),
            ],
        )
        .battlefield(1, [ready(free_creature("Ogre", 2, 2))]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let pair = vec![driver.target_named("Cub")?, driver.target_named("Ogre")?];
    w7d_announce(&mut driver, "Switcheroo", pair)?;
    let ogre = driver.target_named("Ogre")?;
    w7d_cast(
        &mut driver,
        if dies { "Shock" } else { "Sudden Treason" },
        vec![ogre],
    )?;
    assert_eq!(w7d_holds(&driver, "Cub"), 0);
    assert_eq!(w7d_present(&driver, "Ogre"), !dies);
    if !dies {
        assert_eq!(w7d_holds(&driver, "Ogre"), 0);
        driver.idle_until(2, Step::Main1)?;
        assert_eq!(
            (w7d_holds(&driver, "Cub"), w7d_holds(&driver, "Ogre")),
            (0, 1)
        );
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w7d_vedalken_plotter_exchanges_lands(#[values(2, 3, 4)] seats: usize) -> W7dResult {
    let plotter = w7d_oracle(
        "Vedalken Plotter",
        "{U}",
        "Creature — Vedalken Wizard",
        Some((1, 1)),
        "When this creature enters, exchange control of target land you control and target land an opponent controls.",
    );
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [plotter])
        .battlefield(0, [ready(island()), tapped(mountain())])
        .battlefield(seats - 1, [ready(plains())]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let picks = vec![
        driver.target_named("Mountain")?,
        driver.target_named("Plains")?,
    ];
    w7d_cast(&mut driver, "Vedalken Plotter", picks)?;
    assert_eq!(w7d_holds(&driver, "Mountain"), seats - 1);
    assert_eq!(w7d_holds(&driver, "Plains"), 0);
    assert!(
        w7d_tapped(&driver, "Mountain"),
        "it keeps its tapped status"
    );
    driver.tap_for_mana("Plains")?;
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::the_thief_leaves(0)]
#[case::the_owner_leaves(1)]
fn w7d_a_player_leaves_with_a_stolen_creature(
    #[values(2, 3, 4)] seats: usize,
    #[case] leaver: usize,
) -> W7dResult {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [w7d_thievery()])
        .battlefield(0, [ready(island())])
        .battlefield(1, [ready(free_creature("Ogre", 3, 3))]);
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature(&format!("Cub {seat}"), 1, 1))]);
    }
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let mut picks = vec![driver.target_named("Ogre")?];
    for seat in 2..seats {
        picks.push(driver.target_named(&format!("Cub {seat}"))?);
    }
    w7d_cast(&mut driver, "Blatant Thievery", picks)?;
    driver.concede(pid(leaver))?;
    if seats == 2 {
        assert_eq!(
            driver.engine().result(),
            Some(GameResult::Winner(pid(1 - leaver)))
        );
    } else if leaver == 0 {
        // The control effects of the player who left end (CR 800.4a): each creature is back with its owner.
        assert_eq!(w7d_holds(&driver, "Ogre"), 1);
        for seat in 2..seats {
            assert_eq!(w7d_holds(&driver, &format!("Cub {seat}")), seat);
        }
    } else {
        assert!(
            !w7d_present(&driver, "Ogre"),
            "its owner's cards leave the game"
        );
        assert_eq!(w7d_holds(&driver, "Cub 2"), 0);
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w7d_an_exchanged_token_leaves_with_its_owner(#[values(3, 4)] seats: usize) -> W7dResult {
    let howl = w7d_oracle(
        "Howl",
        "{G}",
        "Sorcery",
        None,
        "Create a 2/2 green Wolf creature token.",
    );
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [howl, w7d_switcheroo()])
        .battlefield(0, [ready(island()), ready(mtg_testkit::cards::forest())])
        .battlefield(1, [ready(free_creature("Ogre", 3, 3))]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    w7d_cast(&mut driver, "Howl", Vec::new())?;
    let pair = vec![driver.target_named("Wolf")?, driver.target_named("Ogre")?];
    w7d_cast(&mut driver, "Switcheroo", pair)?;
    assert_eq!(
        (w7d_holds(&driver, "Wolf"), w7d_holds(&driver, "Ogre")),
        (1, 0)
    );
    driver.concede(pid(0))?;
    assert!(
        !w7d_present(&driver, "Wolf"),
        "the token its owner made leaves with them"
    );
    assert_eq!(w7d_holds(&driver, "Ogre"), 1);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w7d_a_stolen_commander_deals_commander_damage_and_goes_home(
    #[values(2, 3, 4)] seats: usize,
) -> W7dResult {
    let scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .at(1, 1, Step::Main1)
        .commanders(1, [legend("Cmd", 0, &[], 3, 3)])
        .hand(0, [w7d_treason(), burn("Bolt", 3)])
        .battlefield(0, w7d_lands(2, mountain));
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Cmd")?;
    w7d_settle(&mut driver, &mut VecDeque::new())?;
    let mine = u32::try_from(seats).unwrap();
    driver.idle_until(mine, Step::Main1)?;
    let cmd = driver.id_of("Cmd")?;
    let target = driver.target_named("Cmd")?;
    w7d_cast(&mut driver, "Sudden Treason", vec![target])?;
    w7d_until(&mut driver, 0, "declare-attackers")?;
    driver.attack(&[("Cmd", 1)])?;
    driver.idle_until(mine, Step::Main2)?;
    let life = driver.state().player(pid(1)).unwrap().life();
    assert_eq!(driver.state().commander_damage(cmd, pid(1)), 3);
    assert_eq!(life, 37, "a Commander table starts at 40");
    let target = driver.target_named("Cmd")?;
    w7d_announce(&mut driver, "Bolt", vec![target])?;
    let mut asked = None;
    for _ in 0..20 {
        let decision = driver.decision()?.clone();
        if let DecisionKind::YesNo(_) = decision.kind {
            asked = Some(decision.player);
            driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            break;
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    assert_eq!(asked, Some(pid(1)), "the owner chooses the command zone");
    let commander = driver.state().commanders().next().unwrap();
    assert_eq!(
        driver.state().object(commander).unwrap().zone(),
        Zone::Command(pid(1))
    );
    replays(&driver, &setup);
    Ok(())
}
