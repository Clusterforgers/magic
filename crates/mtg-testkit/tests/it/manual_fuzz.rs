//! Goal: whole bot games with decks of the cards the engine does not run whole and manual actions taken at random times finish and keep every invariant (zones, life, counters, the replay of the log equal to the live state) at tables of two, three and four seats; the games use every kind of action, the bots never take one themselves, an engine rebuilt from the log at each decision is in the same state and asks the same question, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use std::collections::BTreeSet;

use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::manual::ManualAction;
use mtg_engine::{Engine, EngineError, Event};
use mtg_testkit::{ManualScript, PlayOptions, RandomBot, manual_fuzz_setup, play_game_with};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(30)
}

/// One game in how many priorities takes a scripted action.
const EVERY: usize = 10;

fn options() -> PlayOptions {
    PlayOptions {
        manual_every: EVERY,
        ..PlayOptions::default()
    }
}

/// The name of the kind of action.
pub(crate) fn kind(action: &ManualAction) -> &'static str {
    match action {
        ManualAction::Move { .. } => "move",
        ManualAction::MoveTop { .. } => "move top",
        ManualAction::Counters { .. } => "counters",
        ManualAction::PlayerCounters { .. } => "player counters",
        ManualAction::Life { .. } => "life",
        ManualAction::CreateToken { .. } => "token",
        ManualAction::Tap { .. } => "tap",
        ManualAction::Untap { .. } => "untap",
        ManualAction::Draw { .. } => "draw",
        ManualAction::Discard { .. } => "discard",
        ManualAction::Attach { .. } => "attach",
        ManualAction::Unattach { .. } => "unattach",
        ManualAction::Damage { .. } => "damage",
        ManualAction::AddMana { .. } => "mana",
        ManualAction::Pump { .. } => "pump",
    }
}

#[rstest]
fn seeded_games_with_manual_cards_and_manual_actions_finish_and_keep_every_invariant(
    #[values(2, 3, 4)] players: usize,
) {
    let mut kinds = BTreeSet::new();
    let mut taken = 0;
    for seed in 0..games() {
        let setup = manual_fuzz_setup(players, seed).unwrap();
        let outcome =
            play_game_with(setup, seed, options()).unwrap_or_else(|failure| panic!("{failure}"));
        let logged: Vec<&ManualAction> = outcome
            .events
            .iter()
            .filter_map(|event| match event {
                Event::ManualAction { action, .. } => Some(&**action),
                _ => None,
            })
            .collect();
        assert_eq!(logged.len(), outcome.manual_actions, "seed {seed}");
        taken += logged.len();
        kinds.extend(logged.into_iter().map(kind));
    }
    assert!(taken > 0, "no manual action in {} games", games());
    for wanted in [
        "move", "counters", "life", "token", "tap", "untap", "draw", "discard",
    ] {
        assert!(
            kinds.contains(wanted),
            "no {wanted} in {} games of {players} seats: {kinds:?}",
            games()
        );
    }
}

#[rstest]
fn the_bots_never_take_a_manual_action(#[values(2, 4)] players: usize) {
    for seed in 0..games() {
        let setup = manual_fuzz_setup(players, seed).unwrap();
        let outcome = play_game_with(setup, seed, PlayOptions::default())
            .unwrap_or_else(|failure| panic!("{failure}"));
        assert_eq!(outcome.manual_actions, 0);
        assert!(
            !outcome
                .events
                .iter()
                .any(|event| matches!(event, Event::ManualAction { .. })),
            "seed {seed}"
        );
    }
}

#[rstest]
fn a_rebuilt_engine_is_in_the_state_of_the_original_and_asks_what_it_asks(
    #[values(2, 3, 4)] players: usize,
) -> Result<(), EngineError> {
    let mut actions = 0;
    for seed in 0..8 {
        let setup = manual_fuzz_setup(players, seed).unwrap();
        let mut engine = Engine::new(setup.clone())?;
        let mut bots: Vec<RandomBot> = (0..players as u64)
            .map(|seat| RandomBot::new(seed, seat))
            .collect();
        let mut script = ManualScript::new(seed, EVERY);
        for step in 0..1500 {
            let Some(decision) = engine.decision().cloned() else {
                break;
            };
            let resumed = Engine::resume(setup.clone(), engine.events().to_vec())?;
            assert_eq!(
                resumed.state_hash(),
                engine.state_hash(),
                "seed {seed} step {step}"
            );
            // A cast or activation that is not paid for yet lives outside the state: its payment, targets, modes
            // and the picks of its cost (the only choice of cards that may still be backed out of).
            let outside = engine.replacing()
                || matches!(
                    &decision.kind,
                    DecisionKind::PayMana(_)
                        | DecisionKind::ChooseTargets(_)
                        | DecisionKind::ChooseMode(_)
                )
                || matches!(&decision.kind, DecisionKind::ChooseCards(options) if options.can_cancel)
                // The way a card is cast in (mutate beside its mana cost) is asked before anything is paid.
                || matches!(&decision.kind, DecisionKind::ChooseOption(options) if options.text.starts_with("Choose how to cast"));
            if !outside {
                assert_eq!(
                    resumed.decision(),
                    engine.decision(),
                    "seed {seed} step {step}"
                );
            }
            let scripted = (matches!(decision.kind, DecisionKind::Priority(_)) && script.due())
                .then(|| script.action(engine.state(), decision.player))
                .flatten();
            let answer = match scripted {
                Some(action) => {
                    actions += 1;
                    Answer::Priority(PriorityAction::Manual(action))
                }
                None => bots[decision.player.index()].choose(&decision),
            };
            engine.apply(answer)?;
        }
    }
    assert!(actions > 0, "no manual action was taken");
    Ok(())
}

#[test]
fn a_seeded_game_with_manual_actions_is_the_same_game_every_time() {
    for seed in [3, 17, 91] {
        let first = play_game_with(manual_fuzz_setup(3, seed).unwrap(), seed, options()).unwrap();
        let second = play_game_with(manual_fuzz_setup(3, seed).unwrap(), seed, options()).unwrap();
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}
