//! The amounts that branch on a condition, read the mana pool or the mana spent, find the most common value, count
//! unlocked doors or draw a number from the game's seeded random stream.

use std::collections::BTreeMap;

use mtg_cards::refs::{Ref, Selector};
use mtg_cards::value::{Amount, CounterKind, StatKind};
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaType;

use super::share::Share;
use super::{ResolveError, Scope, Target};
use crate::rng::GameRng;

/// Marks the substreams of random amounts, apart from the streams the shuffles and rolls number from zero.
const RANDOM_AMOUNT: u64 = 1 << 63;

impl Scope<'_> {
    /// The amounts of this file; `amount_of` hands them over.
    pub(super) fn fill_amount(
        &self,
        amount: &Amount,
        share: Option<Share>,
    ) -> Result<i64, ResolveError> {
        match amount {
            Amount::When {
                condition,
                then,
                otherwise,
            } => {
                let branch = if self.holds(condition)? {
                    then
                } else {
                    otherwise
                };
                self.amount_of(branch, share)
            }
            Amount::ManaSpent { of } => Ok(self.spent_of(*of)),
            Amount::ManaColors(who) => self.pooled_colors(who),
            Amount::ManaInPool(who) => self.pooled(who),
            Amount::MostCommon { of, stat } => self.most_common(of, *stat),
            Amount::Doors(of) => self.unlocked_doors(of),
            Amount::Random { min, max } => Ok(self.random_between(*min, *max)),
            _ => Err(ResolveError::Invalid("not a filled amount".into())),
        }
    }

    /// How many counters of the kind are on the objects and players the reference names, all together.
    pub(super) fn counters_on(&self, on: &Ref, counter: &CounterKind) -> Result<i64, ResolveError> {
        let mut total = 0i64;
        for named in self.refs(on)? {
            total += match named {
                Target::Player(player) => self
                    .state
                    .player(player)
                    .map_or(0, |held| i64::from(held.counters_of(counter))),
                Target::Object(object) => self
                    .seen(object)
                    .map_or(0, |seen| i64::from(seen.counters.of(counter))),
            };
        }
        Ok(total)
    }

    /// How much mana of the type, or of any, was spent to cast the spell the source is or came from.
    fn spent_of(&self, of: Option<ManaType>) -> i64 {
        let count = self
            .mana_spent()
            .iter()
            .filter(|mana| of.is_none_or(|kind| **mana == kind))
            .count();
        i64::try_from(count).unwrap_or(0)
    }

    /// The mana of each type in the pool of a player, restricted mana included.
    fn pool_counts(&self, player: PlayerId) -> [u32; 6] {
        let mut counts = [0; 6];
        if let Some(held) = self.state.player(player) {
            let pool = held.mana();
            for mana in ManaType::ALL {
                counts[mana.index()] = pool.get(mana);
            }
            for unit in pool.restricted() {
                counts[unit.mana.index()] += 1;
            }
        }
        counts
    }

    /// The mana in the pools of the players named, all together.
    fn pooled(&self, who: &Ref) -> Result<i64, ResolveError> {
        let mut total = 0i64;
        for player in self.players(who)? {
            total += self
                .pool_counts(player)
                .iter()
                .map(|n| i64::from(*n))
                .sum::<i64>();
        }
        Ok(total)
    }

    /// The colors, colorless not among them, held in the pools of the players named, each counted once.
    fn pooled_colors(&self, who: &Ref) -> Result<i64, ResolveError> {
        let mut held = [false; 6];
        for player in self.players(who)? {
            for (seen, n) in held.iter_mut().zip(self.pool_counts(player)) {
                *seen |= n > 0;
            }
        }
        let colors = ManaType::ALL
            .iter()
            .filter(|mana| **mana != ManaType::Colorless && held[mana.index()])
            .count();
        Ok(i64::try_from(colors).unwrap_or(0))
    }

    /// How many of the objects share the value of the characteristic that the most of them share; for colors each
    /// color is counted once for every object that has it.
    fn most_common(&self, of: &Selector, stat: StatKind) -> Result<i64, ResolveError> {
        let mut tally: BTreeMap<String, i64> = BTreeMap::new();
        for object in self.matching(of)? {
            let Some(seen) = self.seen(object) else {
                continue;
            };
            if stat == StatKind::Colors {
                for color in seen.chars.colors.iter() {
                    *tally.entry(format!("{color:?}")).or_default() += 1;
                }
            } else if let Some(value) = Self::stat(&seen, stat) {
                *tally.entry(value.to_string()).or_default() += 1;
            }
        }
        Ok(tally.into_values().max().unwrap_or(0))
    }

    /// The doors unlocked among the Rooms the selector names (CR 709.5).
    fn unlocked_doors(&self, of: &Selector) -> Result<i64, ResolveError> {
        let mut doors = 0i64;
        for object in self.matching(of)? {
            if let Some(rooms) = self.state.dk1c5_doors(object.id) {
                doors += rooms.iter().map(|open| i64::from(*open)).sum::<i64>();
            }
        }
        Ok(doors)
    }

    /// A number from `min` to `max` from the substream the source object and the effect's place choose, so the same
    /// state gives the same number and a replay of the log rebuilds the game; an inverted range gives `min`.
    fn random_between(&self, min: i32, max: i32) -> i64 {
        let (min, max) = (i64::from(min), i64::from(max));
        let Ok(span) = u64::try_from(max - min + 1) else {
            return min;
        };
        let place = self.state.resolving.as_ref().map_or(0, |resolving| {
            resolving.position.iter().fold(0u64, |all, step| {
                all.wrapping_mul(31)
                    .wrapping_add(u64::try_from(*step).unwrap_or(0) + 1)
            })
        });
        let key = RANDOM_AMOUNT
            ^ (self.source.id.get() << 24)
            ^ (u64::from(self.source.counter) << 8)
            ^ place;
        let index = self.state.rng_index().wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ key;
        let drawn = GameRng::substream(self.state.seed(), index).below(span);
        min + i64::try_from(drawn).unwrap_or(0)
    }
}
