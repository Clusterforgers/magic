//! The Delves leaderboard (`docs/delves.md`, Leaderboard): each player's deepest run per Heat, folded from the stored
//! runs; a run's fold is kept until the run changes, so a read folds only the runs that moved.

use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Mutex, PoisonError};

use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use mtg_campaign::Run;
use mtg_protocol::rest::{DelveBoard, DelveBoardEntry, DelveLeaderboard};
use uuid::Uuid;

use super::{campaign, service, unavailable};
use crate::api::ApiError;
use crate::server::AppState;

/// The most entries a board lists.
pub(crate) const BOARD_LIMIT: usize = 20;

/// One player's part in one run, as the boards rank it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Mark {
    /// The player's account.
    pub(crate) user: String,
    /// The Heat the run was started at.
    pub(crate) heat: u8,
    /// What the board shows.
    pub(crate) entry: DelveBoardEntry,
}

/// Each player's mark in `run`, started at `heat` and last changed at `stamp`: none before a boss fell, none for a
/// bot ally.
pub(crate) fn marks(run: &Run, heat: u8, stamp: &str) -> Vec<Mark> {
    let (floors, bosses) = run.depth();
    if (floors, bosses) == (0, 0) {
        return Vec::new();
    }
    let party = u8::try_from(run.heroes.len()).unwrap_or(u8::MAX);
    let date: String = stamp.chars().take(10).collect();
    run.heroes
        .iter()
        .filter_map(|hero| {
            let user = hero.user.clone()?;
            let commanders = hero.deck.as_ref().map_or_else(Vec::new, |deck| {
                if deck.commanders.is_empty() {
                    vec![deck.name.clone()]
                } else {
                    deck.commanders.iter().map(|c| c.name.clone()).collect()
                }
            });
            Some(Mark {
                user,
                heat,
                entry: DelveBoardEntry {
                    player: hero.name.clone(),
                    floors,
                    bosses,
                    commanders,
                    party,
                    date: date.clone(),
                },
            })
        })
        .collect()
}

/// The order of a board: deeper first (floors cleared, then bosses beaten below them), then whoever got there first.
fn rank_key(entry: &DelveBoardEntry) -> (Reverse<u8>, Reverse<u8>, String, String) {
    (
        Reverse(entry.floors),
        Reverse(entry.bosses),
        entry.date.clone(),
        entry.player.clone(),
    )
}

/// One board per Heat from the greatest to the lowest, each with every player's best mark ranked, at most
/// [`BOARD_LIMIT`].
pub(crate) fn boards(marks: impl IntoIterator<Item = Mark>) -> DelveLeaderboard {
    let mut best: BTreeMap<u8, HashMap<String, DelveBoardEntry>> = (0..=super::MAX_HEAT)
        .map(|heat| (heat, HashMap::new()))
        .collect();
    for mark in marks {
        let board = best.entry(mark.heat).or_default();
        match board.get(&mark.user) {
            Some(held) if rank_key(held) <= rank_key(&mark.entry) => {}
            _ => {
                board.insert(mark.user, mark.entry);
            }
        }
    }
    DelveLeaderboard {
        boards: best
            .into_iter()
            .rev()
            .map(|(heat, players)| {
                let mut entries: Vec<DelveBoardEntry> = players.into_values().collect();
                entries.sort_by_key(rank_key);
                entries.truncate(BOARD_LIMIT);
                DelveBoard { heat, entries }
            })
            .collect(),
    }
}

/// The marks of each folded run with the stamp it was folded at.
#[derive(Debug, Default)]
pub(crate) struct BoardCache {
    runs: Mutex<HashMap<Uuid, (String, Vec<Mark>)>>,
}

