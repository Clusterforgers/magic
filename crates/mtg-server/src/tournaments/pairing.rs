//! Pairings and standings: pure functions over entrants and the matches played so far.
//!
//! An entrant is a number in sign-up order. A match is played between two entrants, or is a bye
//! for one. Match points are 3 for a win or a bye and 1 for a draw; every match is one game, so a
//! game-win fraction counts the same games as the match record (a bye is one game won).

use std::cmp::Ordering;
use std::collections::HashSet;

use mtg_protocol::rest::MatchOutcome;

/// Steps the Swiss search may take before it accepts a rematch.
const SEARCH_BUDGET: usize = 200_000;
/// The least match-win or game-win fraction counted for an opponent.
const FLOOR: f64 = 1.0 / 3.0;

/// One match of a round, played or still being played.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Played {
    /// The first entrant.
    pub(crate) a: u32,
    /// The second entrant; none for a bye.
    pub(crate) b: Option<u32>,
    /// The result; none while the game runs. A bye always counts as won by `a`.
    pub(crate) outcome: Option<MatchOutcome>,
}

impl Played {
    /// The entrant that won, if any; a bye is won by `a`.
    pub(crate) fn winner(&self) -> Option<u32> {
        match (self.b, self.outcome) {
            (None, _) | (Some(_), Some(MatchOutcome::A)) => Some(self.a),
            (Some(b), Some(MatchOutcome::B)) => Some(b),
            _ => None,
        }
    }

    /// Whether the match has a result (a bye always has one).
    pub(crate) fn decided(&self) -> bool {
        self.b.is_none() || self.outcome.is_some()
    }
}

/// One pairing of a new round: two entrants, or a bye for `a`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Pairing {
    /// The first entrant.
    pub(crate) a: u32,
    /// The second entrant; none for a bye.
    pub(crate) b: Option<u32>,
}

/// One entrant's record and tiebreakers.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Standing {
    /// The entrant.
    pub(crate) entrant: u32,
    /// Match points.
    pub(crate) points: u32,
    /// Matches won, byes included.
    pub(crate) wins: u32,
    /// Matches lost.
    pub(crate) losses: u32,
    /// Matches drawn.
    pub(crate) draws: u32,
    /// Byes received.
    pub(crate) byes: u32,
    /// The average match-win fraction of the opponents, each at least 1/3; 0 without opponents.
    pub(crate) opponents_match_win: f64,
    /// The own game-win fraction, at least 1/3.
    pub(crate) game_win: f64,
    /// The average game-win fraction of the opponents, each at least 1/3; 0 without opponents.
    pub(crate) opponents_game_win: f64,
}

/// A small seeded generator, so a stored seed replays the same shuffles.
#[derive(Debug, Clone)]
pub(crate) struct Shuffler(u64);

impl Shuffler {
    /// A generator that starts from `seed`.
    pub(crate) fn new(seed: u64) -> Shuffler {
        Shuffler(seed)
    }

    /// The next value (splitmix64).
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Shuffles `items` in place (Fisher-Yates).
    pub(crate) fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let bound = u64::try_from(i + 1).unwrap_or(u64::MAX);
            let j = usize::try_from(self.next() % bound).unwrap_or(0);
            items.swap(i, j);
        }
    }
}

/// Rounds of Swiss that leave at most one entrant undefeated: the base-2 logarithm of the field,
/// rounded up, at least 1.
pub(crate) fn swiss_rounds(entrants: usize) -> u32 {
    elimination_rounds(entrants).max(1)
}

/// Rounds a single-elimination bracket of `entrants` takes.
pub(crate) fn elimination_rounds(entrants: usize) -> u32 {
    let mut rounds = 0;
    while (1usize << rounds) < entrants {
        rounds += 1;
    }
    rounds
}

/// One entrant's results so far.
struct Record {
    points: u32,
    wins: u32,
    losses: u32,
    draws: u32,
    byes: u32,
    opponents: Vec<u32>,
}

