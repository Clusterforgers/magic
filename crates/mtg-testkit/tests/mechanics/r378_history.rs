//! Goal: facts of the turn's history play at tables of two to four seats: a creature that entered under a player's control last turn makes Ephara draw at the next upkeep and nothing else does, the mana values of the other spells cast this turn add up to Call Forth the Tempest's damage, and Glyph of Doom destroys at end of combat exactly the creatures its Wall blocked this turn; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::{cost, creature, forest, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn assert_replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

/// The corpus card, free to cast, keeping only the abilities `keep` picks.
fn free(name: &str, keep: impl Fn(usize) -> bool) -> Arc<CardDefinition> {
    let mut definition = (*card(name).unwrap()).clone();
    definition.mana_cost = Some(cost(0, &[]));
    let rules = std::mem::take(&mut definition.rules);
    definition.rules = rules
        .into_iter()
        .enumerate()
        .filter(|(at, _)| keep(*at))
        .map(|(_, rule)| rule)
        .collect();
    Arc::new(definition)
}

fn library(count: usize) -> Vec<Arc<CardDefinition>> {
    (0..count).map(|_| free_creature("Filler", 1, 1)).collect()
}

#[rstest]
#[case::a_creature_entered(true)]
#[case::nothing_entered(false)]
fn ephara_draws_after_a_turn_a_creature_entered(
    #[values(2, 3, 4)] seats: usize,
    #[case] entered: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card("Ephara, God of the Polis").unwrap())])
        .hand(0, [free_creature("Grunt", 2, 2)]);
    for seat in 0..seats {
        scenario = scenario.library(seat, library(6));
    }
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    if entered {
        driver.cast("Grunt")?;
    }
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.idle_until(2, Step::Draw)?;
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        hand + usize::from(entered),
        "Ephara draws at the next upkeep only after a creature entered"
    );
    // Nothing entered during turn 2, so the upkeep of turn 3 draws nothing.
    let before = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.idle_until(3, Step::Draw)?;
    // In a two-seat game turn 3 is seat 0's own, whose draw step has drawn its card.
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        before + usize::from(seats == 2),
        "nothing entered last turn"
    );
    assert_replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::none(&[], 0)]
#[case::one(&[3], 3)]
#[case::two(&[3, 2], 5)]
fn the_tempest_deals_the_mana_value_of_the_other_spells(
    #[values(2, 3, 4)] seats: usize,
    #[case] costs: &[u32],
    #[case] damage: u32,
) -> Result<(), ScriptError> {
    let spells: Vec<_> = costs
        .iter()
        .enumerate()
        .map(|(at, generic)| creature(&format!("Spell {at}"), *generic, &[], 1, 1))
        .collect();
    let mut hand = spells;
    hand.push(free("Call Forth the Tempest", |at| at == 2));
    let lands: Vec<_> = (0..6).map(|_| ready(forest())).collect();
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, hand)
        .battlefield(
            0,
            lands
                .into_iter()
                .chain([ready(free_creature("Mine", 0, 9))]),
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Wall", 0, 9))]);
    }
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    for at in 0..costs.len() {
        driver.cast(&format!("Spell {at}"))?;
        driver.pass()?;
        for _ in 1..seats {
            driver.pass()?;
        }
    }
    driver.cast("Call Forth the Tempest")?;
    while !driver.state().stack().is_empty() {
        driver.answer_passively()?;
    }
    let state = driver.state();
    for id in state.zone(Zone::Battlefield) {
        let object = state.object(*id).unwrap();
        if object.card().types.contains(&CardType::Creature) && object.card().name != "Spell 0" {
            let expected = if object.owner() == pid(0) { 0 } else { damage };
            assert_eq!(
                object.permanent().unwrap().damage(),
                expected,
                "{}",
                object.card().name
            );
        }
    }
    assert_replays(&driver, &setup);
    Ok(())
}

/// A 0/8 Wall creature.
fn wall() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Stone Wall")
            .with_type(CardType::Creature)
            .with_subtype("Wall")
            .with_mana_cost(cost(0, &[]))
            .with_power_toughness(0, 8),
    )
}

