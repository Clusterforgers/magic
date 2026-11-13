//! Goal: the campaign's seat rules work in real games: a seat with a Spirit emblem at 0 life becomes a Spirit while a
//! teammate lives (its cards outside the library leave, its emblem arrives) and its team loses once nobody on it lives;
//! boss phases begin one by one at their life thresholds; emblems placed at setup work from the start, cost
//! modifiers on emblems included; and such games replay from their logs.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::color::Color;
use mtg_core::house::HouseRules;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::GameResult;
use mtg_engine::characteristics::characteristics;
use mtg_engine::object::ObjectKind;
use mtg_engine::phases::BossPhase;
use mtg_engine::replay;
use mtg_engine::w6b_emblems::emblem_definition;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, free_creature, instant, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Creatures you control get +1/+1.
const ANTHEM: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#;

fn emblem(name: &str, abilities: &[&str]) -> Arc<CardDefinition> {
    let abilities: Vec<Ability> = abilities
        .iter()
        .map(|a| serde_json::from_str(a).unwrap())
        .collect();
    Arc::new(emblem_definition(name, &abilities))
}

fn haunt() -> Arc<CardDefinition> {
    emblem("Haunt", &[ANTHEM])
}

/// Seats with these lives and teams, each with a Bear in play and one in hand.
fn table(lives: &[i32], teams: Option<Vec<u32>>) -> Scenario {
    let house = HouseRules {
        teams: teams.map(|seats| Teams {
            mode: TeamMode::Teams,
            seats,
        }),
        ..HouseRules::default()
    };
    let mut scenario = Scenario::new(lives.len()).house_rules(house);
    for (seat, life) in lives.iter().enumerate() {
        scenario = scenario
            .life(seat, *life)
            .battlefield(seat, [ready(free_creature(&format!("Bear {seat}"), 2, 2))])
            .hand(seat, [bear()]);
    }
    scenario
}

fn in_game(driver: &Driver, seat: usize) -> bool {
    driver.state().player(pid(seat)).unwrap().is_in_game()
}

fn emblem_names(driver: &Driver, seat: usize) -> Vec<String> {
    let state = driver.state();
    state
        .zone(Zone::Command(pid(seat)))
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.kind() == ObjectKind::Emblem)
        .map(|o| o.card().name.clone())
        .collect()
}