/// The results of `entrants` in `rounds`, in the order of `entrants`.
fn records(entrants: &[u32], rounds: &[Vec<Played>]) -> Vec<Record> {
    let index = |entrant: u32| entrants.iter().position(|&e| e == entrant);
    let mut records: Vec<Record> = entrants
        .iter()
        .map(|_| Record {
            points: 0,
            wins: 0,
            losses: 0,
            draws: 0,
            byes: 0,
            opponents: Vec::new(),
        })
        .collect();
    for played in rounds.iter().flatten() {
        let Some(a) = index(played.a) else { continue };
        match (played.b, played.outcome) {
            (None, _) => {
                let record = &mut records[a];
                record.points += 3;
                record.wins += 1;
                record.byes += 1;
            }
            (Some(b), Some(outcome)) => {
                let Some(b_at) = index(b) else { continue };
                records[a].opponents.push(b);
                records[b_at].opponents.push(played.a);
                let decided = match outcome {
                    MatchOutcome::A => Some((a, b_at)),
                    MatchOutcome::B => Some((b_at, a)),
                    MatchOutcome::Draw => None,
                };
                if let Some((won, lost)) = decided {
                    records[won].points += 3;
                    records[won].wins += 1;
                    records[lost].losses += 1;
                } else {
                    for at in [a, b_at] {
                        records[at].points += 1;
                        records[at].draws += 1;
                    }
                }
            }
            (Some(_), None) => {}
        }
    }
    records
}

/// The standings of `entrants` after `rounds`, the leader first: by match points, then the
/// opponents' match-win fraction, the own game-win fraction, the opponents' game-win fraction, and
/// sign-up order. Matches without a result count for nothing yet.
pub(crate) fn standings(entrants: &[u32], rounds: &[Vec<Played>]) -> Vec<Standing> {
    let index = |entrant: u32| entrants.iter().position(|&e| e == entrant);
    let records = records(entrants, rounds);
    // One game per match, so the match and game fractions share a record.
    let win_fraction = |record: &Record| {
        let played = record.wins + record.losses + record.draws;
        if played == 0 {
            FLOOR
        } else {
            (f64::from(record.points) / f64::from(3 * played)).max(FLOOR)
        }
    };
    let fractions: Vec<f64> = records.iter().map(win_fraction).collect();
    let average = |opponents: &[u32]| {
        let values: Vec<f64> = opponents
            .iter()
            .filter_map(|&opponent| index(opponent).map(|at| fractions[at]))
            .collect();
        if values.is_empty() {
            0.0
        } else {
            values.iter().sum::<f64>() / f64::from(u32::try_from(values.len()).unwrap_or(u32::MAX))
        }
    };
    let mut table: Vec<Standing> = entrants
        .iter()
        .zip(&records)
        .zip(&fractions)
        .map(|((&entrant, record), &fraction)| {
            let opponents = average(&record.opponents);
            Standing {
                entrant,
                points: record.points,
                wins: record.wins,
                losses: record.losses,
                draws: record.draws,
                byes: record.byes,
                opponents_match_win: opponents,
                game_win: fraction,
                opponents_game_win: opponents,
            }
        })
        .collect();
    table.sort_by(|x, y| {
        y.points
            .cmp(&x.points)
            .then(desc(x.opponents_match_win, y.opponents_match_win))
            .then(desc(x.game_win, y.game_win))
            .then(desc(x.opponents_game_win, y.opponents_game_win))
            .then(x.entrant.cmp(&y.entrant))
    });
    table
}

/// Larger first.
fn desc(x: f64, y: f64) -> Ordering {
    y.partial_cmp(&x).unwrap_or(Ordering::Equal)
}

