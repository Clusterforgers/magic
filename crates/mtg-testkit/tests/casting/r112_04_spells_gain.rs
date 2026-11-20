//! Goal: a rule "as you cast spells from your hand this turn, they gain cascade" (`Rule::R112SpellsGain`) gives the
//! keyword to each spell its controller casts that its selector picks as it is cast, so the cascade trigger goes on the
//! stack above the spell; a spell of another player or one the selector leaves out gains nothing, at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Selector, ZoneKind};
use mtg_core::color::Color;
use mtg_engine::effects::Duration;
use mtg_engine::rules::Rule;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{creature, forest};
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

use crate::common::casting::lands;

fn bear(name: &str, generic: u32) -> Arc<CardDefinition> {
    creature(name, generic, &[Color::Green], 2, 2)
}

#[rstest]
fn r112_a_spell_cast_from_hand_gains_cascade_as_it_is_cast(
    #[values(2, 3, 4)] seats: usize,
    #[values(ZoneKind::Hand, ZoneKind::Graveyard)] zone: ZoneKind,
) {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [bear("Big Bear", 2)])
        .battlefield(0, lands(3, forest))
        .library(0, [bear("Cub", 0), bear("Cub", 0)]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver
        .add_rule(
            Rule::R112SpellsGain {
                what: Selector(vec![Pred::In(zone)]),
                keyword: Keyword::Cascade,
            },
            Duration::UntilEndOfTurn,
        )
        .unwrap();
    driver
        .cast_tapping("Big Bear", &["Forest", "Forest", "Forest"])
        .unwrap();
    let spell = driver
        .state()
        .objects()
        .find(|object| object.card().name == "Big Bear")
        .unwrap()
        .id();
    let gained =
        mtg_engine::keywords::keywords_of(driver.state(), spell).contains(&&Keyword::Cascade);
    assert_eq!(gained, zone == ZoneKind::Hand);
    // The cascade trigger waits above the spell, or nothing does.
    let waiting = driver.state().stack().len() + driver.state().pending_triggers().len();
    assert_eq!(waiting, if gained { 2 } else { 1 });
}