#[rstest]
#[case::a_hero_with_a_living_teammate_haunts(&[0, 20, 20], Some(vec![0, 0, 1]), &[0, 1], &[0], &[])]
#[case::without_a_spirit_emblem_the_hero_loses(&[0, 20, 20], Some(vec![0, 0, 1]), &[1], &[], &[0])]
#[case::without_teams_the_hero_loses(&[0, 20, 20], None, &[0, 1], &[], &[0])]
#[case::a_team_at_zero_loses_together(&[0, 0, 20], Some(vec![0, 0, 1]), &[0, 1], &[], &[0, 1])]
#[case::two_of_three_haunt_the_third(&[0, 0, 20, 20], Some(vec![0, 0, 0, 1]), &[0, 1, 2], &[0, 1], &[])]
fn a_hero_at_zero_life_becomes_a_spirit_while_a_teammate_lives(
    #[case] lives: &[i32],
    #[case] teams: Option<Vec<u32>>,
    #[case] spirit_forms: &[usize],
    #[case] spirits: &[usize],
    #[case] lost: &[usize],
) -> Result<(), ScriptError> {
    let mut scenario = table(lives, teams);
    for seat in spirit_forms {
        scenario = scenario.spirit(*seat, haunt());
    }
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    // A team that lost together ended the game before the first decision.
    if driver.state().result().is_none() {
        driver.pass()?;
    }
    for seat in 0..lives.len() {
        let player = driver.state().player(pid(seat)).unwrap();
        assert_eq!(player.is_spirit(), spirits.contains(&seat), "spirit {seat}");
        assert_eq!(
            in_game(&driver, seat),
            !lost.contains(&seat),
            "in game {seat}"
        );
    }
    for seat in spirits {
        let state = driver.state();
        assert_eq!(emblem_names(&driver, *seat), vec!["Haunt".to_owned()]);
        assert!(
            state.zone(Zone::Hand(pid(*seat))).is_empty(),
            "the hand left"
        );
        assert!(
            driver.id_of(&format!("Bear {seat}")).is_err(),
            "the board left"
        );
        assert!(
            !state.zone(Zone::Library(pid(*seat))).is_empty(),
            "the library stays"
        );
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[test]
fn a_spirit_loses_when_its_last_living_teammate_leaves() -> Result<(), ScriptError> {
    let scenario = table(&[0, 20, 20], Some(vec![0, 0, 1])).spirit(0, haunt());
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass()?;
    assert!(driver.state().player(pid(0)).unwrap().is_spirit());
    driver.concede(pid(1))?;
    assert!(!in_game(&driver, 0));
    assert_eq!(driver.state().result(), Some(GameResult::Team(1)));
    Ok(())
}

#[rstest]
#[case::above_both(20, &[])]
#[case::at_the_first(15, &["Phase II"])]
#[case::between(12, &["Phase II"])]
#[case::at_the_second(5, &["Phase II", "Phase III"])]
#[case::below_both(1, &["Phase II", "Phase III"])]
fn boss_phases_begin_in_order_at_their_thresholds(
    #[case] life: i32,
    #[case] begun: &[&str],
) -> Result<(), ScriptError> {
    let phases = [("Phase II", 15), ("Phase III", 5)].map(|(name, at_life)| BossPhase {
        at_life,
        emblem: emblem(name, &[ANTHEM]),
    });
    let scenario = table(&[20, life], None).phases(1, phases);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass()?;
    assert_eq!(emblem_names(&driver, 1), begun);
    assert_eq!(
        driver.state().player(pid(1)).unwrap().phases(),
        (2, begun.len())
    );
    // Each phase's anthem works at once.
    let bear = driver.id_of("Bear 1")?;
    let power = characteristics(driver.state(), bear)
        .unwrap()
        .power
        .unwrap();
    assert_eq!(power, 2 + i32::try_from(begun.len()).unwrap());
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[test]
fn a_setup_emblem_works_from_the_first_turn() -> Result<(), ScriptError> {
    let scenario = table(&[20, 20], None).emblems(0, [emblem("Ember Idol", &[ANTHEM])]);
    let driver = Driver::from_scenario(scenario)?;
    assert_eq!(emblem_names(&driver, 0), vec!["Ember Idol".to_owned()]);
    let power = |seat: usize| {
        let id = driver.id_of(&format!("Bear {seat}")).unwrap();
        characteristics(driver.state(), id).unwrap().power.unwrap()
    };
    assert_eq!((power(0), power(1)), (3, 2));
    Ok(())
}

/// Red spells (cast by the emblem's owner with `mine`) cost {1} less.
fn focus(mine: bool) -> String {
    let you = if mine {
        r#",{"ControlledBy":"You"}"#
    } else {
        ""
    };
    format!(
        r#"{{"CostModifier":{{"applies_to":["Spell",{{"Color":"Red"}}{you}],"change":"Reduce","mana":"{{1}}"}}}}"#
    )
}

#[rstest]
#[case::own_focus(0, true, true)]
#[case::an_opponents_own_focus(1, true, false)]
#[case::an_opponents_shared_focus(1, false, true)]
fn a_cost_modifier_on_an_emblem_changes_what_spells_cost(
    #[case] holder: usize,
    #[case] mine: bool,
    #[case] cheaper: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(2)
        .battlefield(0, [ready(mountain())])
        .hand(0, [instant("Spark", 1, &[Color::Red])])
        .emblems(holder, [emblem("Focus", &[&focus(mine)])]);
    let mut driver = Driver::from_scenario(scenario)?;
    let cast = driver.cast_tapping("Spark", &["Mountain"]);
    assert_eq!(cast.is_ok(), cheaper, "{cast:?}");
    Ok(())
}
