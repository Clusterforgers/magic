//! Goal: a draw replaced by a "may" with no steps ("you may skip that draw instead") asks its controller each time:
//! yes skips the draw, no keeps it, and another player's draw is not asked about.
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use rstest::rstest;

use super::{Processed, process};
use crate::choose::Choice;
use crate::events::Event;
use crate::fixtures::state;
use crate::state::GameState;
use crate::zones::Zone;

const SKIP: &str = r#"{"Replacement":{"when":{"Draw":{"who":"You","except_draw_step":0}},"by":{"Instead":[{"May":{"effects":[]}}]}}}"#;

fn game() -> GameState {
    let rule: Ability = serde_json::from_str(SKIP).unwrap();
    let familiar = CardDefinition::new("Familiar")
        .with_type(CardType::Creature)
        .with_rules(rule);
    state(
        vec![vec![], vec![]],
        vec![vec![std::sync::Arc::new(familiar)], vec![]],
    )
}

fn draw(game: &GameState, player: PlayerId) -> Event {
    let object = game.zone(Zone::Library(player))[0];
    Event::CardDrawn { player, object }
}

#[rstest]
#[case::skipped(Some(true), 0)]
#[case::kept(Some(false), 1)]
#[case::asked(None, 1)]
fn the_controller_may_skip_each_draw(#[case] said: Option<bool>, #[case] drawn: usize) {
    let game = game();
    let event = draw(&game, PlayerId(0));
    let journal: Vec<_> = said
        .iter()
        .map(|yes| (PlayerId(0), Choice::YesNo(*yes)))
        .collect();
    match (process(&game, vec![event.clone()], &journal).unwrap(), said) {
        (Processed::Ask { decision, .. }, None) => assert_eq!(decision.player, PlayerId(0)),
        (Processed::Events(events), Some(_)) => {
            let draws = events.iter().filter(|e| **e == event).count();
            assert_eq!(draws, drawn, "{events:?}");
        }
        (other, _) => panic!("{other:?}"),
    }
}

#[test]
fn another_players_draw_is_not_asked_about() {
    let game = game();
    let event = draw(&game, PlayerId(1));
    match process(&game, vec![event.clone()], &[]).unwrap() {
        Processed::Events(events) => assert_eq!(events, vec![event]),
        other @ Processed::Ask { .. } => panic!("{other:?}"),
    }
}
