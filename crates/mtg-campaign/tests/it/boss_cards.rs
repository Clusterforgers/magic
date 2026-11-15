//! Goal: the boss cards of the content do what their words say in real games: each Enrage waits for the heroes' seventh
//! turn and then grows, a toll is paid off or taken as life, Vex makes a Zombie for each hero creature that dies, Hunting
//! Party spares commanders, and the bosses' relics and the modifier pool change the creatures and costs they name.
use std::sync::Arc;

use mtg_campaign::content;
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::format::Commander;
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, creature, free_creature, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const HERO: usize = 0;
const BOSS: usize = 1;

fn pid(seat: usize) -> PlayerId {
    PlayerId::from_index(seat)
}

fn custom(id: &str) -> Arc<CardDefinition> {
    content().unwrap().card(id).unwrap().definition()
}

/// The hero (seat 0, a Bear and two Mountains) against the boss (seat 1, a Brute and a Mountain) with the content cards `ids` in its
/// command zone; both draw from libraries of Mountains.
fn table(ids: &[&str]) -> Scenario {
    Scenario::new(2)
        .life(HERO, 20)
        .life(BOSS, 40)
        .battlefield(
            HERO,
            [
                ready(free_creature("Bear", 2, 2)),
                ready(mountain()),
                ready(mountain()),
            ],
        )
        .battlefield(
            BOSS,
            [ready(free_creature("Brute", 3, 3)), ready(mountain())],
        )
        .library(HERO, (0..40).map(|_| mountain()))
        .library(BOSS, (0..40).map(|_| mountain()))
        .emblems(BOSS, ids.iter().map(|id| custom(id)))
}

/// The boss's creatures named `name`.
fn count(driver: &Driver, seat: usize, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.controller() == pid(seat) && o.card().name == name)
        .count()
}

fn power(driver: &Driver, name: &str) -> Option<i32> {
    let id = driver.id_of(name).ok()?;
    characteristics(driver.state(), id).and_then(|c| c.power)
}

/// What an Enrage has done by the boss's main phase of a turn: Rats, the Brute's power, the hero's life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Board {
    rats: usize,
    brute: i32,
    hero_life: i32,
}

fn board(driver: &Driver) -> Board {
    Board {
        rats: count(driver, BOSS, "Rat"),
        brute: power(driver, "Brute").unwrap_or(0),
        hero_life: driver.state().player(pid(HERO)).unwrap().life(),
    }
}

// Game turn 2n is the boss's nth; the hero has had n turns by then.
#[rstest]
#[case::the_swarm_waits("swarm_unending", 12, Step::Main1, Board { rats: 0, brute: 3, hero_life: 20 })]
#[case::the_swarm_begins_with_one_rat("swarm_unending", 14, Step::Main1, Board { rats: 1, brute: 3, hero_life: 20 })]
#[case::the_swarm_grows("swarm_unending", 16, Step::Main1, Board { rats: 3, brute: 3, hero_life: 20 })]
#[case::overgrowth_waits("overgrowth", 12, Step::Main1, Board { rats: 0, brute: 3, hero_life: 20 })]
#[case::overgrowth_grows_each_turn("overgrowth", 16, Step::Main1, Board { rats: 0, brute: 5, hero_life: 20 })]
#[case::the_toll_waits("deaths_toll", 12, Step::Main1, Board { rats: 0, brute: 3, hero_life: 20 })]
#[case::an_unpaid_toll_takes_life("deaths_toll", 16, Step::Main1, Board { rats: 0, brute: 3, hero_life: 14 })]
#[case::the_fury_waits("kings_fury", 12, Step::Main2, Board { rats: 0, brute: 3, hero_life: 20 })]
#[case::the_fury_grows("kings_fury", 16, Step::Main2, Board { rats: 0, brute: 5, hero_life: 20 })]
fn an_enrage_waits_for_the_heroes_seventh_turn_then_grows(
    #[case] id: &str,
    #[case] turn: u32,
    #[case] step: Step,
    #[case] expected: Board,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(&[id]))?;
    driver.idle_until(turn, step)?;
    assert_eq!(board(&driver), expected);
    Ok(())
}

#[rstest]
#[case::paid_with_mana(true, 20)]
#[case::declined(false, 17)]
fn a_toll_is_paid_off_or_taken_as_life(
    #[case] pay: bool,
    #[case] life: i32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(&["deaths_toll"]))?;
    driver.idle_until(14, Step::Upkeep)?;
    for _ in 0..40 {
        if driver.state().turn().step() != Step::Upkeep {
            break;
        }
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(pay)),
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    assert_eq!(driver.state().player(pid(HERO)).unwrap().life(), life);
    Ok(())
}

