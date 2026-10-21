//! Goal: spells that change a creature for a while play whole, as their cards say, at tables of two, three and four seats: Giant Growth gives +3/+3 to the creature it targeted until the end of the turn and no longer, in response to burn and in combat on a blocker and an attacker; a second pump has a later timestamp; a keyword granted in the middle of a turn (haste, flying) works at once and ends with the turn; a creature that leaves and comes back is a new object that keeps nothing; the pump and the damage it outlasted end together at cleanup; a negative change that takes toughness to 0 kills even an indestructible creature; a spell whose target is gone does nothing; and the log of such games replays to the live state and is rebuilt at every decision.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::removal::{
    Aim, Step as Cast, check_rebuilds, corpus, pass_to, play_script, resolve_the_stack, titan,
};
use common::{name_of, names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::effects::{Layer, Modification, Selector};
use mtg_engine::events::{DeathCause, ResolutionOutcome};
use mtg_engine::keywords::{has_keyword, keywords_of, summoning_sick};
use mtg_engine::object::PermanentState;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, PresetPermanent, characteristics};
use mtg_testkit::cards::{creature, forest, free_creature, island, mountain};
use mtg_testkit::pump_cards::{gust, rush, shrink, surge, unsummon};
use mtg_testkit::{Driver, Scenario, ScriptError, damaged, ready, sick};
use rstest::rstest;

/// A table of `seats`: seat 0 and seat 1 hold `hands` and control `boards`, and every other seat controls a 1/1.
fn table(
    seats: usize,
    hands: [Vec<Arc<CardDefinition>>; 2],
    boards: [Vec<PresetPermanent>; 2],
) -> Scenario {
    let [mine, theirs] = hands;
    let [my_board, their_board] = boards;
    let mut scenario = Scenario::new(seats)
        .hand(0, mine)
        .hand(1, theirs)
        .battlefield(0, my_board)
        .battlefield(1, their_board);
    for seat in 2..seats {
        scenario = scenario.battlefield(
            seat,
            [ready(free_creature(&format!("Bystander {seat}"), 1, 1))],
        );
    }
    scenario
}

fn wolf() -> Arc<CardDefinition> {
    creature("Wolf", 0, &[Color::Green], 3, 3)
}

fn bears() -> Arc<CardDefinition> {
    corpus("Grizzly Bears")
}

/// The power and toughness the rules give the creature called `name`.
fn stats(driver: &Driver, name: &str) -> (i32, i32) {
    let id = driver.id_of(name).unwrap();
    let chars = characteristics(driver.state(), id).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

fn damage_on(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|o| o.permanent())
        .map_or(0, PermanentState::damage)
}

fn effects_created(events: &[Event]) -> Vec<&mtg_engine::effects::ContinuousEffect> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::EffectCreated { effect } => Some(effect),
            _ => None,
        })
        .collect()
}

fn on_battlefield(driver: &Driver, name: &str) -> bool {
    names(driver.state(), Zone::Battlefield).contains(&name.to_owned())
}

/// Passes priority until `kind` is the pending decision.
fn pass_until_asked(
    driver: &mut Driver,
    asked: impl Fn(&DecisionKind) -> bool,
) -> Result<(), ScriptError> {
    for _ in 0..64 {
        if asked(&driver.decision()?.kind) {
            return Ok(());
        }
        driver.pass()?;
    }
    panic!("the decision never came");
}

// Giant Growth

