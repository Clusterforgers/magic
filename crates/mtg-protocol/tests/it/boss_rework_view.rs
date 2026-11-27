//! Goal: at two, three and four seats every seat sees the same reworked boss view: innate with its level, modifier,
//! relics, both phases by round with the countdown to the next (which moves on as a phase begins), and the boss's
//! Satchel (public) with each card's reusable or once mark and its tax after a cast; a seat without a boss leaves the
//! field out of its JSON; the boss names its personality with whom it hunts.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::PlayerId;
use mtg_core::personality::Personality;
use mtg_engine::boss_sheet::{BossSheet, SheetCard};
use mtg_engine::phases::{BossPhase, PhaseStart};
use mtg_engine::turn::Step;
use mtg_engine::w6b_emblems::emblem_definition;
use mtg_protocol::TableView;
use mtg_protocol::view::{BossPartView, BossView, InnateView};
use mtg_testkit::cards::mountain;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The boss's seat.
const BOSS: usize = 0;

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

fn sheet_card(name: &str, text: &str) -> SheetCard {
    SheetCard {
        name: name.to_owned(),
        text: text.to_owned(),
    }
}

fn phase(name: &str, round: u32) -> BossPhase {
    let emblem: Arc<CardDefinition> = Arc::new(emblem_definition(name, &[]));
    BossPhase {
        begins: PhaseStart::AtRound(round),
        emblem,
    }
}

/// The boss holds Lightning Bolt (reusable) and Giant Growth (once) in its Satchel, three Mountains, Brood III, Deep
/// Pockets, two copies of a relic, and phases at rounds 2 and 3.
fn scenario(seats: usize) -> Scenario {
    let sheet = BossSheet {
        innate: Some(sheet_card("Brood III", "Create 3 Rats.")),
        level: 3,
        modifier: Some(sheet_card("Deep Pockets", "Draw an additional card.")),
        relics: vec![
            sheet_card("Whetstone", "+1/+0."),
            sheet_card("Whetstone", "+1/+0."),
        ],
        phases: vec![
            sheet_card("The Nest Stirs", "Rats have menace."),
            sheet_card("Plague Tide", "Tokens get +2/+0."),
        ],
    };
    Scenario::new(seats)
        .boss_sheet(BOSS, sheet)
        .personality(BOSS, personality_at(seats))
        .phases(BOSS, [phase("The Nest Stirs", 2), phase("Plague Tide", 3)])
        .satchel_reusable(BOSS, [card("Lightning Bolt").unwrap()])
        .satchel(BOSS, [card("Giant Growth").unwrap()])
        .battlefield(BOSS, (0..3).map(|_| ready(mountain())))
}

/// The boss's personality at `seats` seats: the default at two.
fn personality_at(seats: usize) -> Personality {
    Personality::ALL[(seats - 2) % Personality::ALL.len()]
}

/// The boss view of every seat, checked to be the same for each viewer.
fn shared(driver: &Driver, seats: usize) -> BossView {
    let views: Vec<BossView> = (0..seats)
        .map(|viewer| {
            TableView::of(driver.state(), seat(viewer)).players[BOSS]
                .boss
                .clone()
                .expect("the boss seat has a boss view")
        })
        .collect();
    for (viewer, view) in views.iter().enumerate() {
        assert_eq!(view, &views[0], "seat {viewer} sees the same boss");
    }
    views[0].clone()
}

#[rstest]
fn every_seat_sees_the_reworked_boss(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario(seats))?;
    driver.idle_until(1, Step::Main1)?;
    let view = shared(&driver, seats);
    assert_eq!(
        view.innate,
        Some(InnateView {
            name: "Brood III".into(),
            text: "Create 3 Rats.".into(),
            level: 3,
        })
    );
    assert_eq!(
        view.modifier.as_ref().map(|m| m.name.as_str()),
        Some("Deep Pockets")
    );
    let personality = personality_at(seats);
    assert_eq!(
        view.personality,
        Some(BossPartView {
            name: personality.name().into(),
            text: personality.text().into(),
        })
    );
    assert_eq!(view.relics.len(), 2, "copies stack");
    assert!(view.relics.iter().all(|r| *r
        == BossPartView {
            name: "Whetstone".into(),
            text: "+1/+0.".into(),
        }));
    let phases: Vec<(&str, &str, u32, bool)> = view
        .round_phases
        .iter()
        .map(|p| (p.name.as_str(), p.text.as_str(), p.round, p.begun))
        .collect();
    assert_eq!(
        phases,
        [
            ("The Nest Stirs", "Rats have menace.", 2, false),
            ("Plague Tide", "Tokens get +2/+0.", 3, false),
        ]
    );
    assert_eq!(
        view.phase_in,
        Some(2),
        "round 1: rounds 1 and 2 still to end"
    );
    let satchel: Vec<(&str, bool, u32)> = view
        .satchel
        .iter()
        .map(|c| (c.name.as_str(), c.reusable, c.tax))
        .collect();
    assert_eq!(
        satchel,
        [("Giant Growth", false, 0), ("Lightning Bolt", true, 0)]
    );
    assert!(view.satchel.iter().all(|c| c.object.is_some()));
    for other in 1..seats {
        let json =
            serde_json::to_value(&TableView::of(driver.state(), seat(0)).players[other]).unwrap();
        assert!(json.get("boss").is_none(), "a hero has no boss view");
    }
    Ok(())
}

#[rstest]
fn a_reusable_cast_raises_its_tax_for_every_seat(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario(seats))?;
    driver.idle_until(1, Step::Main1)?;
    let target = driver.target_player(1);
    driver.cast_at("Lightning Bolt", &[target])?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    let view = shared(&driver, seats);
    let bolt = view
        .satchel
        .iter()
        .find(|c| c.name == "Lightning Bolt")
        .unwrap();
    assert_eq!((bolt.reusable, bolt.tax), (true, 2), "back with {{2}} more");
    Ok(())
}

#[rstest]
fn the_countdown_moves_on_as_a_phase_begins(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario(seats))?;
    let mut seen = Vec::new();
    for _ in 0..20_000 {
        let view = shared(&driver, seats);
        let begun: Vec<bool> = view.round_phases.iter().map(|p| p.begun).collect();
        let entry = (driver.state().turn().round(), begun, view.phase_in);
        if seen.last() != Some(&entry) {
            seen.push(entry);
        }
        if view.round_phases.iter().all(|p| p.begun) {
            break;
        }
        driver.answer_passively()?;
    }
    let last = seen.last().unwrap();
    assert_eq!(last.1, [true, true]);
    assert_eq!(last.2, None, "no countdown once every phase began");
    assert!(
        seen.iter().any(|(round, begun, phase_in)| *round == 2
            && *begun == [false, false]
            && *phase_in == Some(1)),
        "round 2 counts the phase in at its own end: {seen:?}"
    );
    assert!(
        seen.iter()
            .any(|(_, begun, phase_in)| *begun == [true, false] && phase_in.is_some()),
        "after Phase 1 the countdown is to Phase 2: {seen:?}"
    );
    Ok(())
}