/// The pairings of the next Swiss round for `ranked` (active entrants, the leader first): a bye for
/// the lowest-ranked entrant without one when the field is odd, then each entrant against the
/// highest-ranked one it has not met. A rematch is made only when no pairing avoids it.
pub(crate) fn swiss(ranked: &[u32], rounds: &[Vec<Played>]) -> Vec<Pairing> {
    let mut met: HashSet<(u32, u32)> = HashSet::new();
    let mut had_bye: HashSet<u32> = HashSet::new();
    for played in rounds.iter().flatten() {
        match played.b {
            Some(b) => {
                met.insert((played.a.min(b), played.a.max(b)));
            }
            None => {
                had_bye.insert(played.a);
            }
        }
    }
    let mut field: Vec<u32> = ranked.to_vec();
    let mut pairings = Vec::new();
    let mut bye = None;
    if field.len() % 2 == 1 {
        let at = field
            .iter()
            .rposition(|entrant| !had_bye.contains(entrant))
            .unwrap_or(field.len() - 1);
        bye = Some(field.remove(at));
    }
    let mut budget = SEARCH_BUDGET;
    let fresh = pair_fresh(&field, &met, &mut budget);
    let pairs = fresh.unwrap_or_else(|| field.chunks(2).map(|c| (c[0], c[1])).collect());
    pairings.extend(pairs.into_iter().map(|(a, b)| Pairing { a, b: Some(b) }));
    if let Some(a) = bye {
        pairings.push(Pairing { a, b: None });
    }
    pairings
}

/// Pairs `field` (even length) so nobody meets an old opponent, preferring the highest-ranked
/// partner; `None` when no such pairing exists or the budget ran out.
fn pair_fresh(
    field: &[u32],
    met: &HashSet<(u32, u32)>,
    budget: &mut usize,
) -> Option<Vec<(u32, u32)>> {
    let Some((&first, rest)) = field.split_first() else {
        return Some(Vec::new());
    };
    for at in 0..rest.len() {
        if *budget == 0 {
            return None;
        }
        *budget -= 1;
        let other = rest[at];
        if met.contains(&(first.min(other), first.max(other))) {
            continue;
        }
        let remaining: Vec<u32> = rest
            .iter()
            .enumerate()
            .filter(|&(i, _)| i != at)
            .map(|(_, &e)| e)
            .collect();
        if let Some(mut pairs) = pair_fresh(&remaining, met, budget) {
            pairs.insert(0, (first, other));
            return Some(pairs);
        }
    }
    None
}

/// The pairings of the next elimination round for `alive` (the entrants still in, in bracket order):
/// neighbours meet, and an odd field gives the last entrant without a bye so far a bye.
pub(crate) fn elimination(alive: &[u32], rounds: &[Vec<Played>]) -> Vec<Pairing> {
    let had_bye: HashSet<u32> = rounds
        .iter()
        .flatten()
        .filter(|played| played.b.is_none())
        .map(|played| played.a)
        .collect();
    let mut field = alive.to_vec();
    let mut bye = None;
    if field.len() % 2 == 1 {
        let at = field
            .iter()
            .rposition(|entrant| !had_bye.contains(entrant))
            .unwrap_or(field.len() - 1);
        bye = Some(field.remove(at));
    }
    let mut pairings: Vec<Pairing> = field
        .chunks(2)
        .map(|pair| Pairing {
            a: pair[0],
            b: Some(pair[1]),
        })
        .collect();
    if let Some(a) = bye {
        pairings.push(Pairing { a, b: None });
    }
    pairings
}

/// The entrants still in an elimination bracket after `last` (the latest round), in bracket order:
/// each match's winner, minus the dropped.
pub(crate) fn survivors(last: &[Played], dropped: &HashSet<u32>) -> Vec<u32> {
    last.iter()
        .filter_map(Played::winner)
        .filter(|entrant| !dropped.contains(entrant))
        .collect()
}

#[cfg(test)]
mod tests {
    //! Goal: Swiss avoids rematches and hands out byes fairly, elimination halves the field, and the
    //! standings order by points and the tiebreakers.

    use proptest::prelude::*;
    use rstest::rstest;

    use super::*;

    fn played(a: u32, b: Option<u32>, outcome: Option<MatchOutcome>) -> Played {
        Played { a, b, outcome }
    }

    fn to_played(pairings: &[Pairing], winner_first: bool) -> Vec<Played> {
        pairings
            .iter()
            .map(|p| Played {
                a: p.a,
                b: p.b,
                outcome: p.b.map(|_| {
                    if winner_first {
                        MatchOutcome::A
                    } else {
                        MatchOutcome::B
                    }
                }),
            })
            .collect()
    }