#[rstest]
#[case::the_casters_own("Wolf", (3, 3), (6, 6))]
#[case::a_creature_of_another_seat("Grizzly Bears", (2, 2), (5, 5))]
fn giant_growth_gives_plus_three_until_the_end_of_the_turn(
    #[values(2, 3, 4)] seats: usize,
    #[case] target: &str,
    #[case] before: (i32, i32),
    #[case] after: (i32, i32),
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        [vec![corpus("Giant Growth")], vec![]],
        [vec![ready(forest()), ready(wolf())], vec![ready(bears())]],
    ))?;
    assert_eq!(stats(&driver, target), before);
    let aimed = driver.target_named(target)?;
    let begun = driver.engine().events().len();
    driver.cast_at("Giant Growth", &[aimed])?;
    let spell = driver.state().stack()[0];
    let spell = driver.state().object(spell).unwrap().reference();
    resolve_the_stack(&mut driver)?;

    assert_eq!(stats(&driver, target), after);
    let created = effects_created(&driver.engine().events()[begun..]);
    assert_eq!(created.len(), 1, "one modification, one effect");
    let effect = created[0];
    assert_eq!(effect.layer, Some(Layer::PtModify));
    assert_eq!(
        effect.modification,
        Modification::ModifyPowerToughness {
            power: 3,
            toughness: 3
        }
    );
    assert_eq!(
        effect.selector,
        Selector::Objects(vec![
            driver
                .state()
                .object(driver.id_of(target)?)
                .unwrap()
                .reference()
        ])
    );
    assert_eq!(effect.source, Some(spell), "the spell that made it");
    assert_eq!(effect.controller, pid(0));
    assert_eq!(effect.turn, 1);
    let card = driver.state().object(driver.id_of(target)?).unwrap().card();
    assert_eq!(
        (card.power, card.toughness),
        (Some(before.0), Some(before.1)),
        "the card is as it was printed"
    );
    assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Giant Growth".to_owned()));
    assert_eq!(driver.state().effects().len(), 1);
    Ok(())
}

#[rstest]
fn the_pump_lasts_through_the_turn_and_ends_at_cleanup(
    #[values(2, 3, 4)] seats: usize,
    #[values(Step::End, Step::Main2)] still_pumped_in: Step,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        [vec![corpus("Giant Growth")], vec![]],
        [vec![ready(forest()), ready(wolf())], vec![ready(bears())]],
    ))?;
    let aimed = driver.target_named("Wolf")?;
    driver.cast_at("Giant Growth", &[aimed])?;
    resolve_the_stack(&mut driver)?;
    driver.idle_until(1, still_pumped_in)?;
    assert_eq!(
        stats(&driver, "Wolf"),
        (6, 6),
        "still pumped in {still_pumped_in:?}"
    );
    let begun = driver.engine().events().len();
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(stats(&driver, "Wolf"), (3, 3));
    assert_eq!(driver.state().effects(), []);
    let expired = driver.engine().events()[begun..]
        .iter()
        .filter(|event| matches!(event, Event::EffectExpired { .. }))
        .count();
    assert_eq!(expired, 1);
    Ok(())
}

#[rstest]
fn a_pump_and_the_damage_it_outlasted_end_together_at_cleanup(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 2)] already: u32,
) -> Result<(), ScriptError> {
    // The Wolf is 3/3, so the 3 damage of the Bolt (5 with what it had) would kill it without the pump; the pump and the
    // damage both end in the same batch at cleanup, and it lives on.
    let mut driver = Driver::from_scenario(table(
        seats,
        [vec![corpus("Giant Growth")], vec![corpus("Lightning Bolt")]],
        [
            vec![ready(forest()), damaged(wolf(), already)],
            vec![ready(mountain()), ready(bears())],
        ],
    ))?;
    let aimed = driver.target_named("Wolf")?;
    driver.cast_at("Giant Growth", &[aimed])?;
    resolve_the_stack(&mut driver)?;
    pass_to(&mut driver, 1)?;
    driver.cast_at("Lightning Bolt", &[aimed])?;
    resolve_the_stack(&mut driver)?;
    assert!(on_battlefield(&driver, "Wolf"));
    assert_eq!(damage_on(&driver, "Wolf"), 3 + already);
    driver.idle_until(1, Step::End)?;
    assert!(
        on_battlefield(&driver, "Wolf"),
        "still pumped at the end step"
    );
    let begun = driver.engine().events().len();
    driver.idle_until(2, Step::Upkeep)?;
    assert!(on_battlefield(&driver, "Wolf"), "it did not die at cleanup");
    assert_eq!(
        (stats(&driver, "Wolf"), damage_on(&driver, "Wolf")),
        ((3, 3), 0)
    );
    let events = &driver.engine().events()[begun..];
    assert!(events.iter().any(|e| matches!(e, Event::DamageCleared)));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::EffectExpired { .. }))
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::CreatureDied { .. }))
    );
    Ok(())
}

