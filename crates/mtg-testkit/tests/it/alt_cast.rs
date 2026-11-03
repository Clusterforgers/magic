//! Goal: the one `AltCast` node carries each alternative cost of its table (surge, web-slinging, warp, impending) and the zone, free and flash options; the castable list and the real cast agree (a card is offered exactly when its way is open), the spell remembers its way, and what the way does to the permanent is read from that memory; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{choose_way, lands, pay, ways};
use crate::common::{names, on_battlefield, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::refs::ZoneKind;
use mtg_cards::value::{CastWay, CounterKind};
use mtg_core::card::CardType;
use mtg_engine::PresetPermanent;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::alt_cast_cards;
use mtg_testkit::cards::{forest, free_creature, mountain, plains};
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn bears() -> Arc<CardDefinition> {
    corpus::card("Grizzly Bears").unwrap()
}

/// The table card called `name`.
fn row_card(name: &str) -> Arc<CardDefinition> {
    alt_cast_cards::card(alt_cast_cards::row(name).unwrap())
}

/// Seat 0 holds `hand` and `permanents` at `step` of turn 1; every other seat holds a Forest.
fn table(
    seats: usize,
    step: Step,
    hand: Vec<Arc<CardDefinition>>,
    permanents: Vec<PresetPermanent>,
) -> Scenario {
    (1..seats).fold(
        Scenario::new(seats)
            .at(1, 0, step)
            .hand(0, hand)
            .battlefield(0, permanents),
        |scenario, seat| scenario.hand(seat, [forest()]),
    )
}

/// Answers whatever is asked passively until the stack is empty and nothing waits.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..80 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Whether the card is in the list of castable cards, without casting it.
fn offered(driver: &Driver, name: &str) -> Result<bool, ScriptError> {
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        return Ok(false);
    };
    Ok(options.castable.iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|object| object.card().name == name)
    }))
}

/// Casts `name` for the way whose label starts with `way`, answering what is asked before the payment passively.
fn cast_for(driver: &mut Driver, name: &str, way: &str) -> Result<(), ScriptError> {
    driver.begin_cast(name)?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        choose_way(driver, way)?;
    }
    for _ in 0..10 {
        if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
            break;
        }
        driver.answer_passively()?;
    }
    pay(driver)?;
    settle(driver)
}

fn remembered(driver: &Driver, name: &str) -> Option<CastWay> {
    on_battlefield(driver.state(), name)
        .and_then(|id| driver.state().object(id))
        .and_then(mtg_engine::object::GameObject::cast_way)
}

fn red(count: usize) -> Vec<PresetPermanent> {
    lands(count, mountain)
}

#[rstest]
fn surge_is_offered_only_after_another_spell_was_cast(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let hand = vec![row_card("Surge Beast"), free_creature("Free Bear", 1, 1)];
    let mut driver = Driver::from_scenario(table(seats, Step::Main1, hand, red(1)))?;
    assert!(!offered(&driver, "Surge Beast")?);
    driver.cast("Free Bear")?;
    settle(&mut driver)?;
    assert!(offered(&driver, "Surge Beast")?);
    cast_for(&mut driver, "Surge Beast", "Surge")?;
    assert_eq!(remembered(&driver, "Surge Beast"), Some(CastWay::Surge));
    Ok(())
}

#[rstest]
#[case::a_tapped_creature(true, true)]
#[case::only_untapped_creatures(false, false)]
fn web_slinging_returns_a_tapped_creature_as_its_cost(
    #[case] tapped: bool,
    #[case] open: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut permanents = lands(1, plains);
    permanents.push(if tapped {
        mtg_testkit::tapped(bears())
    } else {
        ready(bears())
    });
    let hand = vec![row_card("Web Beast")];
    let mut driver = Driver::from_scenario(table(seats, Step::Main1, hand, permanents))?;
    assert_eq!(offered(&driver, "Web Beast")?, open);
    if open {
        cast_for(&mut driver, "Web Beast", "Web-slinging")?;
        assert_eq!(remembered(&driver, "Web Beast"), Some(CastWay::WebSlinging));
        assert!(on_battlefield(driver.state(), "Grizzly Bears").is_none());
        assert_eq!(names(driver.state(), Zone::Hand(pid(0))), ["Grizzly Bears"]);
    }
    Ok(())
}

