//! Goal: a mana ability that pays life is offered and accepted only while the player holds that much life (CR 119.4), also
//! for a player who has not lost at 0 or below (Phyrexian Unlife), so no activation applies a payment of life the player cannot make.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use rstest::rstest;

use super::*;
use crate::answers::Outcome;
use crate::decision::PriorityAction;
use crate::fixtures::state;
use crate::fold::apply_event;
use crate::priority::{answer, priority_options};

const ME: PlayerId = PlayerId(0);

/// A Forest whose mana abilities cost `life` more life to activate, like Thran Portal.
fn portal(life: u32) -> Arc<CardDefinition> {
    let json = format!(
        r#"{{"CostModifier":{{"applies_to":["This"],"change":"Increase","mana":"{{0}}","activating":"Mana","life":{life}}}}}"#
    );
    Arc::new(
        CardDefinition::basic_land(Some(Color::Green))
            .with_rules(serde_json::from_str(&json).unwrap()),
    )
}

/// A game whose first player holds `life` and controls a Forest whose mana abilities cost `price` life.
fn game(price: u32, life: i32) -> GameState {
    let mut game = state(vec![vec![], vec![]], vec![vec![portal(price)], vec![]]);
    game.possible.costs = true;
    let held = game.player(ME).unwrap().life();
    let lost = u32::try_from(i64::from(held) - i64::from(life)).unwrap();
    apply_event(
        &mut game,
        &Event::LifeLost {
            player: ME,
            amount: lost,
        },
    )
    .unwrap();
    assert_eq!(game.player(ME).unwrap().life(), life);
    game
}

#[rstest]
#[case::plenty(1, 19, true)]
#[case::exactly_enough(1, 1, true)]
#[case::exactly_enough_for_two(2, 2, true)]
#[case::one_short_for_two(2, 1, false)]
#[case::at_zero(1, 0, false)]
#[case::below_zero(1, -4, false)]
#[case::below_zero_for_two(2, -1, false)]
fn a_life_cost_is_offered_only_to_a_player_who_holds_the_life(
    #[case] price: u32,
    #[case] life: i32,
    #[case] offered: bool,
) {
    let game = game(price, life);
    let options = priority_options(&game, ME);
    assert_eq!(options.mana_abilities.len(), usize::from(offered));
    let source = ManaSource {
        object: game.zone(Zone::Battlefield)[0],
        ability: 0,
    };
    let result = answer(&game, ME, &options, PriorityAction::ActivateMana(source));
    match result {
        Ok(Outcome::Events(events)) => {
            assert!(offered);
            // Every payment the activation makes can be applied.
            let mut after = game.clone();
            for event in &events {
                apply_event(&mut after, event).unwrap();
            }
        }
        Ok(_) => panic!("a mana ability is not a cast"),
        Err(_) => assert!(!offered),
    }
}
