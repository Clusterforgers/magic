//! Goal: a rule in force on a player shows in that player's `effects` for every viewer, a spectator too, and on no other player; a fresh game shows none, and the field is left out of the wire while there are none.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_core::ids::PlayerId;
use mtg_engine::Engine;
use mtg_engine::effects::Duration;
use mtg_engine::rules::{PlayerEffect, Rule};
use mtg_protocol::StateView;
use rstest::rstest;

use crate::common::unique_setup;

const SEATS: usize = 3;
const FOE: PlayerId = PlayerId(1);

/// The effects on each seat as `viewer` sees them.
fn effects(engine: &Engine, viewer: usize) -> Vec<Vec<PlayerEffect>> {
    StateView::for_player(engine.state(), PlayerId::from_index(viewer))
        .table
        .players
        .into_iter()
        .map(|player| player.effects)
        .collect()
}

#[test]
fn a_fresh_game_shows_no_effects_and_leaves_the_field_out() {
    let engine = Engine::new(unique_setup(SEATS, 3)).unwrap();
    for viewer in 0..=SEATS {
        assert!(effects(&engine, viewer).iter().all(Vec::is_empty));
    }
    let view = StateView::for_player(engine.state(), PlayerId(0));
    let wire = serde_json::to_value(&view).unwrap();
    assert!(wire["players"][0].get("effects").is_none());
}

#[rstest]
#[case::cant_lose(Rule::CantLoseGame { who: vec![FOE], winless: Vec::new(), life_only: false }, PlayerEffect::CantLose)]
#[case::lives_at_zero(Rule::CantLoseGame { who: vec![FOE], winless: Vec::new(), life_only: true }, PlayerEffect::LifeSpared)]
#[case::cant_win(Rule::CantLoseGame { who: Vec::new(), winless: vec![FOE], life_only: false }, PlayerEffect::CantWin)]
#[case::no_life_gain(Rule::CantGainLife(vec![FOE]), PlayerEffect::CantGainLife)]
#[case::life_fixed(Rule::LifeTotalCantChange(vec![FOE]), PlayerEffect::LifeCantChange)]
#[case::no_draws(Rule::CantDraw(vec![FOE]), PlayerEffect::CantDraw)]
#[case::skips_draw(Rule::SkipDrawStep(vec![FOE]), PlayerEffect::SkipsDrawStep)]
#[case::no_search(Rule::CantSearch(vec![FOE]), PlayerEffect::CantSearch)]
#[case::no_extra_turns(Rule::SkipExtraTurns(vec![FOE]), PlayerEffect::SkipsExtraTurns)]
#[case::no_hand_limit(Rule::NoMaxHandSize(vec![FOE]), PlayerEffect::NoMaxHandSize)]
#[case::hand_max(Rule::MaxHandSize(vec![(FOE, 4)]), PlayerEffect::MaxHandSize(4))]
#[case::extra_land(Rule::AdditionalLandPlays(vec![(FOE, 1)]), PlayerEffect::ExtraLandPlays(1))]
#[case::hand_revealed(Rule::RevealHand(vec![FOE]), PlayerEffect::HandRevealed)]
#[case::top_revealed(Rule::RevealTop(vec![FOE]), PlayerEffect::TopRevealed)]
#[case::looks_at_top(Rule::LookAtTop(vec![FOE]), PlayerEffect::LooksAtTop)]
#[case::hexproof(
    Rule::PlayersHave { who: vec![FOE], keyword: Keyword::Hexproof },
    PlayerEffect::Keyword(Keyword::Hexproof.shown_name())
)]
fn a_rule_shows_on_the_player_it_names_for_every_viewer(
    #[case] rule: Rule,
    #[case] effect: PlayerEffect,
) {
    let mut engine = Engine::new(unique_setup(SEATS, 5)).unwrap();
    engine.add_rule(rule, Duration::Forever).unwrap();
    for viewer in 0..=SEATS {
        let seen = effects(&engine, viewer);
        for (seat, on) in seen.iter().enumerate() {
            let want = if seat == FOE.index() {
                vec![effect.clone()]
            } else {
                Vec::new()
            };
            assert_eq!(*on, want, "viewer {viewer}, seat {seat}");
        }
    }
}