#[rstest]
#[case::a_hero_creature_dies("Bear", 1)]
#[case::its_own_creature_dies("Brute", 0)]
fn vex_raises_a_zombie_for_each_hero_creature_that_dies(
    #[case] victim: &str,
    #[case] zombies: usize,
) -> Result<(), ScriptError> {
    let scenario = table(&["vex"])
        .hand(BOSS, [burn("Bolt", 3)])
        .at(2, BOSS, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named(victim)?;
    driver.cast_at("Bolt", &[target])?;
    for _ in 0..6 {
        if count(&driver, BOSS, "Zombie") == zombies && driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    assert_eq!(count(&driver, BOSS, "Zombie"), zombies);
    Ok(())
}

#[rstest]
#[case::the_other_creature_goes(true, &["Captain"])]
#[case::a_lone_commander_stays(false, &["Captain"])]
fn hunting_party_spares_a_commander(
    #[case] with_bear: bool,
    #[case] left: &[&str],
) -> Result<(), ScriptError> {
    let captain = free_creature("Captain", 2, 2);
    let mut scenario = Scenario::new(2)
        .format(Arc::new(Commander))
        .life(HERO, 40)
        .life(BOSS, 40)
        .commanders(HERO, [Arc::clone(&captain)])
        .library(HERO, (0..20).map(|_| mountain()))
        .library(BOSS, (0..20).map(|_| mountain()))
        .emblems(BOSS, [custom("hunting_party")]);
    if with_bear {
        scenario = scenario.battlefield(HERO, [ready(free_creature("Bear", 2, 2))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::Main1)?;
    driver.cast_tapping("Captain", &[])?;
    driver.pass()?;
    driver.idle_until(2, Step::Main1)?;
    let state = driver.state();
    let mine: Vec<String> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.controller() == pid(HERO))
        .map(|o| o.card().name.clone())
        .collect();
    assert_eq!(mine, left);
    Ok(())
}

/// A creature's power, toughness and whether it has haste.
type Stats = (i32, i32, bool);

fn stats(driver: &Driver, name: &str) -> Result<Stats, ScriptError> {
    let id = driver.id_of(name)?;
    let state = driver.state();
    let c = characteristics(state, id).unwrap();
    let haste = has_keyword(state, id, &Keyword::Haste);
    Ok((c.power.unwrap_or(0), c.toughness.unwrap_or(0), haste))
}

/// A relic or modifier in the boss's command zone and what it makes of the hero's Bear and the boss's Brute.
#[rstest]
#[case::heartwood_bark_toughens_all("heartwood_bark", (2, 4, false), (3, 5, false))]
#[case::crown_of_cinders_hastens_all("crown_of_cinders", (2, 2, true), (3, 3, true))]
#[case::bloodthirst_is_the_bosses("bloodthirst", (2, 2, false), (4, 3, false))]
#[case::ironhide_is_the_bosses("ironhide", (2, 2, false), (3, 5, false))]
#[case::quickened_is_the_bosses("quickened", (2, 2, false), (3, 3, true))]
fn a_static_relic_or_modifier_changes_the_creatures_it_names(
    #[case] id: &str,
    #[case] bear: Stats,
    #[case] brute: Stats,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(table(&[id]))?;
    assert_eq!(stats(&driver, "Bear")?, bear, "{id}");
    assert_eq!(stats(&driver, "Brute")?, brute, "{id}");
    Ok(())
}

#[test]
fn the_rat_kings_crown_grows_every_rat_and_nothing_else() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(&["skrix", "rat_kings_crown"]))?;
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(stats(&driver, "Rat")?, (2, 2, false));
    assert_eq!(stats(&driver, "Brute")?, (3, 3, false));
    Ok(())
}

#[rstest]
#[case::hoarder_gains_life("hoarder", 42)]
#[case::far_sight_only_looks("far_sight", 40)]
fn an_upkeep_modifier_works_each_boss_upkeep(
    #[case] id: &str,
    #[case] life: i32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(&[id]))?;
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(driver.state().player(pid(BOSS)).unwrap().life(), life);
    Ok(())
}

#[rstest]
#[case::with_the_reliquary(true, 1)]
#[case::without_it(false, 2)]
fn the_bone_reliquary_makes_every_creature_spell_cheaper(
    #[case] reliquary: bool,
    #[case] lands: usize,
) -> Result<(), ScriptError> {
    let ids: &[&str] = if reliquary { &["bone_reliquary"] } else { &[] };
    let scenario = table(ids).hand(HERO, [creature("Ogre", 1, &[Color::Red], 3, 3)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::Main1)?;
    let names = ["Mountain", "Mountain"];
    let cast = driver.cast_tapping("Ogre", &names[..lands]);
    assert!(cast.is_ok(), "{cast:?}");
    Ok(())
}
