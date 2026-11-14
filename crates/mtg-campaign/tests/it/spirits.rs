//! Goal: the Restless Spirit of the content works in real games: it starts with one ectoplasm and gains one each of its
//! upkeeps; each haunt action costs its ectoplasm and does what its text says; Rise Again and the Phoenix Feather bring
//! a Spirit back to life; and no opponent can attack a Spirit or target it, while its teammates still can.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_campaign::content;
use mtg_cards::CardDefinition;
use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::Target;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::DecisionKind;
use mtg_engine::decision::{Answer, PriorityAction};
use mtg_engine::spirits::ectoplasm;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::brain::sim::Settings;
use mtg_testkit::brain::{HeuristicBot, SeatBrain, haunt};
use mtg_testkit::cards::{burn, free_creature, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The haunt actions' ability indices on the Restless Spirit, in the order of its text.
const WAIL: usize = 1;
const WARD: usize = 2;
const WHISPER: usize = 3;
const HINDER: usize = 4;
const HOWL: usize = 5;
const RISE: usize = 6;

const SPIRIT: &str = "Restless Spirit";

fn pid(seat: usize) -> PlayerId {
    PlayerId::from_index(seat)
}

fn custom(id: &str) -> Arc<CardDefinition> {
    content().unwrap().card(id).unwrap().definition()
}

/// Seat 0, a hero at 0 life with the Spirit rule; seat 1, its living teammate; seat 2, the boss with a 3/3 and a
/// Mountain and a Bolt in hand. Every seat has a library of twenty Mountains.
fn table() -> Scenario {
    let house = HouseRules {
        teams: Some(Teams {
            mode: TeamMode::Teams,
            seats: vec![0, 0, 1],
        }),
        ..HouseRules::default()
    };
    let mut scenario = Scenario::new(3)
        .house_rules(house)
        .life(0, 0)
        .life(1, 20)
        .life(2, 40)
        .spirit(0, custom("restless_spirit"))
        .spirit(1, custom("restless_spirit"))
        .battlefield(1, [ready(free_creature("Squire", 2, 2))])
        .battlefield(2, [ready(free_creature("Brute", 3, 3)), ready(mountain())])
        .hand(2, [burn("Bolt", 3)]);
    for seat in 0..3 {
        scenario = scenario.library(seat, (0..20).map(|_| mountain()));
    }
    scenario
}

/// The game after seat 0 fell: it is a Spirit and holds priority in its first main phase.
fn fallen() -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(table())?;
    driver.pass_until(Step::Main1)?;
    assert!(driver.state().player(pid(0)).unwrap().is_spirit());
    Ok(driver)
}

fn ectoplasm_of(driver: &Driver, seat: usize) -> u32 {
    driver
        .state()
        .player(pid(seat))
        .unwrap()
        .counters_of(&ectoplasm())
}

fn has_haunt(driver: &Driver, seat: usize) -> bool {
    let state = driver.state();
    state
        .zone(Zone::Command(pid(seat)))
        .iter()
        .filter_map(|id| state.object(*id))
        .any(|o| o.card().name == SPIRIT)
}

/// Answers passively until seat 0 holds priority in its main phase of turn `turn`.
fn main_of(driver: &mut Driver, turn: u32) -> Result<(), ScriptError> {
    driver.idle_until(turn, Step::Main1)
}

#[test]
fn a_spirit_starts_with_one_ectoplasm_and_gains_one_each_of_its_upkeeps() -> Result<(), ScriptError>
{
    let mut driver = fallen()?;
    assert_eq!(ectoplasm_of(&driver, 0), 1);
    main_of(&mut driver, 4)?;
    assert_eq!(
        ectoplasm_of(&driver, 0),
        2,
        "one more at its own upkeep only"
    );
    main_of(&mut driver, 7)?;
    assert_eq!(ectoplasm_of(&driver, 0), 3);
    Ok(())
}

#[rstest]
#[case::one_ectoplasm(1, &[WAIL, WARD, WHISPER, HINDER])]
#[case::three(3, &[WAIL, WARD, WHISPER, HINDER, HOWL])]
#[case::five(5, &[WAIL, WARD, WHISPER, HINDER, HOWL, RISE])]
fn the_actions_offered_are_the_ones_the_ectoplasm_pays_for(
    #[case] ectoplasm: u32,
    #[case] offered: &[usize],
) -> Result<(), ScriptError> {
    let mut driver = fallen()?;
    main_of(&mut driver, 1 + 3 * (ectoplasm - 1))?;
    assert_eq!(ectoplasm_of(&driver, 0), ectoplasm);
    let mut found = driver.offered_abilities(SPIRIT)?;
    found.sort_unstable();
    assert_eq!(found, offered);
    Ok(())
}

