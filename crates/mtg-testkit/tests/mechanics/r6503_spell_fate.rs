//! Goal: a cast trigger that sends the spell elsewhere "instead of putting it into your graveyard as it resolves" (CR 608.2n, 614.1a; Goliath Daydreamer, River Song's Diary, Lilah, Undefeated Slickshot, Feather, the Redeemed) moves only a resolved spell, never a countered one, and does what the card says with it: exiled with a dream counter and cast free when Goliath attacks, exiled with the Diary and cast at random once four lie there, plotted, or returned to hand at the next end step; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::lands;
use crate::common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::refs::{Ref, Targetable};
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, cost, damage, forest, island, mountain, one_target, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A blue and red instant: 1 damage to any target.
fn zap() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Zap")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Blue, Color::Red]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(Targetable::AnyTarget)],
                effects: vec![damage(Ref::Target(0), 1)],
            }),
    )
}

/// Seat 0 in its first main phase with `permanent`, Mountains, Islands and Plains, and `hand`; every library holds ten
/// Islands.
fn table(seats: usize, permanent: &str, hand: Vec<Arc<CardDefinition>>) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                vec![ready(card(permanent).unwrap())],
                lands(3, mountain),
                lands(2, island),
                lands(2, plains),
                lands(1, forest),
            ]
            .concat(),
        )
        .hand(0, hand);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| island()));
    }
    scenario
}

/// Passes until the stack is empty, answering every other question passively.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The cards in exile called `name`, with their counters of `kind`.
fn exiled(driver: &Driver, name: &str, kind: &CounterKind) -> Vec<u32> {
    let state = driver.state();
    state
        .zone(Zone::Exile)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|card| card.card().name == name)
        .map(|card| card.card_counters_of(kind))
        .collect()
}

fn dream() -> CounterKind {
    CounterKind::Other("dream".to_owned())
}

#[rstest]
fn goliath_exiles_the_resolved_spell_with_a_dream_counter(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(table(seats, "Goliath Daydreamer", vec![burn("Bolt", 3)]))?;
    let before = driver.state().player(pid(seats - 1)).unwrap().life();
    driver.cast_at("Bolt", &[Target::Player(pid(seats - 1))])?;
    settle(&mut driver)?;
    assert_eq!(
        driver.state().player(pid(seats - 1)).unwrap().life(),
        before - 3
    );
    assert_eq!(exiled(&driver, "Bolt", &dream()), [1]);
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))).len(), 0);
    Ok(())
}

#[rstest]
fn a_countered_spell_still_goes_to_the_graveyard(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        "Goliath Daydreamer",
        vec![burn("Bolt", 3), card("Counterspell").unwrap()],
    ))?;
    driver.cast_at("Bolt", &[Target::Player(pid(seats - 1))])?;
    // Goliath's trigger resolves first; then the Bolt is countered.
    for _ in 0..10 {
        if driver.state().stack().len() == 1 {
            break;
        }
        driver.answer_passively()?;
    }
    let bolt = driver
        .state()
        .object(driver.state().stack()[0])
        .unwrap()
        .reference();
    driver.cast_at("Counterspell", &[Target::Object(bolt)])?;
    settle(&mut driver)?;
    let graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    assert!(graveyard.contains(&"Bolt".to_owned()), "{graveyard:?}");
    assert_eq!(exiled(&driver, "Bolt", &dream()).len(), 0);
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn goliath_attacks_and_casts_a_dreamed_card_free(#[case] seats: usize) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(table(seats, "Goliath Daydreamer", vec![burn("Bolt", 3)]))?;
    driver.cast_at("Bolt", &[Target::Player(pid(seats - 1))])?;
    settle(&mut driver)?;
    let before = driver.state().player(pid(1)).unwrap().life();
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Goliath Daydreamer", 1)])?;
    let mut cast = false;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::YesNo(_) if !cast => {
                cast = true;
                driver.answer(mtg_engine::decision::Answer::YesNo(
                    mtg_engine::choose::YesNoChoice(true),
                ))?;
            }
            DecisionKind::ChooseTargets(_) => {
                driver.answer(mtg_engine::decision::Answer::ChooseTargets(
                    mtg_engine::choose::TargetChoice::Targets(vec![Target::Player(pid(1))]),
                ))?;
            }
            _ if driver.state().stack().is_empty()
                && driver.state().turn().step() != Step::DeclareAttackers =>
            {
                break;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert!(cast, "Goliath offers the dreamed Bolt");
    assert!(
        exiled(&driver, "Bolt", &dream()).is_empty(),
        "the Bolt was cast"
    );
    assert!(driver.state().player(pid(1)).unwrap().life() <= before - 3);
    Ok(())
}

/// The last seat is active in its first main phase with `bolts` Bolts and as many Mountains; seat 0 controls River
/// Song's Diary.
fn diary_table(seats: usize, bolts: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, seats - 1, Step::Main1)
        .battlefield(0, [ready(card("River Song's Diary").unwrap())])
        .battlefield(seats - 1, lands(bolts, mountain))
        .hand(seats - 1, (0..bolts).map(|_| burn("Bolt", 1)));
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| island()));
    }
    scenario
}