// Giant Growth in response to burn

#[rstest]
fn giant_growth_in_response_to_burn_saves_the_creature(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] pumped: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        [vec![corpus("Lightning Bolt")], vec![corpus("Giant Growth")]],
        [
            vec![ready(mountain())],
            vec![ready(forest()), ready(bears())],
        ],
    ))?;
    let aimed = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Lightning Bolt", &[aimed])?;
    pass_to(&mut driver, 1)?;
    if pumped {
        driver.cast_at("Giant Growth", &[aimed])?;
    }
    resolve_the_stack(&mut driver)?;
    assert_eq!(
        on_battlefield(&driver, "Grizzly Bears"),
        pumped,
        "3 damage kills a 2/2 and not a 5/5"
    );
    if pumped {
        assert_eq!(damage_on(&driver, "Grizzly Bears"), 3);
        assert_eq!(stats(&driver, "Grizzly Bears"), (5, 5));
        driver.idle_until(2, Step::Upkeep)?;
        assert!(
            on_battlefield(&driver, "Grizzly Bears"),
            "it lives through cleanup"
        );
        assert_eq!(stats(&driver, "Grizzly Bears"), (2, 2));
    }
    Ok(())
}

#[rstest]
fn two_pumps_cannot_save_a_creature_from_enough_burn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        [
            vec![corpus("Lightning Bolt"), corpus("Lightning Bolt")],
            vec![corpus("Giant Growth")],
        ],
        [
            vec![ready(mountain()), ready(mountain())],
            vec![ready(forest()), ready(bears())],
        ],
    ))?;
    let aimed = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Lightning Bolt", &[aimed])?;
    driver.cast_at("Lightning Bolt", &[aimed])?;
    pass_to(&mut driver, 1)?;
    driver.cast_at("Giant Growth", &[aimed])?;
    resolve_the_stack(&mut driver)?;
    assert!(
        !on_battlefield(&driver, "Grizzly Bears"),
        "6 damage on a 5/5"
    );
    assert!(
        driver.state().effects().is_empty(),
        "nothing is left to change"
    );
    Ok(())
}

// Combat

/// Seat 0 attacks seat 1 with the Wolf, seat 1 blocks with the Bears, and the pending decision is priority in the
/// declare blockers step.
fn blocked(seats: usize, hands: [Vec<Arc<CardDefinition>>; 2]) -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(
        table(
            seats,
            hands,
            [
                vec![ready(forest()), ready(wolf())],
                vec![ready(forest()), ready(bears())],
            ],
        )
        .at(1, 0, Step::DeclareAttackers),
    )?;
    driver.attack(&[("Wolf", 1)])?;
    pass_until_asked(&mut driver, |kind| {
        matches!(kind, DecisionKind::DeclareBlockers(_))
    })?;
    driver.block(&[("Grizzly Bears", "Wolf")])?;
    Ok(driver)
}

#[rstest]
fn a_pumped_blocker_survives_and_kills_the_attacker(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = blocked(seats, [vec![], vec![corpus("Giant Growth")]])?;
    pass_to(&mut driver, 1)?;
    let aimed = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Giant Growth", &[aimed])?;
    resolve_the_stack(&mut driver)?;
    driver.pass_until(Step::EndCombat)?;
    assert!(!on_battlefield(&driver, "Wolf"), "5 damage on a 3/3");
    assert!(on_battlefield(&driver, "Grizzly Bears"));
    assert_eq!(damage_on(&driver, "Grizzly Bears"), 3);
    Ok(())
}