/// The ectoplasm a haunt action costs.
fn price_of(action: usize) -> u32 {
    match action {
        HOWL => 3,
        RISE => 5,
        _ => 1,
    }
}

/// The game at the fallen seat's first main phase it reaches holding exactly `held` ectoplasm: it gains one each of its
/// turns, and spends the one it starts with when it is to hold none.
fn holding(held: u32) -> Result<Driver, ScriptError> {
    let mut driver = fallen()?;
    if held == 0 {
        driver.begin_ability(SPIRIT, WARD)?;
        for _ in 0..3 {
            driver.pass()?;
        }
    } else {
        main_of(&mut driver, 1 + 3 * (held - 1))?;
    }
    assert_eq!(ectoplasm_of(&driver, 0), held);
    Ok(driver)
}

/// Chooses the targets the action just begun asks for: the boss for Wail, its Brute for Hinder and Howl.
fn aim(driver: &mut Driver, action: usize) -> Result<(), ScriptError> {
    if !matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
        return Ok(());
    }
    let target = match action {
        WAIL => driver.target_player(2),
        _ => driver.target_named("Brute")?,
    };
    driver.choose_targets(&[target])?;
    Ok(())
}

#[rstest]
#[case::wail(WAIL)]
#[case::ward(WARD)]
#[case::whisper(WHISPER)]
#[case::hinder(HINDER)]
#[case::howl(HOWL)]
#[case::rise_again(RISE)]
fn an_action_spends_its_ectoplasm_as_it_goes_on_the_stack(
    #[case] action: usize,
    #[values(0, 1)] spare: u32,
) -> Result<(), ScriptError> {
    let mut driver = holding(price_of(action) + spare)?;
    assert!(driver.offered_abilities(SPIRIT)?.contains(&action));
    driver.begin_ability(SPIRIT, action)?;
    aim(&mut driver, action)?;
    assert!(
        !driver.state().stack().is_empty(),
        "the action waits on the stack"
    );
    assert_eq!(ectoplasm_of(&driver, 0), spare, "paid before it resolves");
    Ok(())
}

#[rstest]
#[case::wail(WAIL)]
#[case::ward(WARD)]
#[case::whisper(WHISPER)]
#[case::hinder(HINDER)]
#[case::howl(HOWL)]
#[case::rise_again(RISE)]
fn an_action_with_too_little_ectoplasm_is_not_a_legal_action(
    #[case] action: usize,
) -> Result<(), ScriptError> {
    let short = price_of(action) - 1;
    let mut driver = holding(short)?;
    assert!(!driver.offered_abilities(SPIRIT)?.contains(&action));
    assert!(driver.begin_ability(SPIRIT, action).is_err());
    assert_eq!(ectoplasm_of(&driver, 0), short, "nothing was spent");
    assert_eq!(driver.state().stack().len(), 0, "nothing went on the stack");
    Ok(())
}

#[rstest]
#[case::three_wards(3, &[WARD, WARD, WARD])]
#[case::wail_then_hinder(2, &[WAIL, HINDER])]
#[case::howl_then_whisper(4, &[HOWL, WHISPER])]
#[case::whisper_then_howl(4, &[WHISPER, HOWL])]
fn activating_again_with_the_stack_full_stops_when_the_ectoplasm_runs_out(
    #[case] held: u32,
    #[case] actions: &[usize],
) -> Result<(), ScriptError> {
    let mut driver = holding(held)?;
    let mut left = held;
    for action in actions {
        driver.begin_ability(SPIRIT, *action)?;
        aim(&mut driver, *action)?;
        left -= price_of(*action);
        assert_eq!(ectoplasm_of(&driver, 0), left);
    }
    assert_eq!(left, 0);
    assert_eq!(driver.state().stack().len(), actions.len());
    assert_eq!(
        driver.offered_abilities(SPIRIT)?.len(),
        0,
        "no action is offered"
    );
    for action in [WAIL, WARD, WHISPER, HINDER, HOWL, RISE] {
        assert!(driver.begin_ability(SPIRIT, action).is_err());
    }
    Ok(())
}

