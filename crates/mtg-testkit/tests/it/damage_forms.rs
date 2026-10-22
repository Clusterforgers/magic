//! Goal: wither, infect and toxic change what damage does and nothing else: a creature a source with wither or infect hurts gets that many -1/-1 counters and no damage marked, a player an infect source hurts gets that many poison counters and loses no life, toxic gives a player dealt combat damage by it its poison counters on top (several instances add up, none for a blocked creature or for damage that was all prevented, some for damage that was partly prevented), non-combat damage from a source with infect or wither follows the same rule and toxic does not apply to it, lifelink and deathtouch still work, prevention shields and Fog act on the damage before it becomes counters, "whenever an opponent loses life" does not see infect damage, +1/+1 and -1/-1 counters cancel, ten poison counters lose the game, and the rules hold at two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::PresetPermanent;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, keyworded};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

fn filler() -> impl Iterator<Item = Arc<CardDefinition>> {
    (0..10).map(|_| free_creature("Filler", 1, 1))
}

/// Seat 0 about to declare attackers with "A"; seat 1 holds `blockers`; every seat has a library.
fn duel(
    seats: usize,
    attacker: Arc<CardDefinition>,
    blockers: Vec<PresetPermanent>,
) -> Result<Driver, ScriptError> {
    Driver::from_scenario(scenario(seats, attacker, blockers).at(2, 0, Step::DeclareAttackers))
}

fn scenario(
    seats: usize,
    attacker: Arc<CardDefinition>,
    blockers: Vec<PresetPermanent>,
) -> Scenario {
    scenario_with(seats, vec![ready(attacker)], blockers)
}

/// As [`scenario`], with these permanents for seat 0.
fn scenario_with(
    seats: usize,
    mine: Vec<PresetPermanent>,
    blockers: Vec<PresetPermanent>,
) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .battlefield(0, mine)
        .battlefield(1, blockers);
    for seat in 0..seats {
        scenario = scenario.library(seat, filler());
    }
    scenario
}

/// Plays on to the end of the combat, blocking as `blocks` says and answering passively otherwise.
fn through_combat(driver: &mut Driver, blocks: &[(&str, &str)]) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().turn().step() == Step::Main2
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            return Ok(());
        }
        match &driver.decision()?.kind {
            DecisionKind::DeclareBlockers(_) => {
                driver.block(blocks)?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("combat".into()))
}

/// "A" attacks seat `defender`, `blocks` are declared and the combat is played through.
fn fight(driver: &mut Driver, defender: usize, blocks: &[(&str, &str)]) -> Result<(), ScriptError> {
    driver.attack(&[("A", defender)])?;
    through_combat(driver, blocks)
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(common::pid(seat)).unwrap().life()
}

fn poison(driver: &Driver, seat: usize) -> u32 {
    driver
        .state()
        .player(common::pid(seat))
        .unwrap()
        .counters_of(&CounterKind::Poison)
}

/// The permanent called `name` on the battlefield, if it is there.
fn permanent<'a>(driver: &'a Driver, name: &str) -> Option<&'a mtg_engine::object::PermanentState> {
    let id = common::on_battlefield(driver.state(), name)?;
    driver.state().object(id)?.permanent()
}

/// What happened to a creature: damage marked, -1/-1 counters, power and toughness; `None` once it is gone.
fn fate(driver: &Driver, name: &str) -> Option<(u32, u32, (i32, i32))> {
    let permanent = permanent(driver, name)?;
    let id = common::on_battlefield(driver.state(), name)?;
    let chars = characteristics(driver.state(), id)?;
    Some((
        permanent.damage(),
        permanent.counters_of(&CounterKind::MinusOneMinusOne),
        (chars.power?, chars.toughness?),
    ))
}

fn attacker(power: i32, keywords: &[Keyword]) -> Arc<CardDefinition> {
    keyworded("A", power, 9, keywords)
}

