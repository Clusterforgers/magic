//! Goal: a merged permanent (a creature a mutating creature spell merged with) is one object to every seat: the same id with the characteristics of the top card, the abilities of both and the cards it is made of from the top down, the same for each viewer, while the spell is gone from the stack and the cards still in the hands stay hidden from the seats that may not see them; a commander card its owner puts into the command zone apart from the other cards reaches every seat as a public event.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_core::ids::PlayerId;
use mtg_engine::Event;
use mtg_engine::choose::{ChoiceOption, OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::format::Commander;
use mtg_engine::zones::Zone;
use mtg_protocol::{EventView, ObjectView, StateView, project_events};
use mtg_testkit::cards::{bear, forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const GNARR: &str = "Trumpeting Gnarr";

/// Seat 0 holds the Gnarr beside a Bear and five lands; every other seat has a hand of its own.
fn table(seats: usize) -> Scenario {
    let lands = (0..5).map(|i| ready(if i % 2 == 0 { forest() } else { island() }));
    let mut scenario = Scenario::new(seats)
        .library(0, vec![forest(); 8])
        .hand_named(0, [GNARR])
        .battlefield(0, lands.chain([ready(bear())]).collect::<Vec<_>>());
    let others = [
        ["Doom Blade", "Fireball"],
        ["Giant Growth", "Opt"],
        ["Brainstorm", "Lightning Bolt"],
    ];
    for other in 1..seats {
        scenario = scenario
            .library(other, vec![forest(); 8])
            .hand_named(other, others[other - 1]);
    }
    scenario
}

/// Seat 0 casts the Gnarr for its mutate cost at the Bear and everyone lets it resolve, answering `over`.
fn merge(driver: &mut Driver, over: bool) -> Result<(), ScriptError> {
    let target = driver.target_named("Grizzly Bears")?;
    driver.begin_cast(GNARR)?;
    let DecisionKind::ChooseOption(options) = &driver.decision()?.kind else {
        panic!("the way is asked");
    };
    let at = options
        .options
        .iter()
        .position(
            |option| matches!(option, ChoiceOption::Label(label) if label.starts_with("Mutate")),
        )
        .unwrap();
    driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
    driver.choose_targets(&[target])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    for _ in 0..300 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => driver.answer(Answer::YesNo(YesNoChoice(over)))?,
            _ => driver.answer_passively()?,
        };
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
fn every_seat_sees_one_object_made_of_the_cards_from_the_top_down(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] over: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    let bear_id = driver.id_of("Grizzly Bears")?;
    merge(&mut driver, over)?;
    let state = driver.state();
    let top = if over { GNARR } else { "Grizzly Bears" };
    let under = if over { "Grizzly Bears" } else { GNARR };
    let mut seen = Vec::new();
    for seat in 0..seats {
        let viewer = PlayerId::from_index(seat);
        let view = ObjectView::of(state, viewer, bear_id).unwrap();
        assert_eq!(view.name, top);
        let cards: Vec<_> = view.merged.iter().map(|card| card.name.as_str()).collect();
        assert_eq!(cards, [top, under]);
        // The ids are those of the cards as objects of their own later: the permanent's own and the spell's.
        assert!(view.merged.iter().any(|card| card.id == bear_id));
        seen.push(serde_json::to_value(&view).unwrap());
    }
    assert!(
        seen.windows(2).all(|pair| pair[0] == pair[1]),
        "every seat sees the same"
    );
    // The spell's card is no object on the stack or elsewhere, and the hands of the others stay hidden.
    let table = StateView::for_player(state, PlayerId::from_index(0));
    assert_eq!(table.table.stack.count, 0);
    let text =
        serde_json::to_string(&StateView::for_player(state, PlayerId::from_index(0))).unwrap();
    for hidden in [
        "Doom Blade",
        "Fireball",
        "Giant Growth",
        "Opt",
        "Brainstorm",
        "Lightning Bolt",
    ] {
        assert!(!text.contains(hidden), "{hidden}");
    }
    assert!(state.zone(Zone::Battlefield).contains(&bear_id));
    Ok(())
}

#[rstest]
fn a_commander_card_parted_from_a_merged_permanent_reaches_every_seat(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] over: bool,
) -> Result<(), ScriptError> {
    let lands = (0..8).map(|i| ready(if i % 2 == 0 { forest() } else { island() }));
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .format(Arc::new(Commander))
            .commanders(0, [card(GNARR).unwrap()])
            .hand_named(0, ["Snap"])
            .battlefield(0, lands.chain([ready(bear())]).collect::<Vec<_>>()),
        |scenario, seat| scenario.library(seat, vec![forest(); 8]),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let bear_id = driver.id_of("Grizzly Bears")?;
    merge(&mut driver, over)?;
    let name = driver.state().object(bear_id).unwrap().card().name.clone();
    let target = driver.target_named(&name)?;
    driver.cast_at("Snap", &[target])?;
    while !matches!(&driver.decision()?.kind,
        DecisionKind::YesNo(options) if options.text.contains("command zone"))
    {
        driver.answer_passively()?;
    }
    let before = driver.state().clone();
    let start = driver.engine().events().len();
    driver.answer(Answer::YesNo(YesNoChoice(true)))?;
    let events = driver.engine().events()[start..].to_vec();
    let gnarr = driver.state().commanders().next().unwrap();
    assert_eq!(
        driver.state().object(gnarr).unwrap().zone(),
        Zone::Command(PlayerId(0))
    );
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    assert_eq!(batches.len(), seats);
    for batch in batches {
        assert!(
            batch.events.iter().any(|view| matches!(view,
                EventView::Public(Event::CardParted { card, to: Zone::Command(_), .. }) if *card == gnarr)),
            "{:?}",
            batch.events
        );
    }
    Ok(())
}