#[rstest]
fn a_pumped_attacker_survives_and_kills_the_blocker(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = blocked(seats, [vec![corpus("Giant Growth")], vec![]])?;
    let aimed = driver.target_named("Wolf")?;
    driver.cast_at("Giant Growth", &[aimed])?;
    resolve_the_stack(&mut driver)?;
    driver.pass_until(Step::EndCombat)?;
    assert!(on_battlefield(&driver, "Wolf"));
    assert!(
        !on_battlefield(&driver, "Grizzly Bears"),
        "6 damage on a 2/2"
    );
    assert_eq!(damage_on(&driver, "Wolf"), 2);
    assert_eq!(stats(&driver, "Wolf"), (6, 6));
    Ok(())
}

#[rstest]
fn an_unblocked_pumped_attacker_deals_the_power_it_has_now(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        table(
            seats,
            [vec![corpus("Giant Growth")], vec![]],
            [vec![ready(forest()), ready(wolf())], vec![]],
        )
        .at(1, 0, Step::DeclareAttackers),
    )?;
    driver.attack(&[("Wolf", 1)])?;
    let aimed = driver.target_named("Wolf")?;
    driver.cast_at("Giant Growth", &[aimed])?;
    resolve_the_stack(&mut driver)?;
    driver.pass_until(Step::EndCombat)?;
    driver.expect_life(1, 14)?;
    Ok(())
}

// Two pumps and the timestamp order

#[rstest]
fn a_second_pump_comes_after_the_first_in_timestamp_order(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        [
            vec![
                corpus("Giant Growth"),
                corpus("Giant Growth"),
                shrink("Shrivel", 2),
            ],
            vec![],
        ],
        [
            vec![
                ready(forest()),
                ready(forest()),
                ready(mountain()),
                ready(wolf()),
            ],
            vec![ready(bears())],
        ],
    ))?;
    let aimed = driver.target_named("Wolf")?;
    for spell in ["Giant Growth", "Shrivel", "Giant Growth"] {
        driver.cast_at(spell, &[aimed])?;
        resolve_the_stack(&mut driver)?;
    }
    assert_eq!(stats(&driver, "Wolf"), (7, 7), "3/3, +3/+3, -2/-2, +3/+3");
    let effects = driver.state().effects();
    assert_eq!(effects.len(), 3);
    assert!(
        effects
            .windows(2)
            .all(|pair| pair[0].timestamp < pair[1].timestamp && pair[0].id < pair[1].id)
    );
    assert!(
        effects
            .iter()
            .all(|effect| effect.layer == Some(Layer::PtModify))
    );
    Ok(())
}

// A keyword granted in the middle of a turn

#[rstest]
fn granted_haste_lets_a_creature_that_just_arrived_attack(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] granted: bool,
) -> Result<(), ScriptError> {
    let hand = if granted { vec![rush()] } else { vec![] };
    let mut driver = Driver::from_scenario(table(
        seats,
        [hand, vec![]],
        [
            vec![ready(mountain()), sick(free_creature("Newbie", 2, 2))],
            vec![ready(bears())],
        ],
    ))?;
    let id = driver.id_of("Newbie")?;
    assert!(summoning_sick(driver.state(), id));
    if granted {
        let aimed = driver.target_named("Newbie")?;
        driver.cast_at("Rush", &[aimed])?;
        resolve_the_stack(&mut driver)?;
        assert!(has_keyword(driver.state(), id, &Keyword::Haste));
        assert!(!summoning_sick(driver.state(), id));
    }
    // Without a creature that may attack the engine does not ask: it goes past the declare attackers step.
    for _ in 0..64 {
        if matches!(driver.decision()?.kind, DecisionKind::DeclareAttackers(_))
            || driver.state().turn().step() > Step::DeclareAttackers
        {
            break;
        }
        driver.pass()?;
    }
    let offered: Vec<ObjectId> = match &driver.decision()?.kind {
        DecisionKind::DeclareAttackers(options) => {
            options.attackers.iter().map(|a| a.object).collect()
        }
        _ => Vec::new(),
    };
    assert_eq!(offered.contains(&id), granted);
    Ok(())
}