#[rstest]
#[case::two_seats_four_bolts(2, 4, true)]
#[case::three_seats_three_bolts(3, 3, false)]
#[case::four_seats_four_bolts(4, 4, true)]
fn the_diary_exiles_any_players_spell_and_casts_one_at_four(
    #[case] seats: usize,
    #[case] bolts: usize,
    #[case] triggers: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(diary_table(seats, bolts))?;
    for _ in 0..bolts {
        driver.cast_at("Bolt", &[Target::Player(pid(0))])?;
        settle(&mut driver)?;
    }
    let caster = pid(seats - 1);
    assert_eq!(names(driver.state(), Zone::Graveyard(caster)).len(), 0);
    assert_eq!(names(driver.state(), Zone::Exile).len(), bolts);
    driver.idle_until(2, Step::Upkeep)?;
    let diary = driver.state().stack().len() + driver.state().pending_triggers().len();
    assert_eq!(
        diary > 0,
        triggers,
        "the upkeep trigger needs four cards exiled with it"
    );
    Ok(())
}

#[rstest]
#[case::two_seats_multicolored(2, true)]
#[case::three_seats_monocolored(3, false)]
#[case::four_seats_multicolored(4, true)]
fn lilah_plots_a_multicolored_spell(
    #[case] seats: usize,
    #[case] multicolored: bool,
) -> Result<(), ScriptError> {
    let spell = if multicolored { zap() } else { burn("Zap", 1) };
    let mut driver =
        Driver::from_scenario(table(seats, "Lilah, Undefeated Slickshot", vec![spell]))?;
    driver.cast_at("Zap", &[Target::Player(pid(seats - 1))])?;
    settle(&mut driver)?;
    let state = driver.state();
    let plotted: Vec<_> = state
        .zone(Zone::Exile)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|card| card.card().name == "Zap")
        .map(|card| card.plotted().is_some())
        .collect();
    if multicolored {
        assert_eq!(plotted, [true]);
    } else {
        assert_eq!(plotted.len(), 0);
        assert_eq!(names(state, Zone::Graveyard(pid(0))), ["Zap"]);
    }
    Ok(())
}

#[rstest]
fn feather_returns_the_spell_at_the_next_end_step(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        "Feather, the Redeemed",
        vec![card("Giant Growth").unwrap()],
    ))?;
    let feather = driver.target_named("Feather, the Redeemed")?;
    driver.cast_at("Giant Growth", &[feather])?;
    settle(&mut driver)?;
    assert_eq!(names(driver.state(), Zone::Exile), ["Giant Growth"]);
    driver.idle_until(1, Step::End)?;
    settle(&mut driver)?;
    assert!(names(driver.state(), Zone::Hand(pid(0))).contains(&"Giant Growth".to_owned()));
    assert_eq!(names(driver.state(), Zone::Exile).len(), 0);
    Ok(())
}
