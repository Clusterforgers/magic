//! Encounter modifiers: one game-warping rule rolled for every fight from a pool both themes share, each an emblem
//! card. A modifier is never rolled for a boss it names in `not_with`; the roll is keyed by floor and room, so a
//! retried fight keeps it.

use serde::{Deserialize, Serialize};

use crate::rng::Stream;

/// One modifier of the pool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncounterModifier {
    /// The modifier's id, also the id of its card ([`crate::content::CardKind::Encounter`]).
    pub id: String,
    /// The bosses it is never rolled for, by id.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_with: Vec<String>,
}

impl EncounterModifier {
    /// Whether it may be rolled for `boss`.
    pub fn fits(&self, boss: &str) -> bool {
        !self.not_with.iter().any(|id| id == boss)
    }
}

/// The modifier of the fight against `boss` in `room` of `floor`, drawn by the run's `seed` from `pool`; none when no
/// modifier fits the boss.
pub fn draw<'a>(
    pool: &'a [EncounterModifier],
    boss: &str,
    seed: u64,
    floor: u8,
    room: u8,
) -> Option<&'a EncounterModifier> {
    let fits: Vec<&EncounterModifier> = pool.iter().filter(|m| m.fits(boss)).collect();
    let mut rng = Stream::new(seed, "modifier", u64::from(floor), u64::from(room));
    fits.get(rng.below(fits.len())).copied()
}