#[rstest]
fn granted_flying_cannot_be_blocked_by_a_creature_without_flying_or_reach(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] granted: bool,
) -> Result<(), ScriptError> {
    let hand = if granted { vec![gust()] } else { vec![] };
    let mut driver = Driver::from_scenario(table(
        seats,
        [hand, vec![]],
        [vec![ready(mountain()), ready(wolf())], vec![ready(bears())]],
    ))?;
    if granted {
        let aimed = driver.target_named("Wolf")?;
        driver.cast_at("Gust", &[aimed])?;
        resolve_the_stack(&mut driver)?;
    }
    pass_until_asked(&mut driver, |kind| {
        matches!(kind, DecisionKind::DeclareAttackers(_))
    })?;
    driver.attack(&[("Wolf", 1)])?;
    for _ in 0..64 {
        let state = driver.state();
        if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_))
            || state.turn().step() >= Step::CombatDamage
        {
            break;
        }
        driver.pass()?;
    }
    let blockers_asked = matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_));
    assert_eq!(
        blockers_asked, !granted,
        "the Bears can block a Wolf on the ground only"
    );
    Ok(())
}

#[rstest]
fn a_keyword_is_granted_for_the_turn_only(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        [vec![gust()], vec![]],
        [vec![ready(mountain()), ready(wolf())], vec![ready(bears())]],
    ))?;
    let id = driver.id_of("Wolf")?;
    let aimed = driver.target_named("Wolf")?;
    driver.cast_at("Gust", &[aimed])?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(keywords_of(driver.state(), id), [&Keyword::Flying]);
    driver.idle_until(1, Step::End)?;
    assert!(has_keyword(driver.state(), id, &Keyword::Flying));
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(keywords_of(driver.state(), id).len(), 0);
    Ok(())
}

#[rstest]
fn two_changes_of_one_set_of_creatures_are_fixed_when_the_spell_resolves(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        [
            vec![
                surge("Surge"),
                creature("Latecomer", 0, &[Color::Green], 2, 2),
            ],
            vec![],
        ],
        [
            vec![
                ready(forest()),
                ready(forest()),
                ready(forest()),
                ready(wolf()),
                ready(free_creature("Cub", 1, 1)),
            ],
            vec![ready(bears())],
        ],
    ))?;
    let begun = driver.engine().events().len();
    driver.cast("Surge")?;
    resolve_the_stack(&mut driver)?;
    let created = effects_created(&driver.engine().events()[begun..]);
    assert_eq!(
        created.iter().map(|e| e.layer).collect::<Vec<_>>(),
        [Some(Layer::PtModify), Some(Layer::Ability)],
        "one effect for each change, in the order of the text"
    );
    assert_eq!(created[0].timestamp + 1, created[1].timestamp);
    assert_eq!(created[0].selector, created[1].selector);
    assert_eq!(stats(&driver, "Wolf"), (4, 4));
    assert_eq!(stats(&driver, "Cub"), (2, 2));
    assert_eq!(stats(&driver, "Grizzly Bears"), (2, 2), "not one of theirs");
    let wolf = driver.id_of("Wolf")?;
    assert_eq!(keywords_of(driver.state(), wolf), [&Keyword::Trample]);
    // A creature that arrives afterwards is not among those the spell changed (CR 611.2c).
    driver.cast("Latecomer")?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(stats(&driver, "Latecomer"), (2, 2));
    let latecomer = driver.id_of("Latecomer")?;
    assert_eq!(keywords_of(driver.state(), latecomer).len(), 0);
    Ok(())
}

// A new object keeps nothing