#[test]
fn two_wails_drain_the_boss_twice_and_a_third_is_not_possible() -> Result<(), ScriptError> {
    let mut driver = holding(2)?;
    for _ in 0..2 {
        driver.begin_ability(SPIRIT, WAIL)?;
        aim(&mut driver, WAIL)?;
    }
    for _ in 0..6 {
        driver.pass()?;
    }
    assert_eq!(driver.state().player(pid(2)).unwrap().life(), 36);
    assert_eq!(ectoplasm_of(&driver, 0), 0);
    assert!(driver.begin_ability(SPIRIT, WAIL).is_err());
    Ok(())
}

#[test]
fn wail_drains_the_boss_and_weakens_its_creatures() -> Result<(), ScriptError> {
    let mut driver = fallen()?;
    driver.begin_ability(SPIRIT, WAIL)?;
    let boss = driver.target_player(2);
    driver.choose_targets(&[boss])?;
    driver.pass()?;
    driver.pass()?;
    driver.pass()?;
    let state = driver.state();
    assert_eq!(state.player(pid(2)).unwrap().life(), 38);
    let brute = driver.id_of("Brute")?;
    assert_eq!(characteristics(state, brute).unwrap().power, Some(2));
    assert_eq!(ectoplasm_of(&driver, 0), 0);
    Ok(())
}

