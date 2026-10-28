//! Goal: a card milled triggers `Milled` for each card that matches, by the player whose library it left; a surveil or another move from a library to a graveyard never does; a batch of mills over several libraries triggers once and counts the cards.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::refs::{Ref, ZoneKind};
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use super::detect_events::{FOE, ME, fires, pattern};
use crate::events::{Event, ZoneCause};
use crate::fixtures::{creature, setup};
use crate::state::GameState;
use crate::zones::Zone;

const NONLAND: &str = r#"{"Milled":{"what":[{"Not":{"Type":"Land"}}]}}"#;
const OPPONENT_NONLAND: &str = r#"{"Milled":{"what":[{"Not":{"Type":"Land"}}],"by":"Opponents"}}"#;
const BATCH: &str = r#"{"Batch":{"Milled":{"what":[{"Not":{"Type":"Land"}}]}}}"#;

/// Seat 0 controls a Watcher with "when `when`, gain 1 life"; each library holds, from the top, the cards of `top`
/// ('L' a land, 'C' a creature).
fn w8b_board(when: &str, top: &str) -> GameState {
    let watcher = Arc::new(
        CardDefinition::new("Watcher")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(Ability::Triggered {
                when: pattern(when),
                intervening_if: None,
                limit: None,
                from: ZoneKind::Battlefield,
                targets: Vec::new(),
                effects: vec![Effect::GainLife {
                    who: Ref::You,
                    amount: Amount::Fixed(1),
                }],
            }),
    );
    let mut setup = setup(
        vec![Vec::new(); 3],
        vec![vec![watcher], Vec::new(), Vec::new()],
    );
    for player in &mut setup.players {
        player.library = top
            .chars()
            .enumerate()
            .map(|(n, c)| match c {
                'L' => {
                    Arc::new(CardDefinition::new(&format!("Land {n}")).with_type(CardType::Land))
                }
                _ => creature(&format!("Bear {n}"), 2, 2),
            })
            .collect();
    }
    GameState::from_setup(&setup).unwrap()
}

/// The top `count` cards of `player`'s library put into their graveyard with `cause`.
fn w8b_moves(state: &GameState, player: PlayerId, count: usize, cause: ZoneCause) -> Vec<Event> {
    let top: Vec<ObjectId> = state
        .zone(Zone::Library(player))
        .iter()
        .rev()
        .take(count)
        .copied()
        .collect();
    top.into_iter()
        .map(|object| Event::ZoneChanged {
            object,
            from: Zone::Library(player),
            to: Zone::Graveyard(player),
            cause,
        })
        .collect()
}

#[rstest]
#[case::each_nonland_card(NONLAND, ME, ZoneCause::Milled, 3, 2)]
#[case::an_opponents_mill(NONLAND, FOE, ZoneCause::Milled, 3, 2)]
#[case::only_lands(NONLAND, ME, ZoneCause::Milled, 1, 0)]
#[case::a_surveil_is_no_mill(NONLAND, ME, ZoneCause::Moved, 3, 0)]
#[case::a_discard_is_no_mill(NONLAND, ME, ZoneCause::Discarded, 3, 0)]
#[case::opponents_only_not_mine(OPPONENT_NONLAND, ME, ZoneCause::Milled, 3, 0)]
#[case::opponents_only_theirs(OPPONENT_NONLAND, FOE, ZoneCause::Milled, 3, 2)]
#[case::a_batch_once(BATCH, FOE, ZoneCause::Milled, 3, 1)]
#[case::a_batch_of_lands(BATCH, ME, ZoneCause::Milled, 1, 0)]
#[case::a_batch_of_a_surveil(BATCH, ME, ZoneCause::Moved, 3, 0)]
fn w8b_a_mill_triggers_for_the_cards_that_match(
    #[case] when: &str,
    #[case] who: PlayerId,
    #[case] cause: ZoneCause,
    #[case] count: usize,
    #[case] expected: usize,
) {
    let pre = w8b_board(when, "LCCL");
    let events = w8b_moves(&pre, who, count, cause);
    assert_eq!(fires(&pre, &events).len(), expected);
}

#[test]
fn w8b_a_batch_over_several_libraries_triggers_once_and_counts_the_nonland_cards() {
    let pre = w8b_board(BATCH, "CLCC");
    let mut events = w8b_moves(&pre, ME, 2, ZoneCause::Milled);
    events.extend(w8b_moves(&pre, FOE, 4, ZoneCause::Milled));
    events.extend(w8b_moves(&pre, PlayerId(2), 3, ZoneCause::Milled));
    let found = fires(&pre, &events);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].cause.amount, 1 + 3 + 2);
}