#[rstest]
fn a_pumped_creature_that_leaves_and_returns_is_a_new_object_without_the_pump(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        [vec![corpus("Giant Growth"), unsummon("Unsummon")], vec![]],
        [
            vec![
                ready(forest()),
                ready(forest()),
                ready(island()),
                ready(wolf()),
            ],
            vec![ready(bears())],
        ],
    ))?;
    let aimed = driver.target_named("Wolf")?;
    driver.cast_at("Giant Growth", &[aimed])?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(stats(&driver, "Wolf"), (6, 6));
    assert_eq!(driver.state().effects().len(), 1);

    let begun = driver.engine().events().len();
    driver.cast_at("Unsummon", &[aimed])?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), ["Wolf"]);
    assert!(
        driver.state().effects().is_empty(),
        "an effect that names a creature that is gone has nothing left to do and is removed"
    );
    assert!(
        driver.engine().events()[begun..]
            .iter()
            .any(|event| matches!(event, Event::EffectExpired { .. }))
    );
    driver.cast("Wolf")?;
    resolve_the_stack(&mut driver)?;
    assert!(on_battlefield(&driver, "Wolf"));
    assert_eq!(stats(&driver, "Wolf"), (3, 3), "it is a new object");
    Ok(())
}

#[test]
fn an_effect_that_names_an_object_does_not_reach_the_new_object_even_before_it_is_removed()
-> Result<(), ScriptError> {
    // Housekeeping removes a spent effect between steps, but a creature can leave and return inside one batch of events.
    let mut driver = Driver::from_scenario(table(
        2,
        [vec![corpus("Giant Growth")], vec![]],
        [vec![ready(forest()), ready(wolf())], vec![ready(bears())]],
    ))?;
    let aimed = driver.target_named("Wolf")?;
    driver.cast_at("Giant Growth", &[aimed])?;
    resolve_the_stack(&mut driver)?;
    let effect = driver.state().effects()[0].clone();
    let id = driver.id_of("Wolf")?;
    let mut state = driver.state().clone();
    for (from, to) in [
        (Zone::Battlefield, Zone::Hand(pid(0))),
        (Zone::Hand(pid(0)), Zone::Battlefield),
    ] {
        mtg_engine::apply_event(
            &mut state,
            &Event::ZoneChanged {
                object: id,
                from,
                to,
                cause: mtg_engine::events::ZoneCause::Moved,
            },
        )
        .unwrap();
    }
    assert_eq!(state.object(id).unwrap().zone(), Zone::Battlefield);
    assert!(!effect.is_active(&state));
    let chars = characteristics(&state, id).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(3), Some(3)));
    assert_eq!(state.effects().len(), 1, "still listed until housekeeping");
    Ok(())
}

// Negative changes

#[rstest]
#[case::a_two_two("Grizzly Bears", 2, true)]
#[case::a_three_three("Wolf", 2, false)]
#[case::a_three_three_by_three("Wolf", 3, true)]
#[case::an_indestructible_five_five("Titan", 5, true)]
#[case::an_indestructible_five_five_by_less("Titan", 4, false)]
fn a_change_that_takes_toughness_to_zero_kills_the_creature_even_if_it_is_indestructible(
    #[values(2, 3, 4)] seats: usize,
    #[case] victim: &str,
    #[case] by: i32,
    #[case] dies: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        [vec![shrink("Shrivel", by)], vec![]],
        [
            vec![ready(mountain())],
            vec![ready(bears()), ready(wolf()), ready(titan())],
        ],
    ))?;
    let aimed = driver.target_named(victim)?;
    let begun = driver.engine().events().len();
    driver.cast_at("Shrivel", &[aimed])?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(on_battlefield(&driver, victim), !dies);
    let causes: Vec<DeathCause> = driver.engine().events()[begun..]
        .iter()
        .filter_map(|event| match event {
            Event::CreatureDied { cause, .. } => Some(*cause),
            _ => None,
        })
        .collect();
    assert_eq!(
        causes,
        if dies {
            vec![DeathCause::ZeroToughness]
        } else {
            vec![]
        }
    );
    Ok(())
}