#[rstest]
#[case::plain(&[], 3, 17, 0, 20)]
#[case::infect(&[Keyword::Infect], 3, 20, 3, 20)]
#[case::wither(&[Keyword::Wither], 3, 17, 0, 20)]
#[case::toxic(&[Keyword::Toxic(1)], 3, 17, 1, 20)]
#[case::toxic_three(&[Keyword::Toxic(3)], 2, 18, 3, 20)]
#[case::two_instances_of_toxic_add_up(&[Keyword::Toxic(1), Keyword::Toxic(2)], 3, 17, 3, 20)]
#[case::infect_and_toxic(&[Keyword::Infect, Keyword::Toxic(2)], 3, 20, 5, 20)]
#[case::infect_and_wither(&[Keyword::Infect, Keyword::Wither], 3, 20, 3, 20)]
#[case::infect_and_lifelink(&[Keyword::Infect, Keyword::Lifelink], 3, 20, 3, 23)]
#[case::toxic_and_lifelink(&[Keyword::Toxic(1), Keyword::Lifelink], 3, 17, 1, 23)]
#[case::infect_and_first_strike(&[Keyword::Infect, Keyword::FirstStrike], 3, 20, 3, 20)]
#[case::infect_and_double_strike(&[Keyword::Infect, Keyword::DoubleStrike], 3, 20, 6, 20)]
#[case::toxic_and_double_strike(&[Keyword::Toxic(1), Keyword::DoubleStrike], 3, 14, 2, 20)]
fn combat_damage_to_a_player(
    #[case] keywords: &[Keyword],
    #[case] power: i32,
    #[case] life_left: i32,
    #[case] poisoned: u32,
    #[case] own_life: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = duel(seats, attacker(power, keywords), vec![])?;
    fight(&mut driver, 1, &[])?;
    assert_eq!(life(&driver, 1), life_left);
    assert_eq!(poison(&driver, 1), poisoned);
    assert_eq!(life(&driver, 0), own_life);
    // Nobody else is touched.
    for seat in 2..seats {
        assert_eq!((life(&driver, seat), poison(&driver, seat)), (20, 0));
    }
    Ok(())
}

#[rstest]
#[case::plain_damage_is_marked(&[], 3, (2, 5), Some((3, 0, (2, 5))))]
#[case::lethal_plain_damage_kills(&[], 5, (2, 5), None)]
#[case::infect_is_counters(&[Keyword::Infect], 3, (2, 5), Some((0, 3, (-1, 2))))]
#[case::wither_is_counters(&[Keyword::Wither], 3, (2, 5), Some((0, 3, (-1, 2))))]
#[case::infect_kills_by_toughness(&[Keyword::Infect], 5, (2, 5), None)]
#[case::wither_kills_by_toughness(&[Keyword::Wither], 5, (2, 5), None)]
#[case::toxic_alone_is_plain_damage(&[Keyword::Toxic(2)], 3, (2, 5), Some((3, 0, (2, 5))))]
#[case::deathtouch_with_wither_kills(&[Keyword::Deathtouch, Keyword::Wither], 1, (2, 5), None)]
#[case::double_strike_with_wither_twice(&[Keyword::DoubleStrike, Keyword::Wither], 2, (2, 5), Some((0, 4, (-2, 1))))]
fn combat_damage_to_a_blocker(
    #[case] keywords: &[Keyword],
    #[case] power: i32,
    #[case] blocker: (i32, i32),
    #[case] expected: Option<(u32, u32, (i32, i32))>,
) -> Result<(), ScriptError> {
    let mut driver = duel(
        2,
        attacker(power, keywords),
        vec![ready(free_creature("B", blocker.0, blocker.1))],
    )?;
    fight(&mut driver, 1, &[("B", "A")])?;
    assert_eq!(fate(&driver, "B"), expected);
    Ok(())
}

#[rstest]
#[case::plain(&[], Some((3, 0, (2, 6))))]
#[case::infect(&[Keyword::Infect], Some((0, 3, (-1, 3))))]
#[case::wither(&[Keyword::Wither], Some((0, 3, (-1, 3))))]
#[case::toxic_does_not_apply_to_creatures(&[Keyword::Toxic(2)], Some((3, 0, (2, 6))))]
fn a_blocker_with_the_keyword_hurts_the_attacker_the_same_way(
    #[case] keywords: &[Keyword],
    #[case] expected: Option<(u32, u32, (i32, i32))>,
) -> Result<(), ScriptError> {
    let attacker = keyworded("A", 2, 6, &[]);
    let blocker = keyworded("B", 3, 9, keywords);
    let mut driver = duel(2, attacker, vec![ready(blocker)])?;
    fight(&mut driver, 1, &[("B", "A")])?;
    assert_eq!(fate(&driver, "A"), expected);
    // Neither side's player is touched by a creature fight.
    assert_eq!((life(&driver, 1), poison(&driver, 1)), (20, 0));
    Ok(())
}