#[rstest]
fn glyph_of_doom_destroys_what_the_wall_blocked(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(free_creature("Blocked One", 1, 1)),
                ready(free_creature("Free One", 1, 1)),
            ],
        )
        .battlefield(1, [ready(wall())])
        .hand(1, [free("Glyph of Doom", |_| true)]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Blocked One", 1), ("Free One", 1)])?;
    driver.idle_until(1, Step::DeclareBlockers)?;
    if !matches!(
        driver.decision()?.kind,
        mtg_engine::decision::DecisionKind::DeclareBlockers(_)
    ) {
        panic!("seat 1 declares blockers");
    }
    driver.block(&[("Stone Wall", "Blocked One")])?;
    // The active seat passes, then each seat in turn until seat 1 holds priority.
    while driver.decision()?.player != pid(1) {
        driver.pass()?;
    }
    let wall = driver.target_named("Stone Wall")?;
    driver.cast_at("Glyph of Doom", &[wall])?;
    driver.idle_until(1, Step::Main2)?;
    let alive = |name: &str| driver.id_of(name).is_ok();
    assert!(!alive("Blocked One"), "the blocked creature is destroyed");
    assert!(alive("Free One"), "the unblocked creature stays");
    assert!(alive("Stone Wall"));
    assert_replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::once(1)]
#[case::twice(2)]
fn the_second_sun_wins_only_after_another_was_cast(
    #[values(2, 3, 4)] seats: usize,
    #[case] casts: usize,
) -> Result<(), ScriptError> {
    let suns: Vec<_> = (0..2)
        .map(|_| free("Approach of the Second Sun", |_| true))
        .collect();
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1).hand(0, suns);
    for seat in 0..seats {
        scenario = scenario.library(seat, library(10));
    }
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    for _ in 0..casts {
        driver.cast("Approach of the Second Sun")?;
        while !driver.state().stack().is_empty() {
            driver.answer_passively()?;
        }
    }
    let state = driver.state();
    if casts == 1 {
        assert!(state.result().is_none());
        assert_eq!(state.player(pid(0)).unwrap().life(), 27);
        let library = state.zone(Zone::Library(pid(0)));
        let seventh = library[library.len() - 7];
        assert_eq!(
            state.object(seventh).unwrap().card().name,
            "Approach of the Second Sun"
        );
    } else {
        assert!(state.result().is_some(), "the second cast from hand wins");
    }
    assert_replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::one(1)]
#[case::three(3)]
fn a_parade_gives_each_creature_one_for_each_chosen_before_it(
    #[values(2, 3, 4)] seats: usize,
    #[case] creatures: usize,
) -> Result<(), ScriptError> {
    let own: Vec<_> = (0..creatures)
        .map(|at| ready(free_creature(&format!("Marcher {at}"), 1, 1)))
        .collect();
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        own.into_iter()
            .chain([ready(card("Starlight Spectacular").unwrap())]),
    );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Watcher", 1, 1))]);
    }
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let mut line = Vec::new();
    for _ in 0..40 {
        if driver.state().turn().step() == Step::DeclareAttackers {
            break;
        }
        let decision = driver.decision()?.clone();
        match &decision.kind {
            mtg_engine::decision::DecisionKind::OrderCards(options) => {
                // The last creature first: the line is the reverse of the battlefield order.
                line = options.cards.iter().rev().copied().collect();
                driver.answer(mtg_engine::decision::Answer::OrderCards(
                    mtg_engine::choose::OrderChoice(line.clone()),
                ))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let state = driver.state();
    if creatures == 1 {
        assert!(line.is_empty(), "one creature is not asked");
    } else {
        assert_eq!(line.len(), creatures, "every creature is chosen");
    }
    for (place, id) in line.iter().enumerate() {
        let chars = mtg_engine::characteristics::characteristics(state, *id).unwrap();
        let bonus = i32::try_from(place).unwrap();
        assert_eq!(chars.power, Some(1 + bonus));
        assert_eq!(chars.toughness, Some(1 + bonus));
    }
    for id in state.zone(Zone::Battlefield) {
        let object = state.object(*id).unwrap();
        if object.card().name == "Watcher" {
            let chars = mtg_engine::characteristics::characteristics(state, *id).unwrap();
            assert_eq!(
                chars.power,
                Some(1),
                "only its controller's creatures march"
            );
        }
    }
    assert_replays(&driver, &setup);
    Ok(())
}
