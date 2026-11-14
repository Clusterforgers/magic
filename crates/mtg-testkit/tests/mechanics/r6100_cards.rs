//! Goal: the r61-00 facts of the turn play whole at two to four seats: a creature card that left its owner's graveyard
//! (Syrix, Carrier of the Flame), the creatures exiled under the opponents' control (Vren, the Relentless) and a creature
//! that fought (Boxing Ring), each counted for the right player only; the players and planeswalkers a source damaged
//! this game (The Fallen) and a descent that bores and transforms (Brass's Tunnel-Grinder).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, mountain, plains, swamp, walker};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

/// The life of the seat.
fn r6100_life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// The card with this name in the seat's graveyard, as a target.
fn r6100_in_graveyard(driver: &Driver, seat: usize, name: &str) -> Target {
    let state = driver.state();
    let id = state
        .zone(Zone::Graveyard(pid(seat)))
        .iter()
        .copied()
        .find(|id| name_of(state, *id) == name)
        .unwrap();
    Target::Object(state.object(id).unwrap().reference())
}

/// Passes and answers passively until the turn reaches the cleanup step, choosing `targets` for every trigger that asks.
fn r6100_to_cleanup(driver: &mut Driver, targets: &[Target]) -> Result<u32, ScriptError> {
    let mut asked = 0;
    let mut used: Vec<Target> = Vec::new();
    for _ in 0..200 {
        if driver.state().turn().step() == Step::Cleanup || driver.state().turn().number() > 1 {
            return Ok(asked);
        }
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseTargets(options) if !options.can_cancel && !targets.is_empty() => {
                asked += u32::from(used.is_empty());
                let legal: Vec<Target> = targets
                    .iter()
                    .copied()
                    .filter(|t| options.legal.contains(t) && !used.contains(t))
                    .take(1)
                    .collect();
                used.extend(legal.iter().copied());
                driver.choose_targets(&legal)?;
            }
            DecisionKind::ChooseTargets(_) => {
                asked += 1;
                driver.answer_passively()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("end step".into()))
}

/// Answers passively until the stack is empty and the active player has priority again.
fn r6100_resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.pass()?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && decision.player == pid(0)
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("resolving".into()))
}

/// A Phoenix creature card.
fn r6100_phoenix() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Phoenix Chick")
            .with_type(CardType::Creature)
            .with_subtype("Phoenix")
            .with_power_toughness(2, 1),
    )
}

#[rstest]
#[case::a_creature_card_left_your_graveyard(0, "Grizzly Bears", true)]
#[case::a_land_card_is_no_creature_card(0, "Forest", false)]
#[case::an_opponents_graveyard_does_not_count(1, "Grizzly Bears", false)]
fn r6100_syrix_burns_after_a_creature_card_left_your_graveyard(
    #[values(2, 3, 4)] seats: usize,
    #[case] owner: usize,
    #[case] exiled: &str,
    #[case] burns: bool,
) -> Result<(), ScriptError> {
    let gone = if exiled == "Forest" {
        forest()
    } else {
        card("Grizzly Bears").unwrap()
    };
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Syrix, Carrier of the Flame").unwrap()),
                ready(plains()),
            ],
        )
        .hand(0, [card("Funeral Pyre").unwrap()])
        .graveyard(owner, [gone]);
    let mut driver = Driver::from_scenario(scenario)?;
    let target = r6100_in_graveyard(&driver, owner, exiled);
    driver.cast_at("Funeral Pyre", &[target])?;
    driver.pass()?;
    let syrix = driver.target_named("Syrix, Carrier of the Flame")?;
    let victim = driver.target_player(1);
    let before = r6100_life(&driver, 1);
    let asked = r6100_to_cleanup(&mut driver, &[syrix, victim])?;
    assert_eq!(asked, u32::from(burns));
    let expected = if burns { before - 3 } else { before };
    assert_eq!(r6100_life(&driver, 1), expected);
    Ok(())
}

