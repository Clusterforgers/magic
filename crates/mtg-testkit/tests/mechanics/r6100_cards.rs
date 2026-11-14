//! Goal: the r61-00 facts of the turn play whole at two to four seats: a creature card that left its owner's graveyard
//! (Syrix, Carrier of the Flame), the creatures exiled under the opponents' control (Vren, the Relentless) and a creature
//! that fought (Boxing Ring), each counted for the right player only.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, mountain, plains, swamp};
use mtg_testkit::corpus::card;
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
