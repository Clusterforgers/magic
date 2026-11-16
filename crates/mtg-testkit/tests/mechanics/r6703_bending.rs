//! Goal: each of the four bendings (a waterbend cost paid, an earthbend, a firebending ability resolving, an airbend)
//! is logged for its player once, triggers "whenever you <bend>" of that player and "whenever an opponent <bend>s" of
//! the others at every table size, and counts for the turn, so Avatar Aang transforms exactly when all four were done
//! this turn (Avatar Aang rulings).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::refs::Ref;
use mtg_core::card::CardType;
use mtg_engine::events::{Bending, Event};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// An artifact with one ability: `{T}` plus `cost`, `targets`, then `effects` (JSON lists).
fn stone(name: &str, cost: &str, targets: &str, effects: &str) -> Arc<CardDefinition> {
    let json =
        format!(r#"{{"Activated":{{"cost":{cost},"targets":{targets},"effects":{effects}}}}}"#);
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str::<Ability>(&json).unwrap()),
    )
}

/// "Waterbend {1}: nothing", paid by a Forest.
fn water() -> Arc<CardDefinition> {
    stone("Water Stone", r#"[{"Waterbend":1}]"#, "[]", "[]")
}

/// "{T}: Earthbend 1." (target land you control).
fn earth() -> Arc<CardDefinition> {
    let effects = mtg_cards::macros::action_effects("earthbend", 1, &Ref::Target(0)).unwrap();
    stone(
        "Earth Stone",
        r#"["Tap"]"#,
        r#"[{"what":{"Object":[{"Type":"Land"},{"ControlledBy":"You"}]}}]"#,
        &serde_json::to_string(&effects).unwrap(),
    )
}

/// "{T}: Airbend up to one target creature."
fn air() -> Arc<CardDefinition> {
    stone(
        "Air Stone",
        r#"["Tap"]"#,
        r#"[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":1}}]"#,
        r#"[{"Exile":{"what":{"Target":0},"as":"airbent"}},{"Rule":{"rule":{"MayCastRather":{"only":{"Bound":"airbent"},"cost":"{2}"}},"duration":"Forever"}}]"#,
    )
}

/// A 2/2 with firebending 1.
fn fire() -> Arc<CardDefinition> {
    let rules: Vec<Ability> = mtg_cards::macros::keyword_abilities("firebending", 1).unwrap();
    let mut card = CardDefinition::new("Fire Cub")
        .with_type(CardType::Creature)
        .with_power_toughness(2, 2);
    for rule in rules {
        card = card.with_rules(rule);
    }
    Arc::new(card)
}

/// An enchantment: "Whenever <who> <action>, you gain 1 life."
fn watcher(action: &str, who: &str) -> Arc<CardDefinition> {
    let json = format!(
        r#"{{"Triggered":{{"when":{{"Did":{{"action":"{action}","who":"{who}"}}}},"effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new(format!("{who} {action} Watcher"))
            .with_type(CardType::Enchantment)
            .with_rules(serde_json::from_str::<Ability>(&json).unwrap()),
    )
}

fn resolve_all(driver: &mut Driver, log: &mut Vec<Event>) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty() || !driver.state().pending_triggers().is_empty() {
        log.extend(driver.answer_passively()?);
    }
    Ok(())
}

/// Seat 0 bends `element` once in its first turn, with `attacker` for firebending; the events it took.
fn bend(driver: &mut Driver, element: Bending, attacker: &str) -> Result<Vec<Event>, ScriptError> {
    let mut log = Vec::new();
    match element {
        Bending::Water => {
            log.extend(driver.begin_activation("Water Stone")?);
            log.extend(driver.answer_passively()?);
        }
        Bending::Earth => {
            log.extend(driver.begin_activation("Earth Stone")?);
            let land = driver.target_named("Forest")?;
            log.extend(driver.choose_targets(&[land])?);
        }
        Bending::Air => {
            log.extend(driver.begin_activation("Air Stone")?);
            log.extend(driver.choose_targets(&[])?);
        }
        Bending::Fire => {
            driver.pass_until(Step::DeclareAttackers)?;
            log.extend(driver.attack(&[(attacker, 1)])?);
        }
    }
    resolve_all(driver, &mut log)?;
    Ok(log)
}

fn board() -> Vec<mtg_engine::setup::PresetPermanent> {
    vec![
        ready(forest()),
        ready(water()),
        ready(earth()),
        ready(air()),
        ready(fire()),
    ]
}

fn bent(events: &[Event]) -> Vec<Bending> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Bent { player, element } if *player == pid(0) => Some(*element),
            _ => None,
        })
        .collect()
}

fn action_of(element: Bending) -> &'static str {
    match element {
        Bending::Water => "Waterbent",
        Bending::Earth => "Earthbent",
        Bending::Fire => "Firebent",
        Bending::Air => "Airbent",
    }
}

#[rstest]
fn each_bending_is_logged_once_and_triggers_its_watchers(
    #[values(Bending::Water, Bending::Earth, Bending::Fire, Bending::Air)] element: Bending,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let action = action_of(element);
    let others = ["Waterbent", "Earthbent", "Firebent", "Airbent"]
        .into_iter()
        .filter(|other| *other != action)
        .map(|other| ready(watcher(other, "You")));
    let mut scenario = Scenario::new(seats).battlefield(
        0,
        board()
            .into_iter()
            .chain([ready(watcher(action, "You"))])
            .chain(others),
    );
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            [
                ready(watcher(action, "Opponents")),
                ready(watcher(action, "You")),
            ],
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let log = bend(&mut driver, element, "Fire Cub")?;
    assert_eq!(bent(&log), [element]);
    let life = |driver: &Driver, seat: usize| driver.state().player(pid(seat)).unwrap().life();
    assert_eq!(
        life(&driver, 0),
        21,
        "only the watcher of this bending triggers"
    );
    for seat in 1..seats {
        assert_eq!(
            life(&driver, seat),
            21,
            "an opponent's watcher of opponents triggers, not theirs"
        );
    }
    Ok(())
}

/// Seat 0 controls Avatar Aang and the bending sources, and bends the elements in order; Aang attacks for its own
/// firebending when `Fire` is among them.
fn aang_after(elements: &[Bending], seats: usize) -> Result<(Driver, Vec<Event>), ScriptError> {
    let aang = card("Avatar Aang // Aang, Master of Elements").unwrap();
    let mut board = board();
    board.push(ready(aang));
    let library: Vec<_> = (0..10).map(|_| forest()).collect();
    let mut scenario = Scenario::new(seats)
        .battlefield(0, board)
        .library(0, library);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Bystander", 1, 1))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let mut log = Vec::new();
    for element in elements {
        log.extend(bend(&mut driver, *element, "Avatar Aang")?);
    }
    Ok((driver, log))
}

#[rstest]
#[case::all_four(&[Bending::Water, Bending::Earth, Bending::Air, Bending::Fire], true)]
#[case::three_of_them(&[Bending::Water, Bending::Earth, Bending::Air], false)]
#[case::three_with_fire(&[Bending::Water, Bending::Air, Bending::Fire], false)]
fn avatar_aang_draws_for_each_bending_and_transforms_after_all_four(
    #[case] elements: &[Bending],
    #[case] transforms: bool,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (driver, log) = aang_after(elements, seats)?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    assert_eq!(hand, elements.len(), "one card for each bending");
    let aang = driver.id_of("Avatar Aang")?;
    let transformed = log
        .iter()
        .any(|event| matches!(event, Event::Transformed { object, .. } if *object == aang));
    assert_eq!(transformed, transforms);
    Ok(())
}