#[rstest]
fn r6100_syrix_may_be_cast_from_the_graveyard_when_another_phoenix_dies(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] cast: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(r6100_phoenix()),
                ready(mountain()),
                ready(mountain()),
                ready(swamp()),
                ready(swamp()),
                ready(mountain()),
                ready(mountain()),
            ],
        )
        .hand(0, [card("Lightning Bolt").unwrap()])
        .graveyard(0, [card("Syrix, Carrier of the Flame").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    let chick = driver.target_named("Phoenix Chick")?;
    driver.cast_at("Lightning Bolt", &[chick])?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::YesNo(_) => {
                driver.answer(mtg_engine::decision::Answer::YesNo(
                    mtg_engine::choose::YesNoChoice(cast),
                ))?;
            }
            DecisionKind::PayMana(_) if cast => {
                driver.answer(mtg_engine::decision::Answer::PayMana(
                    mtg_engine::decision::PayChoice::Auto,
                ))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let on_battlefield = driver.id_of("Syrix, Carrier of the Flame").is_ok();
    assert_eq!(on_battlefield, cast);
    Ok(())
}

#[rstest]
#[case::an_opponents_creature_exiled(&[(1, true)], 1)]
#[case::an_opponents_creature_that_would_die(&[(1, false)], 1)]
#[case::your_own_creature_does_not_count(&[(0, true)], 0)]
#[case::two_opponents_creatures(&[(1, true), (1, false)], 2)]
fn r6100_vren_makes_a_rat_for_each_creature_exiled_under_an_opponent(
    #[values(2, 3, 4)] seats: usize,
    #[case] removals: &[(usize, bool)],
    #[case] rats: usize,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(card("Vren, the Relentless").unwrap())];
    mine.extend((0..4).map(|_| ready(plains())));
    mine.extend((0..4).map(|_| ready(mountain())));
    let mut theirs = Vec::new();
    let mut hand = Vec::new();
    for (at, (seat, swords)) in removals.iter().enumerate() {
        let victim = ready(free_creature(&format!("Victim {at}"), 2, 2));
        if *seat == 0 {
            mine.push(victim);
        } else {
            theirs.push(victim);
        }
        hand.push(if *swords {
            card("Swords to Plowshares").unwrap()
        } else {
            card("Lightning Bolt").unwrap()
        });
    }
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine)
        .battlefield(1, theirs);
    scenario = scenario.hand(0, hand);
    let mut driver = Driver::from_scenario(scenario)?;
    for (at, (_, swords)) in removals.iter().enumerate() {
        let victim = driver.target_named(&format!("Victim {at}"))?;
        let spell = if *swords {
            "Swords to Plowshares"
        } else {
            "Lightning Bolt"
        };
        driver.cast_at(spell, &[victim])?;
        r6100_resolve(&mut driver)?;
    }
    for at in 0..removals.len() {
        assert!(driver.id_of(&format!("Victim {at}")).is_err());
    }
    r6100_to_cleanup(&mut driver, &[])?;
    let state = driver.state();
    let made: Vec<_> = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| name_of(state, *id) == "Rat")
        .collect();
    assert_eq!(made.len(), rats);
    for rat in made {
        let chars = mtg_engine::characteristics::characteristics(state, rat).unwrap();
        // One for each other Rat token, and Vren, a Rat Rogue.
        let size = 1 + i32::try_from(rats).unwrap();
        assert_eq!(chars.power, Some(size));
        assert_eq!(chars.toughness, Some(size));
        assert_eq!(state.object(rat).unwrap().controller(), pid(0));
    }
    Ok(())
}

#[rstest]
#[case::the_same_mana_value(2, true)]
#[case::another_mana_value(3, false)]
fn r6100_boxing_ring_fights_a_creature_of_the_same_mana_value_and_pays_a_treasure(
    #[values(2, 3, 4)] seats: usize,
    #[case] their_value: u32,
    #[case] fights: bool,
) -> Result<(), ScriptError> {
    let theirs = mtg_testkit::cards::creature("Rival", their_value, &[], 1, 1);
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Boxing Ring").unwrap()),
                ready(forest()),
                ready(forest()),
            ],
        )
        .battlefield(1, [ready(theirs)])
        .hand(0, [card("Grizzly Bears").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    assert!(driver.offered_abilities("Boxing Ring")?.is_empty());
    driver.cast("Grizzly Bears")?;
    driver.pass()?;
    let rival = driver.target_named("Rival")?;
    for _ in 0..20 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseTargets(options) => {
                assert_eq!(options.legal.contains(&rival), fights);
                let chosen: Vec<Target> = options.legal.iter().copied().take(1).collect();
                driver.choose_targets(&chosen)?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(driver.id_of("Rival").is_err(), fights);
    assert_eq!(!driver.offered_abilities("Boxing Ring")?.is_empty(), fights);
    Ok(())
}

/// The counters of the kind on the named permanent.
fn r6100_counters(driver: &Driver, name: &str, kind: &CounterKind) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |held| held.counters_of(kind))
}