/// `GET /api/delves/leaderboard`: each player's deepest run per Heat.
pub(crate) async fn leaderboard(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DelveLeaderboard>, ApiError> {
    let delves = service(&app)?;
    app.auth().require_user(&headers).await?;
    let started = delves.store.started().await.map_err(|e| unavailable(&e))?;
    let stale: Vec<Uuid> = {
        let cache = delves
            .board
            .runs
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        started
            .iter()
            .filter(|run| {
                cache
                    .get(&run.header.id)
                    .is_none_or(|(stamp, _)| *stamp != run.stamp)
            })
            .map(|run| run.header.id)
            .collect()
    };
    let mut events = if stale.is_empty() {
        HashMap::new()
    } else {
        delves
            .store
            .events_of(&stale)
            .await
            .map_err(|e| unavailable(&e))?
    };
    let content = campaign()?;
    let mut cache = delves
        .board
        .runs
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    for run in &started {
        let header = &run.header;
        let Some(log) = events.remove(&header.id) else {
            continue;
        };
        // A run that no longer folds is left off the board rather than failing it.
        let folded = Run::replay(content, &header.theme, header.seed, header.heat, &log)
            .map(|folded| marks(&folded, header.heat, &run.stamp))
            .unwrap_or_default();
        cache.insert(header.id, (run.stamp.clone(), folded));
    }
    let live: HashSet<Uuid> = started.iter().map(|run| run.header.id).collect();
    cache.retain(|id, _| live.contains(id));
    let all: Vec<Mark> = cache
        .values()
        .flat_map(|(_, marks)| marks.clone())
        .collect();
    Ok(Json(boards(all)))
}

#[cfg(test)]
mod tests {
    //! Goal: the boards rank deeper first (floors cleared, then bosses beaten below them, then the earlier day), keep
    //! one best mark per player and Heat, come from the greatest Heat to 0 with an empty board for a Heat nobody played, and stop at
    //! the limit.
    use proptest::prelude::*;
    use rstest::rstest;

    use super::*;

    fn mark(user: &str, heat: u8, floors: u8, bosses: u8, date: &str) -> Mark {
        Mark {
            user: user.to_owned(),
            heat,
            entry: DelveBoardEntry {
                player: user.to_owned(),
                floors,
                bosses,
                commanders: vec!["Atraxa".to_owned()],
                party: 2,
                date: date.to_owned(),
            },
        }
    }

    /// The board of `heat`.
    fn at_heat(got: &DelveLeaderboard, heat: u8) -> &DelveBoard {
        got.boards.iter().find(|b| b.heat == heat).unwrap()
    }

    fn order(board: &DelveBoard) -> Vec<(String, u8, u8)> {
        board
            .entries
            .iter()
            .map(|e| (e.player.clone(), e.floors, e.bosses))
            .collect()
    }

    #[rstest]
    #[case::floors_before_bosses(&[("a", 1, 0), ("b", 0, 3)], &["a", "b"])]
    #[case::bosses_on_the_same_floor(&[("a", 2, 1), ("b", 2, 3)], &["b", "a"])]
    #[case::three_deep(&[("a", 0, 1), ("b", 3, 0), ("c", 1, 2)], &["b", "c", "a"])]
    fn deeper_ranks_first(#[case] runs: &[(&str, u8, u8)], #[case] expected: &[&str]) {
        let got = boards(
            runs.iter()
                .map(|(user, floors, bosses)| mark(user, 2, *floors, *bosses, "2026-11-01")),
        );
        let names: Vec<String> = at_heat(&got, 2)
            .entries
            .iter()
            .map(|e| e.player.clone())
            .collect();
        assert_eq!(names, expected);
    }

    #[test]
    fn a_tie_goes_to_whoever_got_there_first() {
        let got = boards([
            mark("late", 1, 1, 1, "2026-11-09"),
            mark("early", 1, 1, 1, "2026-11-02"),
        ]);
        assert_eq!(at_heat(&got, 1).entries[0].player, "early");
    }

    #[test]
    fn each_player_keeps_their_best_mark_per_heat() {
        let got = boards([
            mark("a", 2, 0, 2, "2026-11-01"),
            mark("a", 2, 1, 0, "2026-11-03"),
            mark("a", 2, 0, 3, "2026-11-04"),
            mark("a", 0, 0, 1, "2026-11-05"),
            mark("b", 2, 0, 1, "2026-11-01"),
        ]);
        let heats: Vec<u8> = got.boards.iter().map(|b| b.heat).collect();
        assert_eq!(
            heats,
            (0..=mtg_campaign::scale::MAX_HEAT)
                .rev()
                .collect::<Vec<u8>>(),
            "the greatest Heat first"
        );
        assert_eq!(
            order(at_heat(&got, 2)),
            vec![("a".to_owned(), 1, 0), ("b".to_owned(), 0, 1)]
        );
        assert_eq!(at_heat(&got, 1).entries, Vec::<DelveBoardEntry>::new());
        assert_eq!(order(at_heat(&got, 0)), vec![("a".to_owned(), 0, 1)]);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]
        #[test]
        fn a_board_is_sorted_unique_and_capped(
            runs in prop::collection::vec((0u8..30, 0u8..=2, 0u8..4, 0u8..4, 1u8..28), 0..80)
        ) {
            let got = boards(runs.iter().map(|(user, heat, floors, bosses, day)| {
                mark(&format!("p{user}"), *heat, *floors, *bosses, &format!("2026-11-{day:02}"))
            }));
            for board in &got.boards {
                prop_assert!(board.entries.len() <= BOARD_LIMIT);
                prop_assert!(board.entries.windows(2).all(|w| rank_key(&w[0]) <= rank_key(&w[1])));
                let mut players: Vec<&str> = board.entries.iter().map(|e| e.player.as_str()).collect();
                players.sort_unstable();
                players.dedup();
                prop_assert_eq!(players.len(), board.entries.len());
                let users = runs
                    .iter()
                    .filter(|r| r.1 == board.heat)
                    .map(|r| r.0)
                    .collect::<HashSet<_>>()
                    .len();
                prop_assert_eq!(board.entries.len(), users.min(BOARD_LIMIT));
            }
        }
    }
}
