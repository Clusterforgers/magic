//! Goal: cards a permanent exiled face down for no player to look at stay linked to it (CR 406.3, 607.2a): its ability that sacrifices it as a cost and its leaves-the-battlefield trigger both put "the cards exiled with it" into their owners' hands, every seat sees the cards face down until then, and the log replays to the same state; at two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, spell};

const SECRETS: [&str; 2] = ["Secret One", "Secret Two"];

/// The cards exiled with it go to their owners' hands: by a sacrifice as the cost of an ability, or by its dies trigger.
#[derive(Debug, Clone, Copy)]
enum Back {
    Sacrifice,
    Dies,
}

/// A Courier with "{0}: exile the top two cards of your library face down" and the way the cards come back.
fn courier(back: Back) -> Arc<CardDefinition> {
    let seal: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":[],"effects":[{"ExileTop":{"count":2,"as":"sealed","sealed":true}}]}}"#,
    )
    .unwrap();
    let home = r#"{"Move":{"to":"Hand","what":{"Each":[{"ExiledWith":"This"},{"In":"Exile"}]}}}"#;
    let back: Ability = serde_json::from_str(&match back {
        Back::Sacrifice => format!(r#"{{"Activated":{{"cost":["SacrificeSelf"],"effects":[{home}]}}}}"#),
        Back::Dies => format!(
            r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"from":"Battlefield","to":"Graveyard"}}}},"effects":[{home}]}}}}"#
        ),
    })
    .unwrap();
    Arc::new(
        CardDefinition::new("Courier")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(seal)
            .with_rules(back),
    )
}

/// Passes priority until the stack is empty and nothing waits to go on it.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..16 {
        let priority = matches!(driver.decision()?.kind, DecisionKind::Priority(_));
        if priority
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.pass()?;
    }
    Ok(())
}

/// The names of the cards in seat 0's hand, sorted.
fn hand(driver: &Driver) -> Vec<String> {
    let mut found = names(driver.state(), Zone::Hand(PlayerId(0)));
    found.retain(|name| SECRETS.contains(&name.as_str()));
    found.sort();
    found
}

#[rstest]
fn the_cards_exiled_with_it_come_back_after_it_leaves(
    #[values(2, 3, 4)] seats: usize,
    #[values(Back::Sacrifice, Back::Dies)] back: Back,
) -> Result<(), ScriptError> {
    let smite = spell(
        "Smite",
        CardType::Instant,
        r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
        r#"[{"Destroy":{"what":{"Target":0}}}]"#,
    );
    let mut library: Vec<_> = SECRETS
        .iter()
        .map(|name| free_creature(name, 2, 2))
        .collect();
    library.extend((0..4).map(|i| free_creature(&format!("Pad {i}"), 1, 1)));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, [ready(courier(back))])
            .library(0, library)
            .hand(0, [smite]),
    )?;
    driver.begin_ability("Courier", 0)?;
    settle(&mut driver)?;
    let exiled: Vec<_> = driver.state().zone(Zone::Exile).to_vec();
    assert_eq!(exiled.len(), 2);
    for id in &exiled {
        for seat in 0..seats {
            assert!(!driver.state().may_look_at(PlayerId::from_index(seat), *id));
        }
    }
    match back {
        Back::Sacrifice => {
            driver.begin_ability("Courier", 1)?;
        }
        Back::Dies => {
            let target = driver.target_named("Courier")?;
            driver.cast_at("Smite", &[target])?;
        }
    }
    settle(&mut driver)?;
    assert_eq!(hand(&driver), SECRETS);
    assert_eq!(
        driver.state().zone(Zone::Exile),
        &[] as &[mtg_core::ids::ObjectId]
    );
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