#[rstest]
#[case::the_player_it_hit(false)]
#[case::the_planeswalker_it_hit(true)]
fn r6100_the_fallen_hurts_each_opponent_and_planeswalker_it_has_dealt_damage_to(
    #[values(2, 3, 4)] seats: usize,
    #[case] at_walker: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card("The Fallen").unwrap())])
        .battlefield(seats - 1, [ready(walker("Untouched Walker", 3))]);
    // The walker it attacks belongs to seat 1, beside the untouched one at two seats.
    let mut first = vec![ready(walker("Target Walker", 3))];
    if seats == 2 {
        first.push(ready(walker("Untouched Walker", 3)));
    }
    scenario = scenario.battlefield(1, first);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 10]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    if at_walker {
        driver.attack_planeswalkers(&[("The Fallen", "Target Walker")])?;
    } else {
        driver.attack(&[("The Fallen", 1)])?;
    }
    driver.idle_until(1, Step::Main2)?;
    let walker_loyalty = r6100_counters(&driver, "Target Walker", &CounterKind::Loyalty);
    assert_eq!(walker_loyalty, if at_walker { 1 } else { 3 });
    let lives: Vec<i32> = (0..seats).map(|seat| r6100_life(&driver, seat)).collect();
    let next = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(next, Step::Draw)?;
    for seat in 1..seats {
        let hurt = seat == 1 && !at_walker;
        assert_eq!(
            r6100_life(&driver, seat),
            lives[seat] - i32::from(hurt),
            "seat {seat}"
        );
    }
    assert_eq!(r6100_life(&driver, 0), lives[0]);
    // The planeswalker it hit lost its last loyalty; one it never hit is untouched.
    assert_eq!(driver.id_of("Target Walker").is_err(), at_walker);
    assert_eq!(
        r6100_counters(&driver, "Untouched Walker", &CounterKind::Loyalty),
        3
    );
    Ok(())
}

#[rstest]
#[case::the_third_bore_counter_transforms(2, true, true)]
#[case::the_second_does_not(1, true, false)]
#[case::no_descent_no_counter(2, false, false)]
fn r6100_brass_bores_after_a_descent_and_transforms_at_three(
    #[values(2, 3, 4)] seats: usize,
    #[case] bore: u32,
    #[case] descend: bool,
    #[case] transforms: bool,
) -> Result<(), ScriptError> {
    const BRASS: &str = "Brass's Tunnel-Grinder // Tecutlan, the Searing Rift";
    let bore_kind = CounterKind::Other("bore".to_owned());
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                with_counters(ready(card(BRASS).unwrap()), bore_kind.clone(), bore),
                ready(card("Grizzly Bears").unwrap()),
                ready(mountain()),
            ],
        )
        .hand(0, [card("Lightning Bolt").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    let target = if descend {
        driver.target_named("Grizzly Bears")?
    } else {
        driver.target_player(1)
    };
    driver.cast_at("Lightning Bolt", &[target])?;
    r6100_resolve(&mut driver)?;
    r6100_to_cleanup(&mut driver, &[])?;
    let id = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            name_of(driver.state(), *id).starts_with("Brass")
                || name_of(driver.state(), *id).starts_with("Tecutlan")
        })
        .unwrap();
    let object = driver.state().object(id).unwrap();
    let held = object.permanent().unwrap().counters_of(&bore_kind);
    if transforms {
        assert_eq!(held, 0);
        let chars = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
        assert!(chars.types.contains(CardType::Land));
    } else {
        assert_eq!(held, bore + u32::from(descend));
        let chars = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
        assert!(!chars.types.contains(CardType::Land));
    }
    Ok(())
}

