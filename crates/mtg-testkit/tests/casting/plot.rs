//! Goal: plotting is a special action from the hand at sorcery timing for the plot cost; it exiles the card face up without the stack, the free cast opens as a sorcery on a later turn than the plot (a creature, a sorcery-timed instant), only for the owner, and a card that leaves exile is no longer plotted.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CastWay;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::events::ZoneCause;
use mtg_engine::object::PermanentState;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, apply_event};
use mtg_testkit::cards::{cost, forest, keyworded_creature};
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

use crate::common::casting::{lands, resolve};
use crate::common::{name_of, names, pid};

fn plot_cost() -> Keyword {
    Keyword::Plot(Cost::mana("{2}".parse().unwrap()))
}

/// A creature for {4}{G}{G} with plot {2}.
fn creature() -> Arc<CardDefinition> {
    keyworded_creature(
        "Plotter",
        4,
        &[Color::Green, Color::Green],
        3,
        3,
        &[plot_cost()],
    )
}

/// An instant for {4}{G}{G} with plot {2}.
fn instant() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Plotted Flash")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(4, &[Color::Green, Color::Green]))
            .with_rules(Ability::Keyword(plot_cost())),
    )
}

/// A creature for {4}{G}{G} without plot.
fn plain() -> Arc<CardDefinition> {
    keyworded_creature("Plain Bear", 4, &[Color::Green, Color::Green], 3, 3, &[])
}

/// Seat 0 holds `card` and `n` Forests at `turn`'s `step`, seat 1 is the other player.
fn game(
    card: Arc<CardDefinition>,
    forests: usize,
    turn: u32,
    step: Step,
) -> Result<Driver, ScriptError> {
    let active = usize::from(turn.is_multiple_of(2));
    Driver::from_scenario(
        Scenario::new(2)
            .at(turn, active, step)
            .library(0, vec![forest(); 12])
            .library(1, vec![forest(); 12])
            .hand(0, [card])
            .battlefield(0, lands(forests, forest)),
    )
}

fn plottable(driver: &Driver) -> Vec<String> {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        return Vec::new();
    };
    options
        .plottable
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect()
}

fn castable(driver: &Driver) -> Vec<String> {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        return Vec::new();
    };
    options
        .castable
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect()
}

fn find(driver: &Driver, zone: Zone, name: &str) -> ObjectId {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == name)
        .unwrap_or_else(|| panic!("{name} is not in {zone:?}"))
}

fn tapped(driver: &Driver) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|o| o.controller() == pid(0) && o.permanent().is_some_and(PermanentState::tapped))
        .count()
}

fn plot(driver: &mut Driver, name: &str) -> Result<Vec<Event>, ScriptError> {
    let id = find(driver, Zone::Hand(pid(0)), name);
    driver.answer(Answer::Priority(PriorityAction::Plot(id)))?;
    driver.pay_tapping(&["Forest", "Forest"], 0)
}

/// Waits for seat 0's decision.
fn to_owner(driver: &mut Driver) -> Result<(), ScriptError> {
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    Ok(())
}

#[rstest]
#[case::creature(creature())]
#[case::instant(instant())]
fn plotting_exiles_the_card_face_up_for_the_plot_cost_without_the_stack(
    #[case] card: Arc<CardDefinition>,
) -> Result<(), ScriptError> {
    let name = card.name.clone();
    let mut driver = game(card, 3, 1, Step::Main1)?;
    assert_eq!(plottable(&driver), std::slice::from_ref(&name));
    let events = plot(&mut driver, &name)?;
    assert!(events.iter().any(|event| matches!(
        event,
        Event::ZoneChanged {
            from: Zone::Hand(_),
            to: Zone::Exile,
            cause: ZoneCause::Plotted,
            ..
        }
    )));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::SpellCast { .. }))
    );
    let id = find(&driver, Zone::Exile, &name);
    let object = driver.state().object(id).unwrap();
    assert_eq!(object.plotted(), Some(1));
    assert_eq!(object.face_down(), None, "plotted cards lie face up");
    assert_eq!(tapped(&driver), 2, "the plot cost was paid");
    assert_eq!(driver.state().zone(Zone::Stack), []);
    assert_eq!(plottable(&driver), Vec::<String>::new());
    assert_eq!(
        castable(&driver),
        Vec::<String>::new(),
        "not on the turn it was plotted in"
    );
    Ok(())
}

