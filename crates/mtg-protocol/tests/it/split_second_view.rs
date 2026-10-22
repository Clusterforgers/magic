//! Goal: the legal actions the wire carries to each seat while a spell with split second is on the stack hold no spell and no ability that is not a mana ability, at two to four seats: the priority decision of every seat in turn lists no castable card and no activatable ability (the field is left out), says nothing is left to do, still lists the mana abilities and passing, and the seat that sees the stack empty again gets its spells and abilities back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_testkit::cards::mountain;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::Value;

/// Every seat holds a Lightning Bolt, a Mother of Runes, a Sol Ring and Mountains; seat 0 also holds a Sudden Shock.
fn table(seats: usize) -> Driver {
    let mut scenario = Scenario::new(seats);
    for seat in 0..seats {
        let hand = if seat == 0 {
            vec![
                card("Sudden Shock").unwrap(),
                card("Lightning Bolt").unwrap(),
            ]
        } else {
            vec![card("Lightning Bolt").unwrap()]
        };
        scenario = scenario.hand(seat, hand).battlefield(
            seat,
            (0..3)
                .map(|_| ready(mountain()))
                .chain([
                    ready(card("Mother of Runes").unwrap()),
                    ready(card("Sol Ring").unwrap()),
                ])
                .collect::<Vec<_>>(),
        );
    }
    Driver::from_scenario(scenario).unwrap()
}

/// The priority options of the pending decision as the wire writes them, and who is asked.
fn wire(driver: &Driver) -> (PlayerId, Value) {
    let decision = driver.decision().unwrap();
    let value = serde_json::to_value(decision).unwrap();
    let options = value
        .pointer("/kind/Priority")
        .cloned()
        .unwrap_or_else(|| panic!("a priority decision, not {value}"));
    (decision.player, options)
}

fn list(options: &Value, field: &str) -> usize {
    options
        .get(field)
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

#[rstest]
fn every_seat_is_offered_mana_and_passing_only_while_the_spell_is_on_the_stack(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = table(seats);
    let (_, before) = wire(&driver);
    assert!(list(&before, "castable") > 0 && list(&before, "abilities") > 0);
    driver.begin_cast("Sudden Shock").unwrap();
    let target = driver.target_player(1);
    driver.choose_targets(&[target]).unwrap();
    driver.pay_tapping(&["Mountain", "Mountain"], 0).unwrap();
    for seat in 0..seats {
        let (asked, options) = wire(&driver);
        assert_eq!(asked, PlayerId::from_index(seat));
        assert_eq!(list(&options, "castable"), 0, "seat {seat}");
        assert!(options.get("abilities").is_none(), "seat {seat}: {options}");
        assert!(list(&options, "mana_abilities") > 0, "seat {seat}");
        assert_eq!(options["can_act"], Value::Bool(false), "seat {seat}");
        assert_eq!(options["can_pass"], Value::Bool(true), "seat {seat}");
        driver.pass().unwrap();
    }
    let (asked, after) = wire(&driver);
    assert_eq!(asked, PlayerId::from_index(0));
    assert!(list(&after, "castable") > 0 && list(&after, "abilities") > 0);
    assert_eq!(after["can_act"], Value::Bool(true));
}
