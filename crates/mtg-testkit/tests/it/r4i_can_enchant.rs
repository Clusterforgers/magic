//! Goal: `Pred::CanEnchantSource` holds for an Aura card in a hand exactly when its enchant ability lets it enchant the source ("an Aura card that could enchant this creature", CR 303.4), whoever controls the source.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::refs::Selector;
use mtg_engine::resolve::Scope;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::attach_cards::{hex_aura, mighty_aura, rune_aura};
use mtg_testkit::cards::bear;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

#[rstest]
#[case::enchant_creature("Mighty Aura", true)]
#[case::enchant_land("Rune Aura", false)]
#[case::enchant_opponents_creature("Hex Aura", false)]
fn an_aura_could_enchant_the_source_when_its_restriction_allows_it(
    #[case] aura: &str,
    #[case] could: bool,
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, vec![ready(bear())])
            .hand(
                0,
                vec![
                    mighty_aura().unwrap(),
                    rune_aura().unwrap(),
                    hex_aura().unwrap(),
                ],
            )
            .at(1, 0, Step::Main1),
    )?;
    let source = driver
        .state()
        .object(driver.id_of("Grizzly Bears")?)
        .unwrap()
        .reference();
    let card = driver
        .state()
        .zone(Zone::Hand(pid(0)))
        .iter()
        .copied()
        .find(|id| crate::common::name_of(driver.state(), *id) == aura)
        .and_then(|id| driver.state().object(id))
        .unwrap()
        .reference();
    let selector: Selector =
        serde_json::from_str(r#"[{"Subtype":"Aura"},"CanEnchantSource"]"#).unwrap();
    let scope = Scope::announcing(driver.state(), pid(0), source);
    assert_eq!(scope.selects(&selector, card).unwrap(), could);
    Ok(())
}