#[rstest]
fn a_pump_and_a_shrink_together_decide_what_the_creature_is(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        [vec![shrink("Shrivel", 4)], vec![corpus("Giant Growth")]],
        [
            vec![ready(mountain())],
            vec![ready(forest()), ready(bears())],
        ],
    ))?;
    let aimed = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Shrivel", &[aimed])?;
    pass_to(&mut driver, 1)?;
    driver.cast_at("Giant Growth", &[aimed])?;
    resolve_the_stack(&mut driver)?;
    assert!(
        on_battlefield(&driver, "Grizzly Bears"),
        "2/2, +3/+3 first, then -4/-4"
    );
    assert_eq!(stats(&driver, "Grizzly Bears"), (1, 1));
    Ok(())
}

// A target that is gone

#[rstest]
fn giant_growth_does_nothing_when_its_target_is_gone(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        [vec![corpus("Giant Growth")], vec![corpus("Lightning Bolt")]],
        [
            vec![ready(forest()), ready(wolf())],
            vec![ready(mountain()), ready(bears())],
        ],
    ))?;
    let aimed = driver.target_named("Wolf")?;
    let begun = driver.engine().events().len();
    driver.cast_at("Giant Growth", &[aimed])?;
    pass_to(&mut driver, 1)?;
    driver.cast_at("Lightning Bolt", &[aimed])?;
    resolve_the_stack(&mut driver)?;
    let events = &driver.engine().events()[begun..];
    assert!(!on_battlefield(&driver, "Wolf"), "3 damage on a 3/3");
    assert_eq!(effects_created(events).len(), 0);
    assert_eq!(driver.state().effects(), []);
    assert!(events.iter().any(|event| matches!(
        event,
        Event::ResolutionEnded {
            outcome: ResolutionOutcome::Fizzled,
            ..
        }
    )));
    assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Giant Growth".to_owned()));
    Ok(())
}

#[rstest]
fn giant_growth_is_not_offered_without_a_creature_to_target(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [corpus("Giant Growth")])
            .battlefield(0, [ready(forest())]),
    )?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority is asked");
    };
    assert_eq!(options.castable.len(), 0);
    assert!(driver.begin_cast("Giant Growth").is_err());
    Ok(())
}

// The log

#[rstest]
fn the_log_replays_to_the_live_state_and_is_rebuilt_at_every_decision(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), mtg_engine::EngineError> {
    let setup = table(
        seats,
        [
            vec![corpus("Lightning Bolt"), corpus("Giant Growth"), gust()],
            vec![corpus("Giant Growth")],
        ],
        [
            vec![
                ready(mountain()),
                ready(mountain()),
                ready(forest()),
                ready(wolf()),
            ],
            vec![ready(forest()), ready(bears())],
        ],
    )
    .setup()
    .unwrap();
    let played = play_script(
        setup,
        vec![
            Cast {
                seat: 0,
                card: "Lightning Bolt",
                aim: Aim::Creature("Grizzly Bears"),
            },
            Cast {
                seat: 1,
                card: "Giant Growth",
                aim: Aim::Creature("Grizzly Bears"),
            },
            Cast {
                seat: 0,
                card: "Giant Growth",
                aim: Aim::Creature("Wolf"),
            },
            Cast {
                seat: 0,
                card: "Gust",
                aim: Aim::Creature("Wolf"),
            },
        ],
    )?;
    let created = played
        .log
        .iter()
        .filter(|event| matches!(event, Event::EffectCreated { .. }))
        .count();
    assert_eq!(created, 3, "two pumps and a keyword");
    check_rebuilds(&played)?;
    let state = mtg_engine::replay(&played.setup, &played.log)?;
    let wolf = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| name_of(&state, *id) == "Wolf")
        .unwrap();
    assert_eq!(keywords_of(&state, wolf), [&Keyword::Flying]);
    Ok(())
}