#[test]
fn an_indestructible_creature_still_dies_when_counters_take_its_toughness()
-> Result<(), ScriptError> {
    let wall = keyworded("B", 0, 2, &[Keyword::Indestructible]);
    let mut driver = duel(2, attacker(2, &[Keyword::Infect]), vec![ready(wall)])?;
    fight(&mut driver, 1, &[("B", "A")])?;
    assert_eq!(fate(&driver, "B"), None);
    Ok(())
}

#[rstest]
#[case::wither_cancels_a_plus_counter(1, 1, 0, 0)]
#[case::wither_cancels_a_pair_of_each_it_can(2, 3, 0, 1)]
#[case::wither_beyond_the_plus_counters(2, 4, 0, 2)]
#[case::plus_counters_to_spare(3, 1, 2, 0)]
fn minus_counters_and_plus_counters_cancel(
    #[case] plus: u32,
    #[case] power: i32,
    #[case] plus_left: u32,
    #[case] minus_left: u32,
) -> Result<(), ScriptError> {
    let wall = with_counters(
        ready(free_creature("B", 0, 9)),
        CounterKind::PlusOnePlusOne,
        plus,
    );
    let mut driver = duel(2, attacker(power, &[Keyword::Wither]), vec![wall])?;
    fight(&mut driver, 1, &[("B", "A")])?;
    let held = |kind| permanent(&driver, "B").map(|p| p.counters_of(&kind));
    assert_eq!(held(CounterKind::PlusOnePlusOne), Some(plus_left));
    assert_eq!(held(CounterKind::MinusOneMinusOne), Some(minus_left));
    Ok(())
}

#[rstest]
#[case::nine_poison_is_alive(8, 1)]
#[case::ten_by_infect(7, 3)]
#[case::far_beyond(8, 6)]
#[case::still_alive(2, 3)]
fn ten_poison_counters_lose_the_game(
    #[case] already: u32,
    #[case] power: i32,
) -> Result<(), ScriptError> {
    // Seat 1 starts with some poison counters; a third seat keeps the game going.
    let scenario = scenario(3, attacker(power, &[Keyword::Infect]), vec![])
        .at(2, 0, Step::DeclareAttackers)
        .counters(1, CounterKind::Poison, already);
    let mut driver = Driver::from_scenario(scenario)?;
    fight(&mut driver, 1, &[])?;
    assert_eq!(poison(&driver, 1), already + u32::try_from(power).unwrap());
    let seat = driver.state().player(common::pid(1)).unwrap();
    assert_eq!(seat.is_in_game(), poison(&driver, 1) < 10);
    assert_eq!(life(&driver, 1), 20);
    Ok(())
}

#[test]
fn poison_goes_to_the_player_attacked_and_to_no_one_else() -> Result<(), ScriptError> {
    let mut driver = duel(
        4,
        attacker(4, &[Keyword::Infect, Keyword::Toxic(1)]),
        vec![],
    )?;
    fight(&mut driver, 2, &[])?;
    assert_eq!(poison(&driver, 2), 5);
    for seat in [0, 1, 3] {
        assert_eq!((life(&driver, seat), poison(&driver, seat)), (20, 0));
    }
    Ok(())
}

/// A card of seat 0 that watches life lost.
fn scorekeeper() -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(
        r#"{"Triggered":{"when":{"LifeLost":{"who":"Opponents"}},"effects":[{"GainLife":{"amount":1}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Scorekeeper")
            .with_type(CardType::Creature)
            .with_power_toughness(0, 1)
            .with_rules(ability),
    )
}

#[rstest]
#[case::plain(&[], 21)]
#[case::wither(&[Keyword::Wither], 21)]
#[case::toxic(&[Keyword::Toxic(1)], 21)]
#[case::infect(&[Keyword::Infect], 20)]
fn infect_damage_is_no_life_lost(
    #[case] keywords: &[Keyword],
    #[case] watcher_life: i32,
) -> Result<(), ScriptError> {
    let scenario = scenario_with(
        2,
        vec![ready(attacker(3, keywords)), ready(scorekeeper())],
        vec![],
    )
    .at(2, 0, Step::DeclareAttackers);
    let mut driver = Driver::from_scenario(scenario)?;
    fight(&mut driver, 1, &[])?;
    assert_eq!(life(&driver, 0), watcher_life);
    Ok(())
}

// Prevention

