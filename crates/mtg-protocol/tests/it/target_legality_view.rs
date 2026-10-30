//! Goal: `ObjectView.targets_legal` says, in the shape of `targets` and alike for every seat, whether each target of a spell on the stack is still legal as the resolution checks it: a target that left, came back as a new object or gained hexproof is illegal, one that stays or a player is legal, a spell whose every target is illegal fizzles, and a card without targets leaves the field out of the JSON.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::Event;
use mtg_engine::choose::Target;
use mtg_engine::effects::{Duration, Modification, Selector};
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_protocol::{ObjectView, StateView};
use mtg_testkit::cards::{bear, burn, giant, mountain, twin_burn};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// What happens to a creature target while the spell waits on the stack.
#[derive(Debug, Clone, Copy)]
enum Fate {
    Stays,
    Dies,
    Returns,
    GainsHexproof,
}

/// Seat 0 has Bolt, Twin Bolt and lands; seat 1 has a Bear and a Giant.
fn table() -> Driver {
    Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [burn("Bolt", 3), twin_burn("Twin Bolt")])
            .battlefield(0, (0..4).map(|_| ready(mountain())))
            .battlefield(1, [ready(bear()), ready(giant())]),
    )
    .unwrap()
}

fn befall(driver: &mut Driver, name: &str, fate: Fate) {
    let object = driver.id_of(name).unwrap();
    // The card keeps its id; its zone-change counter tells the new object from the old (CR 400.7).
    let to = |driver: &mut Driver, zone| {
        driver
            .manual_by(PlayerId(1), &ManualAction::Move { object, to: zone })
            .unwrap();
    };
    match fate {
        Fate::Stays => {}
        Fate::Dies => to(driver, ManualZone::Graveyard),
        Fate::Returns => {
            to(driver, ManualZone::Graveyard);
            to(driver, ManualZone::Battlefield);
        }
        Fate::GainsHexproof => {
            let this = Selector::Objects(vec![driver.state().object(object).unwrap().reference()]);
            let hexproof = Modification::AddKeyword(mtg_cards::keyword::Keyword::Hexproof);
            driver
                .add_effect_from(object, this, hexproof, Duration::UntilEndOfTurn)
                .unwrap();
        }
    }
}

fn top(driver: &Driver, viewer: usize) -> ObjectView {
    let id = *driver.state().stack().last().unwrap();
    StateView::for_player(driver.state(), PlayerId::from_index(viewer))
        .objects
        .into_iter()
        .find(|o| o.id == id)
        .unwrap()
}

#[rstest]
#[case::stays(Fate::Stays, true)]
#[case::dies(Fate::Dies, false)]
#[case::returns_as_a_new_object(Fate::Returns, false)]
#[case::gains_hexproof(Fate::GainsHexproof, false)]
fn a_creature_target_is_legal_while_the_resolution_would_find_it(
    #[case] fate: Fate,
    #[case] legal: bool,
) {
    let mut driver = table();
    let at = driver.target_named("Grizzly Bears").unwrap();
    driver.cast_at("Bolt", &[at]).unwrap();
    befall(&mut driver, "Grizzly Bears", fate);
    for viewer in 0..2 {
        let shown = top(&driver, viewer);
        assert_eq!(shown.targets, [vec![at]]);
        assert_eq!(shown.targets_legal, [vec![legal]], "seat {viewer}");
    }
    let mut events: Vec<Event> = Vec::new();
    while !driver.state().stack().is_empty() {
        events.extend(driver.pass().unwrap());
    }
    let fizzled = events.iter().any(|event| {
        matches!(
            event,
            Event::ResolutionEnded {
                outcome: ResolutionOutcome::Fizzled,
                ..
            }
        )
    });
    assert_eq!(fizzled, !legal, "the view foretells the resolution");
}

#[rstest]
#[case::both_stay(Fate::Stays, [true, true])]
#[case::first_dies(Fate::Dies, [false, true])]
fn each_target_specification_has_its_own_list(#[case] fate: Fate, #[case] legal: [bool; 2]) {
    let mut driver = table();
    let bear = driver.target_named("Grizzly Bears").unwrap();
    let player = driver.target_player(1);
    driver.begin_cast("Twin Bolt").unwrap();
    driver.choose_targets(&[bear]).unwrap();
    driver.choose_targets(&[player]).unwrap();
    driver
        .answer(mtg_engine::decision::Answer::PayMana(
            mtg_engine::decision::PayChoice::Auto,
        ))
        .unwrap();
    befall(&mut driver, "Grizzly Bears", fate);
    for viewer in 0..2 {
        let shown = top(&driver, viewer);
        assert_eq!(shown.targets, [vec![bear], vec![player]]);
        assert_eq!(shown.targets_legal, [vec![legal[0]], vec![legal[1]]]);
    }
    assert!(matches!(player, Target::Player(_)));
}

#[test]
fn the_field_is_left_out_of_the_json_without_targets() {
    let mut driver = table();
    let at = driver.target_player(1);
    driver.cast_at("Bolt", &[at]).unwrap();
    let json = serde_json::to_value(top(&driver, 1)).unwrap();
    assert_eq!(json["targets_legal"], serde_json::json!([[true]]));
    let view = StateView::for_player(driver.state(), PlayerId(1));
    for object in view.objects.iter().filter(|o| o.targets.is_empty()) {
        let json = serde_json::to_value(object).unwrap();
        assert!(json.get("targets_legal").is_none(), "{}", object.name);
    }
}