#[test]
fn hinder_taps_a_boss_creature_that_stays_tapped() -> Result<(), ScriptError> {
    let mut driver = fallen()?;
    driver.begin_ability(SPIRIT, HINDER)?;
    let brute = driver.target_named("Brute")?;
    driver.choose_targets(&[brute])?;
    for _ in 0..3 {
        driver.pass()?;
    }
    let id = driver.id_of("Brute")?;
    assert!(
        driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    // Through the boss's own untap step: it is still tapped in the boss's main phase.
    driver.idle_until(3, Step::Main1)?;
    assert!(
        driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    Ok(())
}

#[test]
fn whisper_has_each_teammate_scry_two() -> Result<(), ScriptError> {
    let mut driver = fallen()?;
    driver.begin_ability(SPIRIT, WHISPER)?;
    for _ in 0..3 {
        if matches!(driver.decision()?.kind, DecisionKind::Scry(_)) {
            break;
        }
        driver.pass()?;
    }
    let decision = driver.decision()?;
    assert_eq!(decision.player, pid(1));
    let DecisionKind::Scry(options) = &decision.kind else {
        panic!("a scry for the teammate, got {}", decision.kind.name());
    };
    assert_eq!(options.cards.len(), 2);
    Ok(())
}

#[test]
fn ward_prevents_the_next_three_damage_to_each_teammate() -> Result<(), ScriptError> {
    let mut driver = fallen()?;
    driver.begin_ability(SPIRIT, WARD)?;
    for _ in 0..3 {
        driver.pass()?;
    }
    assert_eq!(ectoplasm_of(&driver, 0), 0);
    // The boss bolts the teammate in the same turn: all 3 damage is prevented.
    for _ in 0..20 {
        if driver.decision()?.player == pid(2) {
            break;
        }
        driver.pass()?;
    }
    let teammate = driver.target_player(1);
    driver.cast_at("Bolt", &[teammate])?;
    for _ in 0..3 {
        driver.pass()?;
    }
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 20);
    Ok(())
}

#[test]
fn rise_again_returns_the_spirit_with_ten_life_and_five_cards() -> Result<(), ScriptError> {
    let mut driver = fallen()?;
    main_of(&mut driver, 13)?;
    assert_eq!(ectoplasm_of(&driver, 0), 5);
    driver.begin_ability(SPIRIT, RISE)?;
    for _ in 0..3 {
        driver.pass()?;
    }
    let state = driver.state();
    let hero = state.player(pid(0)).unwrap();
    assert!(!hero.is_spirit());
    assert_eq!(hero.life(), 10);
    assert_eq!(state.zone(Zone::Hand(pid(0))).len(), 5);
    assert_eq!(ectoplasm_of(&driver, 0), 0);
    assert!(!has_haunt(&driver, 0), "the haunt emblem is gone");
    Ok(())
}

#[test]
fn the_phoenix_feather_revives_a_fallen_teammate() -> Result<(), ScriptError> {
    let scenario = table().emblems(1, [custom("phoenix_feather")]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(2, Step::Main1)?;
    assert!(driver.state().player(pid(0)).unwrap().is_spirit());
    driver.begin_ability("Phoenix Feather", 0)?;
    for _ in 0..3 {
        driver.pass()?;
    }
    let state = driver.state();
    assert!(!state.player(pid(0)).unwrap().is_spirit());
    assert_eq!(state.player(pid(0)).unwrap().life(), 8);
    assert_eq!(state.player(pid(1)).unwrap().life(), 14);
    assert_eq!(state.zone(Zone::Hand(pid(0))).len(), 4);
    Ok(())
}

#[test]
fn no_opponent_can_attack_a_spirit() -> Result<(), ScriptError> {
    let mut driver = fallen()?;
    driver.idle_until(3, Step::DeclareAttackers)?;
    let decision = driver.decision()?;
    assert_eq!(decision.player, pid(2));
    let DecisionKind::DeclareAttackers(options) = &decision.kind else {
        panic!("the boss attacks, got {}", decision.kind.name());
    };
    for attacker in &options.attackers {
        assert!(attacker.defenders.contains(&AttackTarget::Player(pid(1))));
        assert!(!attacker.defenders.contains(&AttackTarget::Player(pid(0))));
    }
    Ok(())
}

#[rstest]
#[case::by_the_boss(2, false)]
#[case::by_a_teammate(1, true)]
fn only_the_spirits_team_can_target_it(
    #[case] caster: usize,
    #[case] legal: bool,
) -> Result<(), ScriptError> {
    let scenario = table()
        .hand(1, [burn("Bolt", 3)])
        .battlefield(1, [ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::Main1)?;
    for _ in 0..20 {
        if driver.decision()?.player == pid(caster) {
            break;
        }
        driver.pass()?;
    }
    driver.begin_cast("Bolt")?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("Bolt asks for a target");
    };
    assert_eq!(options.legal.contains(&Target::Player(pid(0))), legal);
    assert!(options.legal.contains(&Target::Player(pid(1))) || caster == 1);
    Ok(())
}

/// The priority options pending for `seat`.
fn priority_of(driver: &Driver, seat: usize) -> mtg_engine::decision::PriorityOptions {
    let decision = driver.decision().unwrap();
    assert_eq!(decision.player, pid(seat));
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("priority, got {}", decision.kind.name());
    };
    (**options).clone()
}

#[test]
fn a_bot_spirit_rises_as_soon_as_it_can() -> Result<(), ScriptError> {
    let mut driver = fallen()?;
    main_of(&mut driver, 13)?;
    let options = priority_of(&driver, 0);
    let play = haunt::plan(driver.engine(), pid(0), &options).unwrap();
    assert!(matches!(play, PriorityAction::Activate(a) if a.ability == RISE));
    Ok(())
}

#[test]
fn a_bot_spirit_wards_its_team_against_a_big_attack_and_saves_otherwise() -> Result<(), ScriptError>
{
    let scenario = table().battlefield(2, [ready(free_creature("Ogre", 5, 5))]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::Main1)?;
    // Out of combat with one ectoplasm a bot Spirit saves it.
    let options = priority_of(&driver, 0);
    assert_eq!(
        haunt::plan(driver.engine(), pid(0), &options),
        Some(PriorityAction::Pass)
    );
    driver.idle_until(3, Step::DeclareAttackers)?;
    driver.attack(&[("Ogre", 1)])?;
    for _ in 0..4 {
        if driver.decision()?.player == pid(0) {
            break;
        }
        driver.pass()?;
    }
    let options = priority_of(&driver, 0);
    let play = haunt::plan(driver.engine(), pid(0), &options).unwrap();
    assert!(matches!(play, PriorityAction::Activate(a) if a.ability == WARD));
    Ok(())
}

#[test]
fn a_living_bot_with_the_phoenix_feather_revives_its_fallen_teammate() -> Result<(), ScriptError> {
    let scenario = table().emblems(1, [custom("phoenix_feather")]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(2, Step::Main1)?;
    let options = priority_of(&driver, 1);
    let play = haunt::plan(driver.engine(), pid(1), &options).unwrap();
    assert!(matches!(play, PriorityAction::Activate(a) if a.ability == 0));
    Ok(())
}

#[test]
fn the_boss_bot_attacks_only_the_living() -> Result<(), ScriptError> {
    let mut driver = fallen()?;
    driver.idle_until(3, Step::DeclareAttackers)?;
    let decision = driver.decision()?.clone();
    let mut brain = HeuristicBot::new(7, pid(2), Settings::HARD);
    let Answer::DeclareAttackers(attacks) = brain.choose(&decision, Some(driver.engine())) else {
        panic!("the boss declares attackers");
    };
    for attack in &attacks {
        assert_ne!(
            attack.1,
            AttackTarget::Player(pid(0)),
            "a Spirit is never attacked"
        );
    }
    Ok(())
}