/// Seat 1 holds `spell` and the mana for it; "A" has been declared as an attacker and seat 1 has priority.
fn defended(keywords: &[Keyword], power: i32, spell: &str) -> Result<Driver, ScriptError> {
    let scenario = scenario(2, attacker(power, keywords), vec![])
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(
            1,
            [
                ready(mtg_testkit::cards::forest()),
                ready(mtg_testkit::cards::plains()),
            ],
        )
        .hand(1, [card(spell).unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("A", 1)])?;
    driver.pass()?;
    Ok(driver)
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
#[case::infect_and_fog(&[Keyword::Infect], 3, "Fog", 20, 0)]
#[case::toxic_and_fog(&[Keyword::Toxic(2)], 3, "Fog", 20, 0)]
#[case::infect_and_a_shield_of_four(&[Keyword::Infect], 6, "Mending Hands", 20, 2)]
#[case::infect_and_a_shield_that_stops_all(&[Keyword::Infect], 3, "Mending Hands", 20, 0)]
#[case::toxic_with_damage_left_after_a_shield(&[Keyword::Toxic(2)], 6, "Mending Hands", 18, 2)]
#[case::toxic_with_all_the_damage_prevented(&[Keyword::Toxic(2)], 3, "Mending Hands", 20, 0)]
fn prevention_acts_before_damage_becomes_counters(
    #[case] keywords: &[Keyword],
    #[case] power: i32,
    #[case] spell: &str,
    #[case] life_left: i32,
    #[case] poisoned: u32,
) -> Result<(), ScriptError> {
    let mut driver = defended(keywords, power, spell)?;
    if spell == "Fog" {
        driver.cast("Fog")?;
    } else {
        let me = driver.target_player(1);
        driver.cast_at("Mending Hands", &[me])?;
    }
    settle(&mut driver)?;
    through_combat(&mut driver, &[])?;
    assert_eq!(life(&driver, 1), life_left);
    assert_eq!(poison(&driver, 1), poisoned);
    Ok(())
}

// Damage that is not combat damage

/// An instant that costs nothing: "Target creature you control deals 2 damage to any target".
fn shove() -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}},{"what":"AnyTarget"}],"effects":[{"DealDamage":{"source":{"Target":0},"to":{"Target":1},"amount":2}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Shove")
            .with_type(CardType::Instant)
            .with_mana_cost(mtg_testkit::cards::cost(0, &[]))
            .with_rules(ability),
    )
}

#[rstest]
#[case::plain_to_a_player(&[], false, 18, 0, None)]
#[case::infect_to_a_player(&[Keyword::Infect], false, 20, 2, None)]
#[case::wither_to_a_player(&[Keyword::Wither], false, 18, 0, None)]
#[case::toxic_is_for_combat_damage_only(&[Keyword::Toxic(2)], false, 18, 0, None)]
#[case::plain_to_a_creature(&[], true, 20, 0, Some((2, 0, (2, 5))))]
#[case::infect_to_a_creature(&[Keyword::Infect], true, 20, 0, Some((0, 2, (0, 3))))]
#[case::wither_to_a_creature(&[Keyword::Wither], true, 20, 0, Some((0, 2, (0, 3))))]
#[case::toxic_to_a_creature(&[Keyword::Toxic(2)], true, 20, 0, Some((2, 0, (2, 5))))]
fn damage_from_a_spell_follows_the_keywords_of_its_source(
    #[case] keywords: &[Keyword],
    #[case] at_the_creature: bool,
    #[case] life_left: i32,
    #[case] poisoned: u32,
    #[case] expected: Option<(u32, u32, (i32, i32))>,
) -> Result<(), ScriptError> {
    let scenario = scenario(
        2,
        attacker(1, keywords),
        vec![ready(free_creature("B", 2, 5))],
    )
    .at(2, 0, Step::Main1)
    .hand(0, [shove()]);
    let mut driver = Driver::from_scenario(scenario)?;
    let source = driver.target_named("A")?;
    let target = if at_the_creature {
        driver.target_named("B")?
    } else {
        driver.target_player(1)
    };
    driver.begin_cast("Shove")?;
    driver.choose_targets(&[source])?;
    driver.choose_targets(&[target])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 1), life_left);
    assert_eq!(poison(&driver, 1), poisoned);
    if at_the_creature {
        assert_eq!(fate(&driver, "B"), expected);
    }
    Ok(())
}
