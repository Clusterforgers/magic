//! How a boss scales: its innate's level by depth (bosses the party defeated in the run) and Heat, its life by party
//! size only, and the rounds its two phases begin in (`docs/delves.md`, Difficulty).

/// The most heroes in a party.
pub const MOST: usize = 6;
/// The hottest Heat.
pub const MAX_HEAT: u8 = 2;
/// The rounds whose last turn's end step begins Phase 1 and Phase 2.
pub const PHASE_ROUNDS: [u32; 2] = [8, 14];
/// A boss's life against one hero.
pub const BOSS_LIFE: i32 = 40;
/// What each hero after the first adds to a boss's life.
pub const BOSS_LIFE_PER_HERO: i32 = 15;
/// The bosses of a run whose depth counts one point each; beyond them every second boss adds a point.
pub const FULL_DEPTH: u32 = 4;

/// The boss's scaling rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scale;

impl Scale {
    /// The level (1 to [`LEVELS`](crate::innate::LEVELS)) of a boss's innate at `depth` (bosses the party defeated in
    /// the run, over all floors) and `heat`: `1 + (depth points + Heat) / 2`.
    pub fn innate_level(depth: u32, heat: u8) -> u8 {
        let points = if depth < FULL_DEPTH {
            depth
        } else {
            FULL_DEPTH + (depth - FULL_DEPTH) / 2
        };
        let level = 1 + u32::midpoint(points, u32::from(heat.min(MAX_HEAT)));
        u8::try_from(level.min(u32::from(crate::innate::LEVELS))).unwrap_or(crate::innate::LEVELS)
    }

    /// A boss's starting life against `heroes` heroes (at least one).
    pub fn boss_life(heroes: usize) -> i32 {
        let extra = i32::try_from(heroes.clamp(1, MOST) - 1).unwrap_or(0);
        BOSS_LIFE + BOSS_LIFE_PER_HERO * extra
    }
}