#[rstest]
fn impending_enters_with_time_counters_and_is_no_creature_until_they_are_gone(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] impending: bool,
) -> Result<(), ScriptError> {
    let hand = vec![row_card("Impending Beast")];
    let lands = if impending {
        lands(2, forest)
    } else {
        lands(5, forest)
    };
    let mut driver = Driver::from_scenario(table(seats, Step::Main1, hand, lands))?;
    cast_for(
        &mut driver,
        "Impending Beast",
        if impending { "Impending" } else { "Cast for" },
    )?;
    let id = on_battlefield(driver.state(), "Impending Beast").unwrap();
    let object = driver.state().object(id).unwrap();
    let time = object
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::Other("time".to_owned()));
    assert_eq!(time, if impending { 4 } else { 0 });
    assert_eq!(
        mtg_engine::characteristics::characteristics(driver.state(), id)
            .unwrap()
            .is_creature(),
        !impending
    );
    Ok(())
}

#[rstest]
fn warp_is_exiled_at_the_end_step_and_cast_later_for_its_mana_cost(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let hand = vec![row_card("Warp Beast")];
    let permanents = red(4);
    let mut driver = Driver::from_scenario(table(seats, Step::Main1, hand, permanents))?;
    cast_for(&mut driver, "Warp Beast", "Warp")?;
    assert_eq!(remembered(&driver, "Warp Beast"), Some(CastWay::Warp));
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(names(driver.state(), Zone::Exile), ["Warp Beast"]);
    driver.idle_until(1 + u32::try_from(seats).unwrap(), Step::Main1)?;
    assert!(offered(&driver, "Warp Beast")?);
    driver.begin_cast("Warp Beast")?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        assert!(!ways(&driver)?.iter().any(|way| way.starts_with("Warp")));
    }
    pay(&mut driver)?;
    settle(&mut driver)?;
    assert_eq!(remembered(&driver, "Warp Beast"), None);
    assert!(on_battlefield(driver.state(), "Warp Beast").is_some());
    Ok(())
}

/// A card with an alternative cost from the graveyard.
fn from_graveyard(flash: bool) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Grave Dancer")
            .with_type(CardType::Creature)
            .with_mana_cost(mtg_core::mana::ManaCost::parse("{5}{B}").unwrap())
            .with_power_toughness(2, 2)
            .with_rules(Ability::AltCast {
                way: CastWay::Warp,
                cost: None,
                from: ZoneKind::Graveyard,
                condition: None,
                flash,
            }),
    )
}

#[rstest]
#[case::at_sorcery_timing(Step::Main1, false, true)]
#[case::in_combat_without_flash(Step::DeclareAttackers, false, false)]
#[case::in_combat_with_flash(Step::DeclareAttackers, true, true)]
fn a_free_cast_from_the_graveyard_follows_its_timing(
    #[case] step: Step,
    #[case] flash: bool,
    #[case] open: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, step, vec![], vec![]).graveyard(0, [from_graveyard(flash)]);
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(offered(&driver, "Grave Dancer")?, open);
    if open {
        cast_for(&mut driver, "Grave Dancer", "Warp")?;
        assert_eq!(remembered(&driver, "Grave Dancer"), Some(CastWay::Warp));
    }
    Ok(())
}

#[test]
fn bot_games_with_the_table_cards_keep_every_invariant_and_use_each_way() {
    use mtg_engine::Event;
    use mtg_testkit::{PlayOptions, alt_cast_fuzz_setup, play_game_with};

    use crate::common::seeds::{for_each_seed, fuzz_games};

    let seen = std::sync::Mutex::new(std::collections::BTreeSet::<CastWay>::new());
    for_each_seed(0..fuzz_games(16), |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                alt_cast_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            for event in &outcome.events {
                if let Event::SpellCast { choices, .. } = event
                    && let Some(way) = choices.way
                {
                    seen.insert(way);
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for row in &alt_cast_cards::ROWS {
        assert!(
            seen.contains(&row.way),
            "no game cast {}: {seen:?}",
            row.name
        );
    }
}
