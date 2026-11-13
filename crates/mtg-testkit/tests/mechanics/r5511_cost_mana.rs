//! Goal: Elemental Resonance adds, at the beginning of its controller's first main phase, mana equal to the mana cost of
//! the permanent it enchants (colorless for generic mana, one color of a hybrid symbol picked by its controller), and
//! nothing for a permanent without a mana cost, at 2 to 4 seats; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaType::{
    self, Black as B, Blue as U, Colorless as C, Green as G, Red as R, White as W,
};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

fn thing(cost: &str) -> Arc<CardDefinition> {
    let definition = CardDefinition::new("Thing").with_type(CardType::Artifact);
    Arc::new(if cost.is_empty() {
        definition
    } else {
        definition.with_mana_cost(mtg_core::mana::ManaCost::parse(cost).unwrap())
    })
}

fn pool(driver: &Driver) -> Vec<u32> {
    let mana = driver.state().player(PlayerId(0)).unwrap().mana().clone();
    ManaType::ALL.iter().map(|kind| mana.get(*kind)).collect()
}

fn counted(kinds: &[ManaType]) -> Vec<u32> {
    ManaType::ALL
        .iter()
        .map(|kind| u32::try_from(kinds.iter().filter(|k| *k == kind).count()).unwrap())
        .collect()
}

#[rstest]
#[case::generic_and_colors("{2}{W}{U}", &[C, C, W, U])]
#[case::one_color("{R}", &[R])]
#[case::first_of_a_hybrid("{W/U}{B}", &[W, B])]
#[case::no_cost("", &[])]
#[case::green_twice("{1}{G}{G}", &[C, G, G])]
fn r5511_resonance_adds_the_enchanted_permanents_cost(
    #[case] cost: &str,
    #[case] added: &[ManaType],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(Scenario::new(seats).at(1, 0, Step::Upkeep).battlefield(
            0,
            [
                ready(thing(cost)),
                attached_to(ready(card("Elemental Resonance").unwrap()), 0, 0),
            ],
        ))?;
    driver.idle_until(1, Step::Main1)?;
    for _ in 0..10 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(pool(&driver), counted(added));
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