const OJER: &str = "Ojer Axonil, Deepest Might // Temple of Power";

/// An instant for {0} with no color that deals `amount` damage to any target.
fn r6100_colorless_burn(amount: i32) -> Arc<CardDefinition> {
    let shaped = mtg_testkit::cards::burn("Gray Shot", amount);
    Arc::new(
        CardDefinition::new("Gray Shot")
            .with_type(CardType::Instant)
            .with_mana_cost(mtg_testkit::cards::cost(0, &[]))
            .with_abilities_of([shaped.as_ref()]),
    )
}

#[rstest]
#[case::red_noncombat_damage_to_an_opponent_rises(true, true, 4)]
#[case::to_a_creature_it_stays(true, false, 3)]
#[case::a_colorless_source_stays(false, true, 3)]
fn r6100_ojer_axonil_raises_small_red_noncombat_damage_to_an_opponent(
    #[values(2, 3, 4)] seats: usize,
    #[case] red: bool,
    #[case] at_player: bool,
    #[case] dealt: i32,
) -> Result<(), ScriptError> {
    let spell = if red {
        mtg_testkit::cards::burn("Red Shot", 3)
    } else {
        r6100_colorless_burn(3)
    };
    let name = spell.name.clone();
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card(OJER).unwrap()), ready(mountain())])
        .battlefield(1, [ready(free_creature("Big Rival", 5, 5))])
        .hand(0, [spell]);
    let mut driver = Driver::from_scenario(scenario)?;
    let target = if at_player {
        driver.target_player(1)
    } else {
        driver.target_named("Big Rival")?
    };
    let before = r6100_life(&driver, 1);
    driver.cast_at(&name, &[target])?;
    r6100_resolve(&mut driver)?;
    if at_player {
        assert_eq!(r6100_life(&driver, 1), before - dealt);
    } else {
        let rival = driver.id_of("Big Rival")?;
        let marked = driver
            .state()
            .object(rival)
            .unwrap()
            .permanent()
            .unwrap()
            .damage();
        assert_eq!(i32::try_from(marked).unwrap(), dealt);
        assert_eq!(r6100_life(&driver, 1), before);
    }
    Ok(())
}

#[rstest]
fn r6100_ojer_axonil_returns_tapped_and_transformed(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card(OJER).unwrap())])
        .hand(0, [r6100_colorless_burn(4)]);
    let mut driver = Driver::from_scenario(scenario)?;
    let ojer = driver.target_named("Ojer Axonil, Deepest Might")?;
    driver.cast_at("Gray Shot", &[ojer])?;
    r6100_resolve(&mut driver)?;
    let id = driver.id_of("Temple of Power")?;
    let object = driver.state().object(id).unwrap();
    assert!(object.permanent().unwrap().tapped());
    assert_eq!(object.owner(), pid(0));
    let chars = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    assert!(chars.types.contains(CardType::Land));
    assert!(!chars.types.contains(CardType::Creature));
    Ok(())
}

#[rstest]
#[case::six_noncombat_damage(2, true)]
#[case::three_is_too_little(1, false)]
fn r6100_temple_of_power_transforms_after_four_red_noncombat_damage(
    #[values(2, 3, 4)] seats: usize,
    #[case] shots: usize,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let temple = Arc::new(card(OJER).unwrap().faces[1].clone());
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(temple),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
            ],
        )
        .hand(
            0,
            (0..shots).map(|_| mtg_testkit::cards::burn("Red Shot", 3)),
        );
    let mut driver = Driver::from_scenario(scenario)?;
    assert!(driver.offered_abilities("Temple of Power")?.is_empty());
    for _ in 0..shots {
        let target = driver.target_player(1);
        driver.begin_cast("Red Shot")?;
        driver.choose_targets(&[target])?;
        driver.pay_tapping(&["Mountain"], 0)?;
        r6100_resolve(&mut driver)?;
    }
    assert_eq!(
        !driver.offered_abilities("Temple of Power")?.is_empty(),
        offered
    );
    Ok(())
}
