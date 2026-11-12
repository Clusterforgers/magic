//! Goal: seeded mirror games between bot brains on the duel decks: Normal and Hard beat Easy clearly at tables of 2 to
//! 4 seats, Hard holds its own against Normal, a team never aims harm at itself in 2v2 or Two-Headed Giant, and every
//! game ends without a stall or a broken invariant.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::{Decision, Engine, GameResult};
use mtg_testkit::bot::apply_answer;
use mtg_testkit::brain::{Level, SeatBrain, brain};
use mtg_testkit::brain_decks::{Duel, duel_setup};
use mtg_testkit::harness::{PlayOptions, play_brains};
use rstest::rstest;

/// The decks of a table of `seats` in game `seed`: one deck for every seat (a mirror match, so skill decides and not
/// the decks), Gruul for two seeds, then Orzhov for two, so a seat that alternates with the seed plays both.
fn decks(seats: usize, seed: u64) -> Vec<Duel> {
    let duel = if (seed / 2).is_multiple_of(2) {
        Duel::Gruul
    } else {
        Duel::Orzhov
    };
    vec![duel; seats]
}

/// Plays game `seed` with `levels` by seat and answers the seat that won, if one did.
fn winner(levels: &[Level], seed: u64) -> Option<PlayerId> {
    let setup = duel_setup(&decks(levels.len(), seed), seed, HouseRules::default()).unwrap();
    let mut brains: Vec<Box<dyn SeatBrain>> = levels
        .iter()
        .enumerate()
        .map(|(seat, level)| brain(*level, seed, PlayerId::from_index(seat), false))
        .collect();
    let options = PlayOptions {
        max_answers: 20_000,
        ..PlayOptions::default()
    };
    let outcome = play_brains(setup, seed, options, &mut brains).unwrap();
    match outcome.result {
        GameResult::Winner(player) => Some(player),
        _ => None,
    }
}

/// The share of `games` seeded games that a `strong` seat wins against `weak` ones at a table of `seats`, the strong
/// seat moving round the table from game to game.
fn win_rate(strong: Level, weak: Level, seats: usize, games: u64) -> f64 {
    let mut wins = 0;
    for seed in 0..games {
        let at = usize::try_from(seed).unwrap() % seats;
        let mut levels = vec![weak; seats];
        levels[at] = strong;
        if winner(&levels, 1_000 + seed) == Some(PlayerId::from_index(at)) {
            wins += 1;
        }
    }
    let rate = f64::from(wins) / f64::from(u32::try_from(games).unwrap());
    eprintln!("{strong:?} against {weak:?} at {seats} seats: {wins}/{games} ({rate:.2})");
    rate
}

#[rstest]
#[case::head_to_head(Level::Normal, Level::Easy, 2, 20, 0.8)]
#[case::three_seats(Level::Normal, Level::Easy, 3, 12, 0.6)]
#[case::four_seats(Level::Normal, Level::Easy, 4, 12, 0.6)]
#[case::hard_head_to_head(Level::Hard, Level::Easy, 2, 10, 0.8)]
#[case::hard_against_normal(Level::Hard, Level::Normal, 2, 40, 0.45)]
fn the_stronger_level_wins_its_share(
    #[case] strong: Level,
    #[case] weak: Level,
    #[case] seats: usize,
    #[case] games: u64,
    #[case] share: f64,
) {
    assert!(win_rate(strong, weak, seats, games) >= share);
}

/// The duel decks' cards that only ever hurt what they target.
const HARMFUL: [&str; 5] = [
    "Lightning Bolt",
    "Doom Blade",
    "Swords to Plowshares",
    "Pacifism",
    "Fireball",
];

/// The harm a heuristic seat aimed at its own team in one answer: a harmful card's target that is a teammate or a
/// teammate's permanent (the seat itself included), or an attack at a teammate.
fn harms(engine: &Engine, decision: &Decision, answer: &Answer) -> usize {
    let state = engine.state();
    let ours = |player: PlayerId| state.are_teammates(player, decision.player);
    let owner = |target: &Target| match target {
        Target::Player(player) => Some(*player),
        Target::Object(object) => state
            .object(object.id)
            .map(mtg_engine::object::GameObject::controller),
    };
    match (&decision.kind, answer) {
        (
            DecisionKind::ChooseTargets(options),
            Answer::ChooseTargets(TargetChoice::Targets(targets)),
        ) => {
            let harmful = state
                .object(options.source)
                .is_some_and(|source| HARMFUL.contains(&source.card().name.as_str()));
            if harmful {
                targets
                    .iter()
                    .filter(|t| owner(t).is_some_and(ours))
                    .count()
            } else {
                0
            }
        }
        (DecisionKind::DeclareAttackers(_), Answer::DeclareAttackers(attack)) => attack
            .iter()
            .filter(|(_, target)| matches!(target, AttackTarget::Player(p) if ours(*p)))
            .count(),
        _ => 0,
    }
}

/// Plays a team game of four seats in `mode` seeded with `seed`, team 0 at `strong` and team 1 at `weak`; answers
/// whether team 0 won and the harm its seats aimed at their own team.
fn team_game(mode: TeamMode, seed: u64, strong: Level, weak: Level) -> (bool, usize) {
    let teams = Teams::even(mode, 2, 2);
    let house = HouseRules {
        teams: Some(teams.clone()),
        ..HouseRules::default()
    };
    let setup = duel_setup(&decks(4, seed), seed, house).unwrap();
    let mut brains: Vec<Box<dyn SeatBrain>> = (0..4)
        .map(|seat| {
            let player = PlayerId::from_index(seat);
            let level = if teams.team_of(player) == 0 {
                strong
            } else {
                weak
            };
            brain(level, seed, player, false)
        })
        .collect();
    let mut engine = Engine::new(setup).unwrap();
    let mut harm = 0;
    for _ in 0..30_000 {
        let Some(decision) = engine.decision().cloned() else {
            break;
        };
        let seat = decision.player;
        let answer = brains[seat.index()].choose(&decision, Some(&engine));
        if teams.team_of(seat) == 0 {
            harm += harms(&engine, &decision, &answer);
        }
        apply_answer(&mut engine, &decision, answer).unwrap();
    }
    let won = matches!(engine.result(), Some(GameResult::Team(0)));
    (won, harm)
}

#[rstest]
#[case::two_against_two(TeamMode::Teams)]
#[case::two_headed_giant(TeamMode::TwoHeadedGiant)]
fn a_team_never_harms_itself_and_normal_teams_win(#[case] mode: TeamMode) {
    let games = 10;
    let mut wins = 0;
    for seed in 0..games {
        let (won, harm) = team_game(mode, 2_000 + seed, Level::Normal, Level::Easy);
        assert_eq!(harm, 0, "game {seed} aimed harm at a teammate");
        wins += u32::from(won);
    }
    eprintln!("Normal team against Easy team in {mode:?}: {wins}/{games}");
    assert!(wins >= 8);
}
