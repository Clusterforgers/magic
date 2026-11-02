//! Goal: the key of a per-ability yield is read off an ability on the stack as every seat reads it (its source's name and its words), and nothing else on or off the stack has one.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_protocol::{AutoYield, StateView};
use mtg_testkit::cards::{burn, mountain, pinger};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// What is on the stack when the key is asked for.
#[derive(Debug, Clone, Copy)]
enum Top {
    Ability,
    Spell,
}

/// A duel with `top` on the stack, controlled by seat 0.
fn duel(top: Top) -> Driver {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [burn("Bolt", 3)])
            .battlefield(0, [ready(pinger("Pinger", 1)), ready(mountain())]),
    )
    .unwrap();
    let at = driver.target_player(1);
    match top {
        Top::Ability => {
            driver.begin_activation("Pinger").unwrap();
            driver.choose_targets(&[at]).unwrap();
        }
        Top::Spell => {
            driver.cast_at("Bolt", &[at]).unwrap();
        }
    }
    driver
}

#[test]
fn an_ability_on_the_stack_is_keyed_by_what_every_seat_reads_on_it() {
    let driver = duel(Top::Ability);
    let ability = *driver.state().stack().last().unwrap();
    let key = AutoYield::of(driver.state(), ability).unwrap();
    assert_eq!(key.name, "Pinger");
    for viewer in [PlayerId(0), PlayerId(1)] {
        let view = StateView::for_player(driver.state(), viewer)
            .objects
            .into_iter()
            .find(|o| o.id == ability)
            .unwrap();
        assert_eq!(
            (view.name, view.rules_text.unwrap_or_default()),
            (key.name.clone(), key.text.clone())
        );
    }
}

#[rstest]
#[case::a_spell_on_the_stack(Top::Spell, true)]
#[case::the_source_of_an_ability(Top::Ability, false)]
fn nothing_but_an_ability_on_the_stack_has_a_key(#[case] top: Top, #[case] on_stack: bool) {
    let driver = duel(top);
    let id = if on_stack {
        *driver.state().stack().last().unwrap()
    } else {
        driver.id_of("Pinger").unwrap()
    };
    assert_eq!(AutoYield::of(driver.state(), id), None);
    assert_eq!(AutoYield::of(driver.state(), ObjectId(9_999)), None);
}
