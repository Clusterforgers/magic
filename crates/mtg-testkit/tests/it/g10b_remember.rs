//! Goal: a counter and an attach with an `as` name bind what they did at tables of two, three and four seats: the countered
//! spell as the card it became (a later step returns it to its owner's hand) and none when the spell can't be countered,
//! the Equipment that became attached and none when nothing could be attached, so "if you do" reads the outcome.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::removal::spell_named;
use common::{pid, spell};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An instant that does nothing, which can't be countered when `sturdy`.
fn victim(sturdy: bool) -> Arc<CardDefinition> {
    let card = spell("Victim", CardType::Instant, "[]", "[]");
    if !sturdy {
        return card;
    }
    let rule: Ability =
        serde_json::from_str(r#"{"Rule":{"from":"Stack","rule":{"CantBeCountered":"This"}}}"#)
            .unwrap();
    Arc::new((*card).clone().with_rules(rule))
}

/// Counters target spell; the spell countered this way returns to its owner's hand.
fn snatch() -> Arc<CardDefinition> {
    spell(
        "Snatch",
        CardType::Instant,
        r#"[{"what":{"Object":["Spell"]}}]"#,
        r#"[{"Counter":{"what":{"Target":0},"as":"countered"}},{"Move":{"what":{"Bound":"countered"},"to":"Hand"}}]"#,
    )
}

fn pass_until_priority(driver: &mut Driver, seat: usize) -> Result<(), ScriptError> {
    for _ in 0..16 {
        if driver.decision()?.player == pid(seat) {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck(format!("priority for seat {seat}")))
}

#[rstest]
#[case::countered_it_returns(false, Zone::Hand(pid(0)))]
#[case::it_cant_be_countered_and_resolves(true, Zone::Graveyard(pid(0)))]
fn the_spell_countered_this_way_is_the_card_it_became(
    #[case] sturdy: bool,
    #[case] lands: Zone,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [victim(sturdy)])
            .hand(1, [snatch()]),
    )?;
    driver.cast("Victim")?;
    pass_until_priority(&mut driver, 1)?;
    let target = spell_named(&driver, "Victim");
    driver.cast_at("Snatch", &[target])?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    driver.expect_zone(lands, &["Victim"])?;
    driver.expect_zone(Zone::Graveyard(pid(1)), &["Snatch"])
}

/// An Equipment.
fn blade() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Blade")
            .with_type(CardType::Artifact)
            .with_subtype("Equipment"),
    )
}

/// Attaches the permanents named `what` to target creature; if one became attached, its controller draws a card.
fn hook(what: &str) -> Arc<CardDefinition> {
    spell(
        "Hook",
        CardType::Sorcery,
        r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
        &format!(
            r#"[{{"Attach":{{"what":{{"Each":[{{"Named":"{what}"}}]}},"to":{{"Target":0}},"as":"attached"}}}},{{"If":{{"condition":{{"CountOf":{{"subject":{{"Bound":"attached"}},"selector":[],"op":"Ge","right":1}}}},"then":[{{"Draw":{{}}}}]}}}}]"#
        ),
    )
}

#[rstest]
#[case::an_equipment_becomes_attached("Blade", true)]
#[case::a_creature_is_never_attached("Ogre", false)]
fn if_you_do_reads_what_became_attached(
    #[case] what: &str,
    #[case] drew: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .hand(0, [hook(what)])
        .battlefield(
            0,
            [
                ready(blade()),
                ready(free_creature("Bear", 2, 2)),
                ready(free_creature("Ogre", 3, 3)),
            ],
        )
        .library(0, (0..5).map(|_| free_creature("Filler", 1, 1)));
    for seat in 1..seats {
        scenario = scenario.library(seat, (0..5).map(|_| free_creature("Filler", 1, 1)));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let bear = driver.target_named("Bear")?;
    driver.cast_at("Hook", &[bear])?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    let state = driver.state();
    let named = |name: &str| {
        state
            .zone(Zone::Battlefield)
            .iter()
            .copied()
            .find(|id| state.object(*id).is_some_and(|o| o.card().name == name))
            .unwrap()
    };
    let bear = named("Bear");
    let attached = state.host_of(named(what)) == Some(bear);
    assert_eq!(attached, drew);
    let hand = state.zone(Zone::Hand(pid(0))).len();
    assert_eq!(hand, usize::from(drew));
    Ok(())
}
