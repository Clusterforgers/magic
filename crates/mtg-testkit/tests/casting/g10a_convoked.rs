//! Goal: a spell remembers the creatures that convoked it (CR 702.51) and the permanent it becomes keeps them: "put a +1/+1 counter on each creature that convoked it" as it enters reaches exactly those creatures, "enters with two +1/+1 counters for each creature that convoked it" counts them, a spell cast without convoking names none, and the log replays to the same state; at two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_testkit::cards::{creature, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// What the convoke spell does with the creatures that convoked it.
#[derive(Debug, Clone, Copy)]
enum Reads {
    /// Venerated Loxodon: when it enters, a +1/+1 counter on each of them.
    Counters,
    /// Ancient Imperiosaur: it enters with two +1/+1 counters for each of them.
    Enters,
}

/// A `{3}` creature with convoke that reads the creatures that convoked it.
fn elephant(reads: Reads) -> Arc<CardDefinition> {
    let rules = match reads {
        Reads::Counters => {
            r#"{"Triggered":{"when":{"ZoneChange":{"to":"Battlefield","what":["This"]}},"effects":[{"PutCounters":{"counter":"+1/+1","on":"Convoked"}}]}}"#
        }
        Reads::Enters => {
            r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":"+1/+1","count":{"Times":{"by":2,"of":{"CountOf":{"of":"Convoked","filter":[]}}}}}}}}"#
        }
    };
    let rules: Ability = serde_json::from_str(rules).unwrap();
    Arc::new(
        CardDefinition::new("Elephant")
            .with_type(CardType::Creature)
            .with_mana_cost(ManaCost::parse("{3}").unwrap())
            .with_power_toughness(2, 2)
            .with_rules(Ability::Keyword(Keyword::Convoke))
            .with_rules(rules),
    )
}

/// The +1/+1 counters on the named permanent.
fn counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |p| p.counters_of(&CounterKind::PlusOnePlusOne))
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

#[rstest]
fn the_creatures_that_convoked_it_are_the_ones_it_reads(
    #[values(2, 3, 4)] seats: usize,
    #[values(Reads::Counters, Reads::Enters)] reads: Reads,
    #[values(0, 2, 3)] convoking: usize,
) -> Result<(), ScriptError> {
    let golems = ["Golem A", "Golem B", "Golem C"];
    let mut seat: Vec<_> = golems
        .iter()
        .map(|name| ready(creature(name, 2, &[], 2, 2)))
        .collect();
    seat.extend((0..3).map(|_| ready(plains())));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, seat)
            .hand(0, [elephant(reads)]),
    )?;
    driver.begin_cast("Elephant")?;
    let lands: Vec<&str> = (convoking..3).map(|_| "Plains").collect();
    let helpers: Vec<(&str, Option<mtg_core::color::Color>)> = golems[..convoking]
        .iter()
        .map(|name| (*name, None))
        .collect();
    driver.pay_helped(&lands, &helpers, &[], 0)?;
    settle(&mut driver)?;
    for (at, golem) in golems.iter().enumerate() {
        let expected = match reads {
            Reads::Counters => u32::from(at < convoking),
            Reads::Enters => 0,
        };
        assert_eq!(counters(&driver, golem), expected, "{golem}");
    }
    let own = match reads {
        Reads::Counters => 0,
        Reads::Enters => 2 * u32::try_from(convoking).unwrap(),
    };
    assert_eq!(counters(&driver, "Elephant"), own);
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