    #[rstest]
    #[case(1, 1, 0)]
    #[case(2, 1, 1)]
    #[case(3, 2, 2)]
    #[case(4, 2, 2)]
    #[case(5, 3, 3)]
    #[case(8, 3, 3)]
    #[case(9, 4, 4)]
    fn round_counts(#[case] entrants: usize, #[case] swiss: u32, #[case] bracket: u32) {
        assert_eq!(swiss_rounds(entrants), swiss);
        assert_eq!(elimination_rounds(entrants), bracket);
    }

    #[test]
    fn the_shuffle_is_a_permutation_fixed_by_its_seed() {
        let mut one: Vec<u32> = (0..20).collect();
        let mut two = one.clone();
        Shuffler::new(7).shuffle(&mut one);
        Shuffler::new(7).shuffle(&mut two);
        assert_eq!(one, two);
        let mut sorted = one.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..20).collect::<Vec<_>>());
        assert_ne!(one, (0..20).collect::<Vec<_>>());
    }

    #[rstest]
    #[case::win(Some(MatchOutcome::A), [3, 0], [1, 0, 0], [0, 1, 0])]
    #[case::loss(Some(MatchOutcome::B), [0, 3], [0, 1, 0], [1, 0, 0])]
    #[case::draw(Some(MatchOutcome::Draw), [1, 1], [0, 0, 1], [0, 0, 1])]
    #[case::unplayed(None, [0, 0], [0, 0, 0], [0, 0, 0])]
    fn match_points(
        #[case] outcome: Option<MatchOutcome>,
        #[case] points: [u32; 2],
        #[case] first: [u32; 3],
        #[case] second: [u32; 3],
    ) {
        let table = standings(&[0, 1], &[vec![played(0, Some(1), outcome)]]);
        let of = |e: u32| table.iter().find(|s| s.entrant == e).cloned();
        let (Some(zero), Some(one)) = (of(0), of(1)) else {
            panic!("both entrants are listed");
        };
        assert_eq!([zero.points, one.points], points);
        assert_eq!([zero.wins, zero.losses, zero.draws], first);
        assert_eq!([one.wins, one.losses, one.draws], second);
    }

    #[test]
    fn a_bye_is_a_win_and_no_opponent() {
        let table = standings(&[0], &[vec![played(0, None, None)]]);
        assert_eq!(table[0].points, 3);
        assert_eq!(table[0].byes, 1);
        assert!(table[0].opponents_match_win.abs() < 1e-9);
        assert!((table[0].game_win - 1.0).abs() < 1e-9);
    }

    #[test]
    fn opponents_match_win_breaks_a_tie_in_points() {
        // 0 beat 2 who went on to win; 1 beat 3 who lost again. Both 0 and 1 have 3 points.
        let rounds = vec![
            vec![
                played(0, Some(2), Some(MatchOutcome::A)),
                played(1, Some(3), Some(MatchOutcome::A)),
            ],
            vec![
                played(0, Some(1), Some(MatchOutcome::Draw)),
                played(2, Some(3), Some(MatchOutcome::A)),
            ],
        ];
        let table = standings(&[0, 1, 2, 3], &rounds);
        let order: Vec<u32> = table.iter().map(|s| s.entrant).collect();
        assert_eq!(order, vec![0, 1, 2, 3]);
        assert!(table[0].opponents_match_win > table[1].opponents_match_win);
        // An opponent who won nothing counts as 1/3, not 0.
        assert!(table[1].opponents_match_win >= FLOOR);
    }

    #[test]
    fn swiss_never_rematches_while_avoidable() {
        let ranked = [0, 1, 2, 3];
        let rounds = vec![vec![
            played(0, Some(1), Some(MatchOutcome::A)),
            played(2, Some(3), Some(MatchOutcome::A)),
        ]];
        let next = swiss(&ranked, &rounds);
        assert_eq!(
            next,
            vec![Pairing { a: 0, b: Some(2) }, Pairing { a: 1, b: Some(3) }]
        );
    }

    #[test]
    fn swiss_rematches_only_when_forced() {
        // Two entrants left can only meet again.
        let rounds = vec![vec![played(0, Some(1), Some(MatchOutcome::A))]];
        assert_eq!(swiss(&[0, 1], &rounds), vec![Pairing { a: 0, b: Some(1) }]);
    }

    #[test]
    fn the_bye_goes_to_the_lowest_ranked_without_one() {
        let rounds = vec![vec![
            played(0, Some(1), Some(MatchOutcome::A)),
            played(2, None, None),
        ]];
        let next = swiss(&[0, 2, 1], &rounds);
        assert!(next.contains(&Pairing { a: 1, b: None }));
        let next = swiss(&[0, 1, 2], &rounds);
        assert!(next.contains(&Pairing { a: 1, b: None }));
    }

    #[rstest]
    #[case(2, 1)]
    #[case(5, 3)]
    #[case(8, 3)]
    #[case(13, 4)]
    fn elimination_ends_with_one_entrant(#[case] field: u32, #[case] rounds_needed: u32) {
        let mut alive: Vec<u32> = (0..field).collect();
        let mut rounds: Vec<Vec<Played>> = Vec::new();
        while alive.len() > 1 {
            let pairings = elimination(&alive, &rounds);
            let round = to_played(&pairings, true);
            alive = survivors(&round, &HashSet::new());
            rounds.push(round);
        }
        assert_eq!(rounds.len(), usize::try_from(rounds_needed).unwrap_or(0));
        assert_eq!(alive, vec![0]);
    }

    #[test]
    fn a_dropped_winner_leaves_the_bracket() {
        let round = vec![
            played(0, Some(1), Some(MatchOutcome::A)),
            played(2, Some(3), Some(MatchOutcome::B)),
        ];
        let dropped: HashSet<u32> = [3].into_iter().collect();
        assert_eq!(survivors(&round, &dropped), vec![0]);
    }

    proptest! {
        #[test]
        fn swiss_seats_everyone_once_without_early_rematches(
            field in 2u32..24,
            seed in any::<u64>(),
            results in proptest::collection::vec(0u8..3, 64),
        ) {
            let entrants: Vec<u32> = (0..field).collect();
            let mut order = entrants.clone();
            Shuffler::new(seed).shuffle(&mut order);
            let mut rounds: Vec<Vec<Played>> = Vec::new();
            let mut results = results.into_iter().cycle();
            for round in 0..swiss_rounds(field as usize) {
                let ranked: Vec<u32> = if round == 0 {
                    order.clone()
                } else {
                    standings(&entrants, &rounds).iter().map(|s| s.entrant).collect()
                };
                let pairings = swiss(&ranked, &rounds);
                let mut seen: Vec<u32> = pairings
                    .iter()
                    .flat_map(|p| std::iter::once(p.a).chain(p.b))
                    .collect();
                seen.sort_unstable();
                prop_assert_eq!(&seen, &entrants);
                prop_assert!(pairings.iter().filter(|p| p.b.is_none()).count() <= 1);
                for p in &pairings {
                    if let Some(b) = p.b {
                        let rematch = rounds.iter().flatten().any(|old| {
                            old.b.is_some_and(|ob| (old.a, ob) == (p.a, b) || (old.a, ob) == (b, p.a))
                        });
                        // Within log2(field) rounds a fresh pairing always exists.
                        prop_assert!(!rematch);
                    }
                }
                let outcomes = [MatchOutcome::A, MatchOutcome::B, MatchOutcome::Draw];
                rounds.push(
                    pairings
                        .iter()
                        .map(|p| Played {
                            a: p.a,
                            b: p.b,
                            outcome: p.b.map(|_| outcomes[usize::from(results.next().unwrap_or(0))]),
                        })
                        .collect(),
                );
            }
            let byes: Vec<u32> = rounds.iter().flatten().filter(|p| p.b.is_none()).map(|p| p.a).collect();
            let unique: HashSet<u32> = byes.iter().copied().collect();
            prop_assert_eq!(unique.len(), byes.len());
        }
    }
}
