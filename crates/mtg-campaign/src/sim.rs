//! Fight lengths for the bot simulator (`examples/delve_sim.rs`): the round each fight ended in and the boss phases
//! it reached, tallied per boss into one greppable line.

use mtg_engine::events::Event;

/// How long one fight lasted: the round it ended in and how many of the boss side's phases began.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FightLength {
    /// The round the fight ended in (`mtg_engine::turn::TurnState::round`).
    pub round: u32,
    /// The boss side's phases that began.
    pub phases: u8,
}

impl FightLength {
    /// The length of a fight that ended in round `round` after `events`, the boss's side seated from seat `party` on.
    pub fn of(round: u32, events: &[Event], party: usize) -> FightLength {
        let phases = events
            .iter()
            .filter(|e| matches!(e, Event::PhaseBegan { player, .. } if usize::try_from(player.get()).is_ok_and(|at| at >= party)))
            .count();
        FightLength {
            round,
            phases: u8::try_from(phases).unwrap_or(u8::MAX),
        }
    }
}

/// The lengths of a series of fights.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lengths {
    fights: Vec<FightLength>,
}

impl Lengths {
    /// Adds one fight.
    pub fn add(&mut self, length: FightLength) {
        self.fights.push(length);
    }

    /// Adds every fight of `other`.
    pub fn merge(&mut self, other: &Lengths) {
        self.fights.extend(&other.fights);
    }

    /// The fights counted.
    pub fn len(&self) -> usize {
        self.fights.len()
    }

    /// Whether no fight is counted.
    pub fn is_empty(&self) -> bool {
        self.fights.is_empty()
    }

    /// The median round the fights ended in (the lower middle of an even count), none without fights.
    pub fn median(&self) -> Option<u32> {
        let mut rounds: Vec<u32> = self.fights.iter().map(|f| f.round).collect();
        rounds.sort_unstable();
        rounds.get(rounds.len().saturating_sub(1) / 2).copied()
    }

    /// The share of the fights, in whole percent, that reached phase `phase` (1 or 2).
    pub fn reached(&self, phase: u8) -> u32 {
        let reached = self.fights.iter().filter(|f| f.phases >= phase).count();
        percent(reached, self.fights.len())
    }

    /// The share of the fights, in whole percent, that ended in each round from the first to the last seen.
    pub fn by_round(&self) -> Vec<(u32, u32)> {
        let (Some(first), Some(last)) = (
            self.fights.iter().map(|f| f.round).min(),
            self.fights.iter().map(|f| f.round).max(),
        ) else {
            return Vec::new();
        };
        (first..=last)
            .map(|round| {
                let ended = self.fights.iter().filter(|f| f.round == round).count();
                (round, percent(ended, self.fights.len()))
            })
            .collect()
    }

    /// The tally as `key=value` fields: `median=11 p1=75% p2=25% ended=9:13%,10:25%,…` (rounds with no fight left out).
    pub fn fields(&self) -> String {
        let ended: Vec<String> = self
            .by_round()
            .into_iter()
            .filter(|(_, share)| *share > 0)
            .map(|(round, share)| format!("{round}:{share}%"))
            .collect();
        format!(
            "median={} p1={}% p2={}% ended={}",
            self.median()
                .map_or_else(|| "-".to_owned(), |m| m.to_string()),
            self.reached(1),
            self.reached(2),
            if ended.is_empty() {
                "-".to_owned()
            } else {
                ended.join(",")
            }
        )
    }
}

/// `part` of `whole` in whole percent, rounded to the nearest; 0 of nothing.
fn percent(part: usize, whole: usize) -> u32 {
    if whole == 0 {
        return 0;
    }
    let share = (part.saturating_mul(200) + whole) / whole.saturating_mul(2);
    u32::try_from(share).unwrap_or(u32::MAX)
}
