//! Goal: a player may concede at any point of a game with 2 to 4 seats; the loss is clean (objects gone, decision and turn recomputed, game ended when one player is left), invariants hold and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_core::ids::PlayerId;
use mtg_engine::player::{LossReason, PlayerStatus};
use mtg_engine::state::Stage;
use mtg_engine::turn::Step;
use mtg_engine::{Engine, EngineError, Event, GameResult, GameSetup, replay};
use mtg_testkit::{InvariantChecker, RandomBot, fuzz_setup};
use proptest::prelude::*;
use rstest::rstest;

/// Upper bound on answers when playing a game out.
const MAX_ANSWERS: usize = 200_000;

/// A seeded bot game that can be stopped anywhere; invariants are checked as it goes.
struct Game {
    setup: GameSetup,
    engine: Engine,
    bots: Vec<RandomBot>,
    checker: InvariantChecker,
}

/// Which seat concedes, relative to the pending decision.
#[derive(Debug, Clone, Copy)]
enum Who {
    /// The seat that must answer.
    Holder,
    /// Some other seat, preferably one that is neither the holder nor the active player.
    Bystander,
    /// The active player.
    Active,
}

impl Game {
    fn new(players: usize, seed: u64) -> Game {
        let setup = fuzz_setup(players, seed);
        let engine = Engine::new(setup.clone()).expect("fuzz setups are valid");
        let bots = (0u64..)
            .take(players)
            .map(|seat| RandomBot::new(seed, seat))
            .collect();
        let mut checker = InvariantChecker::new(&setup)
            .expect("fresh checker")
            .with_full_check_every(25);
        checker
            .observe(engine.state(), engine.events())
            .expect("invariants at the start");
        Game {
            setup,
            engine,
            bots,
            checker,
        }
    }

    fn check(&mut self) {
        self.checker
            .observe_engine(&self.engine)
            .expect("invariants");
    }

    /// One bot answer; false once the game is over.
    fn step(&mut self) -> bool {
        let Some(decision) = self.engine.decision().cloned() else {
            return false;
        };
        let answer = self.bots[decision.player.index()].choose(&decision);
        self.engine.apply(answer).expect("bot answers are legal");
        self.check();
        true
    }

    /// Plays up to `answers` bot answers, fewer if the game ends first.
    fn play(&mut self, answers: usize) {
        for _ in 0..answers {
            if !self.step() {
                break;
            }
        }
    }

    fn play_out(&mut self) {
        for _ in 0..MAX_ANSWERS {
            if !self.step() {
                return;
            }
        }
        panic!("the game did not finish within {MAX_ANSWERS} answers");
    }

    fn alive(&self) -> Vec<PlayerId> {
        self.engine.state().alive_players().collect()
    }

    fn holder(&self) -> Option<PlayerId> {
        self.engine.decision().map(|d| d.player)
    }

    fn pick(&self, who: Who) -> Option<PlayerId> {
        let holder = self.holder()?;
        let active = self.engine.state().turn().active();
        let alive = self.alive();
        match who {
            Who::Holder => Some(holder),
            Who::Active => alive.contains(&active).then_some(active),
            Who::Bystander => alive
                .iter()
                .copied()
                .find(|p| *p != holder && *p != active)
                .or_else(|| alive.iter().copied().find(|p| *p != holder)),
        }
    }

    fn concede(&mut self, player: PlayerId) -> Vec<Event> {
        let events = self.engine.concede(player).expect("concede succeeds");
        self.checker
            .observe_engine(&self.engine)
            .expect("invariants after concede");
        self.checker
            .full_check(self.engine.state())
            .expect("full invariants after concede");
        events
    }

    /// The live state equals a replay of the log, and a rebuilt engine agrees with the live one.
    fn assert_replays(&self) {
        let replayed = replay(&self.setup, self.engine.events()).expect("log replays");
        assert_eq!(replayed.state_hash(), self.engine.state_hash());
        let resumed =
            Engine::resume(self.setup.clone(), self.engine.events().to_vec()).expect("log resumes");
        assert_eq!(resumed.state_hash(), self.engine.state_hash());
        assert_eq!(resumed.events().len(), self.engine.events().len());
        assert_eq!(resumed.decision(), self.engine.decision());
    }

    /// One `GameEnded`, a result that matches the survivors, and a log that replays.
    fn assert_finished(&self) {
        let ended: Vec<&Event> = self
            .engine
            .events()
            .iter()
            .filter(|e| matches!(e, Event::GameEnded { .. }))
            .collect();
        assert_eq!(ended.len(), 1, "exactly one GameEnded");
        assert!(self.engine.decision().is_none());
        match (self.engine.result(), self.alive().as_slice()) {
            (Some(GameResult::Winner(winner)), [only]) => assert_eq!(winner, *only),
            (Some(GameResult::Draw), []) => {}
            (result, alive) => panic!("result {result:?} does not fit survivors {alive:?}"),
        }
        self.checker
            .check_full_replay(self.engine.state(), self.engine.events())
            .expect("replay equals live state");
    }
}

fn count_events(events: &[Event], wanted: fn(&Event) -> bool) -> usize {
    events.iter().filter(|e| wanted(e)).count()
}

