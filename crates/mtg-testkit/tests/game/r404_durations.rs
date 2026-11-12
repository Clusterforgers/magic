//! Goal: control gained "for as long as that creature is enchanted" holds while an Aura stays on the creature and ends
//! when the last one leaves (or at once when none was there), and control gained "for as long as they're the monarch"
//! holds while that player stays the monarch and ends when another player becomes it, at tables of two to four seats;
//! the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, free_creature, island};
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

fn ability(json: serde_json::Value) -> Ability {
    serde_json::from_value(json).unwrap()
}

/// The creature of Rootwater Matriarch: "{T}: Gain control of target creature for as long as that creature is enchanted."
fn matriarch() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Matriarch")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 3)
            .with_rules(ability(serde_json::json!({"Activated": {
                "cost": ["Tap"],
                "targets": [{"what": {"Object": [{"Type": "Creature"}]}}],
                "effects": [{"Continuous": {"affected": {"Target": 0}, "mods": [{"SetController": "You"}],
                    "duration": "WhileEnchanted"}}]}}))),
    )
}

/// An Aura that enchants a creature and does nothing else.
fn aura(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Enchantment)
            .with_subtype("Aura")
            .with_rules(ability(
                serde_json::json!({"Keyword": {"Enchant": [{"Type": "Creature"}]}}),
            )),
    )
}

fn controller(driver: &Driver, name: &str) -> Option<PlayerId> {
    mtg_engine::characteristics::controller_of(driver.state(), driver.id_of(name).ok()?)
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..30 {
        if driver.state().stack().is_empty() && driver.state().pending_triggers().is_empty() {
            break;
        }
        driver.answer_passively()?;
    }
    Ok(())
}

/// The Auras on the Bear and what happens to them after it is taken.
#[derive(Debug, Clone, Copy)]
enum Auras {
    /// One Aura that stays.
    Stays,
    /// One Aura that its owner puts into the graveyard.
    Leaves,
    /// Two Auras, one of which leaves.
    OneOfTwoLeaves,
    /// No Aura at all: the control ends as it begins.
    None,
}

#[rstest]
fn control_lasts_while_the_creature_is_enchanted(
    #[values(2, 3, 4)] seats: usize,
    #[values(Auras::Stays, Auras::Leaves, Auras::OneOfTwoLeaves, Auras::None)] auras: Auras,
) -> Result<(), ScriptError> {
    let game = || {
        let mut theirs = vec![ready(free_creature("Bear", 2, 2))];
        if !matches!(auras, Auras::None) {
            theirs.push(attached_to(ready(aura("Aura")), 1, 0));
        }
        if matches!(auras, Auras::OneOfTwoLeaves) {
            theirs.push(attached_to(ready(aura("Second Aura")), 1, 0));
        }
        let mut scenario = Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .battlefield(0, [ready(matriarch())])
            .battlefield(1, theirs);
        for seat in 0..seats {
            scenario = scenario.library(seat, (0..20).map(|_| island()));
        }
        scenario
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    driver.begin_activation("Matriarch")?;
    let bear = driver.target_named("Bear")?;
    driver.choose_targets(&[bear])?;
    settle(&mut driver)?;
    if matches!(auras, Auras::Leaves | Auras::OneOfTwoLeaves) {
        assert_eq!(controller(&driver, "Bear"), Some(PlayerId(0)));
        let object = driver.id_of("Aura")?;
        driver.manual_by(
            PlayerId(1),
            &ManualAction::Move {
                object,
                to: ManualZone::Graveyard,
            },
        )?;
    }
    driver.idle_until(3, Step::Main1)?;
    let expected = match auras {
        Auras::Stays | Auras::OneOfTwoLeaves => PlayerId(0),
        Auras::Leaves | Auras::None => PlayerId(1),
    };
    assert_eq!(controller(&driver, "Bear"), Some(expected), "{auras:?}");
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

/// A free instant with one spell ability.
fn instant(name: &str, json: serde_json::Value) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(ability(json)),
    )
}

/// Garland's second ability: "Whenever an opponent becomes the monarch, gain control of target creature that player
/// controls for as long as they're the monarch."
fn kidnapper() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Kidnapper")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_rules(ability(serde_json::json!({"Triggered": {
                "when": {"BecomesMonarch": {"who": "Opponents"}},
                "targets": [{"what": {"Object": [{"Type": "Creature"}, {"ControlledBy": "Triggering"}]}}],
                "effects": [{"Continuous": {"affected": {"Target": 0}, "mods": [{"SetController": "You"}],
                    "duration": {"WhileMonarch": "Triggering"}}}]}}))),
    )
}

/// Who becomes the monarch after the Bear is taken.
#[derive(Debug, Clone, Copy)]
enum Next {
    /// Nobody: its owner stays the monarch.
    Nobody,
    /// The player who took it.
    Taker,
    /// Another opponent (only at three seats or more).
    Other,
}

#[rstest]
fn control_lasts_while_that_player_is_the_monarch(
    #[values(2, 3, 4)] seats: usize,
    #[values(Next::Nobody, Next::Taker, Next::Other)] next: Next,
) -> Result<(), ScriptError> {
    if matches!(next, Next::Other) && seats < 3 {
        return Ok(());
    }
    let game = || {
        let mut scenario = Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .battlefield(0, [ready(kidnapper())])
            .battlefield(1, [ready(free_creature("Bear", 2, 2))])
            .hand(
                0,
                [
                    instant("Crown Them", serde_json::json!({"Spell": {
                        "targets": [{"what": "Opponent"}],
                        "effects": [{"BecomeMonarch": {"who": {"Target": 0}}}]}})),
                    instant("Crown Another", serde_json::json!({"Spell": {
                        "targets": [{"what": "Opponent"}],
                        "effects": [{"BecomeMonarch": {"who": {"Target": 0}}}]}})),
                    instant("Crown Me", serde_json::json!({"Spell": {
                        "effects": [{"BecomeMonarch": {"who": "You"}}]}})),
                ],
            );
        for seat in 0..seats {
            scenario = scenario.library(seat, (0..20).map(|_| island()));
        }
        scenario
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    let them = driver.target_player(1);
    driver.cast_at("Crown Them", &[them])?;
    settle(&mut driver)?;
    assert_eq!(controller(&driver, "Bear"), Some(PlayerId(0)));
    match next {
        Next::Nobody => {}
        Next::Taker => {
            driver.cast("Crown Me")?;
            settle(&mut driver)?;
        }
        Next::Other => {
            let other = driver.target_player(2);
            driver.cast_at("Crown Another", &[other])?;
            settle(&mut driver)?;
        }
    }
    let expected = match next {
        Next::Nobody => PlayerId(0),
        Next::Taker | Next::Other => PlayerId(1),
    };
    assert_eq!(controller(&driver, "Bear"), Some(expected), "{next:?}");
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
