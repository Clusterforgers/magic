//! Goal: a base power and toughness read after the effects that set them (7a and 7b) and before the ones that modify them (7c), not as the card prints them: a printed 1/1 that an effect makes 3/3 has base power 3, a pump after it changes the power and not the base.
use mtg_cards::value::Amount;
use mtg_core::ids::PlayerId;
use rstest::rstest;

use super::tests::{effect, named, pump};
use crate::effects::Modification;
use crate::fixtures::{creature, setup};
use crate::resolve::Scope;
use crate::state::GameState;

fn cub() -> GameState {
    let game = setup(
        vec![vec![], vec![]],
        vec![vec![creature("Cub", 1, 1)], vec![]],
    );
    GameState::from_setup(&game).unwrap()
}

fn read(game: &GameState, stat: &str) -> i64 {
    let amount: Amount =
        serde_json::from_str(&format!(r#"{{"Stat":{{"of":"This","stat":"{stat}"}}}}"#)).unwrap();
    Scope::announcing(game, PlayerId(0), named(game, "Cub"))
        .amount(&amount)
        .unwrap()
}

#[rstest]
#[case::as_printed(&[], (1, 1), (1, 1))]
#[case::set_to_three(&[Modification::SetPowerToughness { power: 3, toughness: 3 }], (3, 3), (3, 3))]
#[case::a_pump_alone_is_no_base(&[Modification::ModifyPowerToughness { power: 2, toughness: 2 }], (3, 3), (1, 1))]
#[case::set_then_pumped(
    &[Modification::SetPowerToughness { power: 3, toughness: 3 }, Modification::ModifyPowerToughness { power: 2, toughness: 2 }],
    (5, 5),
    (3, 3)
)]
fn the_base_is_what_the_setting_effects_leave(
    #[case] effects: &[Modification],
    #[case] now: (i32, i32),
    #[case] base: (i64, i64),
) {
    let mut game = cub();
    for (stamp, modification) in effects.iter().enumerate() {
        effect(&mut game, "Cub", modification.clone(), 10 + stamp as u64);
    }
    assert_eq!(pump(&game, "Cub"), Some(now));
    assert_eq!(
        (read(&game, "BasePower"), read(&game, "BaseToughness")),
        base
    );
}