#[rstest]
fn conceding_at_any_point_keeps_the_game_consistent(
    #[values(2, 3, 4)] players: usize,
    #[values(1, 2)] seed: u64,
    #[values(0, 1, 6, 15, 40, 90, 200, 450, 900)] answers: usize,
    #[values(Who::Holder, Who::Bystander, Who::Active)] who: Who,
) {
    let mut game = Game::new(players, seed);
    game.play(answers);
    let Some(conceder) = game.pick(who) else {
        return; // the game ended before this point
    };
    let holder = game.holder().expect("a decision is pending");
    let state = game.engine.state();
    let (active, stage, turn) = (state.turn().active(), state.stage(), state.turn().number());
    let step = state.turn().step();
    let owned = state.objects().filter(|o| o.owner() == conceder).count();
    let alive_before = game.alive().len();

    let events = game.concede(conceder);

    let state = game.engine.state();
    assert_eq!(
        events.first(),
        Some(&Event::PlayerLost {
            player: conceder,
            reason: LossReason::Conceded
        })
    );
    assert!(matches!(
        state
            .player(conceder)
            .map(mtg_engine::player::Player::status),
        Some(PlayerStatus::Lost(LossReason::Conceded))
    ));
    let survivors = game.alive();
    assert_eq!(survivors.len(), alive_before - 1);

    if let [winner] = survivors[..] {
        assert_eq!(game.engine.result(), Some(GameResult::Winner(winner)));
        assert!(game.engine.decision().is_none());
        assert!(matches!(events.last(), Some(Event::GameEnded { .. })));
        assert_eq!(
            count_events(&events, |e| matches!(e, Event::GameEnded { .. })),
            1
        );
    } else {
        assert_eq!(game.engine.result(), None);
        let next = game.engine.decision().expect("the game goes on").player;
        assert!(survivors.contains(&next), "{next:?} has left the game");
        assert!(state.objects().all(|o| o.owner() != conceder));
        assert_eq!(
            count_events(&events, |e| matches!(e, Event::ObjectLeftGame { .. })),
            owned,
            "every object of the conceder leaves"
        );
        if conceder != holder && conceder != active {
            assert_eq!(next, holder, "an uninvolved concession keeps the decision");
        }
        if conceder == active && stage == Stage::Turns {
            let began: Vec<(u32, PlayerId)> = events
                .iter()
                .filter_map(|e| match e {
                    Event::TurnBegan { number, active, .. } => Some((*number, *active)),
                    _ => None,
                })
                .collect();
            if let Some(&(number, next)) = began.first() {
                // Only cleanup, which asks nobody else, ends the turn within the concession.
                assert_eq!(step, Step::Cleanup);
                assert_eq!((began.len(), number), (1, turn + 1));
                assert_eq!(Some(next), state.next_alive_after(conceder));
            } else {
                assert_eq!(state.turn().number(), turn, "the turn goes on");
                assert_eq!(state.turn().active(), conceder);
                assert_eq!(state.active_player(), None, "without an active player");
            }
        }
    }
    game.assert_replays();

    game.play_out();
    game.assert_finished();
    assert_ne!(
        game.engine.result(),
        Some(GameResult::Winner(conceder)),
        "the conceder cannot win"
    );
}

#[rstest]
fn the_last_player_standing_wins(
    #[values(2, 3, 4)] players: usize,
    #[values(0, 1, 2, 3)] survivor: usize,
    #[values(0, 25)] answers_between: usize,
) {
    if survivor >= players {
        return;
    }
    let mut game = Game::new(players, 7);
    for seat in (0..players).filter(|s| *s != survivor) {
        game.play(answers_between);
        game.concede(pid(seat));
    }
    assert_eq!(
        game.engine.result(),
        Some(GameResult::Winner(pid(survivor)))
    );
    let losses = game
        .engine
        .events()
        .iter()
        .filter(|e| {
            matches!(
                e,
                Event::PlayerLost {
                    reason: LossReason::Conceded,
                    ..
                }
            )
        })
        .count();
    assert_eq!(losses, players - 1);
    game.assert_finished();
    game.assert_replays();
}

#[rstest]
#[case::unknown_seat(pid(9), EngineError::NotInGame(pid(9)))]
#[case::already_out(pid(1), EngineError::NotInGame(pid(1)))]
fn conceding_when_not_in_the_game_is_rejected_and_changes_nothing(
    #[case] player: PlayerId,
    #[case] expected: EngineError,
) {
    let mut game = Game::new(3, 4);
    game.play(30);
    game.concede(pid(1));
    let (hash, events) = (game.engine.state_hash(), game.engine.events().len());
    assert_eq!(game.engine.concede(player), Err(expected));
    assert_eq!(game.engine.state_hash(), hash);
    assert_eq!(game.engine.events().len(), events);
}

#[test]
fn nobody_can_concede_after_the_game_is_over() {
    let mut game = Game::new(2, 5);
    game.play_out();
    let events = game.engine.events().len();
    for seat in 0..2 {
        assert_eq!(game.engine.concede(pid(seat)), Err(EngineError::NoDecision));
    }
    assert_eq!(game.engine.events().len(), events);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn random_concessions_never_break_a_game(
        seed in any::<u64>(),
        players in 2usize..=4,
        plan in prop::collection::vec((0usize..150, 0usize..4), 1..4),
    ) {
        let mut game = Game::new(players, seed);
        for (answers, seat) in plan {
            game.play(answers);
            let seat = pid(seat % players);
            if game.engine.decision().is_some() && game.engine.state().is_alive(seat) {
                game.concede(seat);
            }
        }
        game.play_out();
        game.assert_finished();
        game.assert_replays();
    }
}
