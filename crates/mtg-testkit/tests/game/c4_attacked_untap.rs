//! Goal: "untap all creatures that attacked this turn" untaps exactly the creatures declared as attackers this turn, at tables of two, three and four seats: a creature that stayed home keeps its state, a creature of another player that did not attack stays tapped, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_core::card::CardType;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

/// The turn the scenarios start in.
const START: u32 = 4;

/// A free instant: "Untap all creatures that attacked this turn."
fn rally() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Rally")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![Effect::Untap {
                    what: Ref::Each(Selector(vec![
                        Pred::Type(CardType::Creature),
                        Pred::AttackedThisTurn,
                    ])),
                }],
            }),
    )
}

/// Whether the permanent called `name` is tapped.
fn is_tapped(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped()
}

#[rstest]
#[case::one_attacker_in_two(2, &["Alpha"])]
#[case::two_attackers_in_three(3, &["Alpha", "Beta"])]
#[case::the_other_one_in_four(4, &["Beta"])]
fn only_the_creatures_that_attacked_this_turn_untap(
    #[case] seats: usize,
    #[case] attackers: &[&str],
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(START, 0, Step::Main1)
        .hand(0, [rally()]);
    scenario = scenario.battlefield(
        0,
        [
            ready(free_creature("Alpha", 2, 2)),
            ready(free_creature("Beta", 2, 2)),
            tapped(free_creature("Idle", 2, 2)),
        ],
    );
    scenario = scenario.battlefield(1, [tapped(free_creature("Guard", 1, 1))]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    let declared: Vec<(&str, usize)> = attackers.iter().map(|name| (*name, 1)).collect();
    driver.attack(&declared)?;
    for name in attackers {
        assert!(is_tapped(&driver, name), "{name} attacked and is tapped");
    }
    driver.cast("Rally")?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    for name in ["Alpha", "Beta"] {
        assert!(!is_tapped(&driver, name), "{name} is untapped");
    }
    assert!(is_tapped(&driver, "Idle"), "a creature that stayed home");
    assert!(is_tapped(&driver, "Guard"), "a creature of another player");
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