#[rstest]
#[case::main_phase(1, Step::Main1, 3, true)]
#[case::second_main(1, Step::Main2, 3, true)]
#[case::upkeep(1, Step::Upkeep, 3, false)]
#[case::combat(1, Step::BeginCombat, 3, false)]
#[case::end_step(1, Step::End, 3, false)]
#[case::opponents_main(2, Step::Main1, 3, false)]
#[case::one_land_short(1, Step::Main1, 1, false)]
fn a_card_is_plotted_only_as_a_sorcery_with_the_cost_in_hand(
    #[case] turn: u32,
    #[case] step: Step,
    #[case] forests: usize,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let mut driver = game(creature(), forests, turn, step)?;
    to_owner(&mut driver)?;
    let expected: Vec<String> = if offered {
        vec!["Plotter".to_owned()]
    } else {
        Vec::new()
    };
    assert_eq!(plottable(&driver), expected);
    Ok(())
}

#[test]
fn a_card_without_plot_is_not_plotted() -> Result<(), ScriptError> {
    let driver = game(plain(), 3, 1, Step::Main1)?;
    assert_eq!(plottable(&driver), Vec::<String>::new());
    Ok(())
}

#[test]
fn a_plotted_creature_is_cast_free_on_a_later_turn_in_a_main_phase() -> Result<(), ScriptError> {
    let mut driver = game(creature(), 3, 1, Step::Main1)?;
    plot(&mut driver, "Plotter")?;
    driver.idle_until(3, Step::Main1)?;
    assert_eq!(castable(&driver), ["Plotter"]);
    let id = find(&driver, Zone::Exile, "Plotter");
    let before = tapped(&driver);
    driver.answer(Answer::Priority(PriorityAction::CastSpell(id)))?;
    driver.pay_tapping(&[], 0)?;
    let cast = driver
        .engine()
        .events()
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::SpellCast { choices, .. } => Some(choices.way),
            _ => None,
        })
        .expect("the spell was cast");
    assert_eq!(cast, Some(CastWay::Plotted));
    assert_eq!(tapped(&driver), before, "no mana was paid");
    resolve(&mut driver)?;
    assert_eq!(
        names(driver.state(), Zone::Battlefield)
            .iter()
            .filter(|n| *n == "Plotter")
            .count(),
        1
    );
    Ok(())
}

#[test]
fn a_plotted_instant_is_cast_only_at_sorcery_timing() -> Result<(), ScriptError> {
    let mut driver = game(instant(), 3, 1, Step::Main1)?;
    plot(&mut driver, "Plotted Flash")?;
    driver.idle_until(3, Step::Upkeep)?;
    to_owner(&mut driver)?;
    assert_eq!(
        castable(&driver),
        Vec::<String>::new(),
        "an instant is cast as a sorcery"
    );
    driver.idle_until(3, Step::Main1)?;
    assert_eq!(castable(&driver), ["Plotted Flash"]);
    Ok(())
}

#[test]
fn a_plotted_card_is_not_cast_in_the_opponents_turn_or_by_the_opponent() -> Result<(), ScriptError>
{
    let mut driver = game(creature(), 3, 1, Step::Main1)?;
    plot(&mut driver, "Plotter")?;
    driver.idle_until(2, Step::Main1)?;
    to_owner(&mut driver)?;
    assert_eq!(castable(&driver), Vec::<String>::new());
    Ok(())
}

#[rstest]
#[case::to_the_graveyard(Zone::Graveyard(pid(0)))]
#[case::to_the_hand(Zone::Hand(pid(0)))]
fn a_plotted_card_that_leaves_exile_is_a_new_object_that_is_not_plotted(
    #[case] to: Zone,
) -> Result<(), ScriptError> {
    let mut driver = game(creature(), 3, 1, Step::Main1)?;
    plot(&mut driver, "Plotter")?;
    let id = find(&driver, Zone::Exile, "Plotter");
    let mut state = driver.state().clone();
    apply_event(
        &mut state,
        &Event::ZoneChanged {
            object: id,
            from: Zone::Exile,
            to,
            cause: ZoneCause::Moved,
        },
    )
    .unwrap();
    apply_event(
        &mut state,
        &Event::ZoneChanged {
            object: id,
            from: to,
            to: Zone::Exile,
            cause: ZoneCause::Exiled,
        },
    )
    .unwrap();
    assert_eq!(state.object(id).unwrap().plotted(), None);
    Ok(())
}

#[test]
fn a_replay_of_the_log_rebuilds_a_game_with_a_plotted_card() -> Result<(), ScriptError> {
    let mut driver = game(creature(), 3, 1, Step::Main1)?;
    plot(&mut driver, "Plotter")?;
    let id = find(&driver, Zone::Exile, "Plotter");
    driver.idle_until(3, Step::Main1)?;
    let rebuilt = mtg_engine::Engine::resume(
        driver.engine().setup().clone(),
        driver.engine().events().to_vec(),
    )
    .unwrap();
    assert_eq!(rebuilt.state_hash(), driver.engine().state_hash());
    assert_eq!(rebuilt.state().object(id).unwrap().plotted(), Some(1));
    Ok(())
}
