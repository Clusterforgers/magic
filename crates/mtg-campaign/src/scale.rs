//! How a boss scales: its innate's level by depth (bosses the party defeated in the run) and Heat, its life and
//! reinforcements by party size only, and the rounds its two phases begin in (`docs/delves.md`, Difficulty).

use crate::run::{
    BOSS_SCALE_FROM, PARTY_SCALE_FROM, REINFORCEMENTS_FADE_FROM, THEME_REINFORCEMENTS_FROM,
    THEME_SUPPLIES_FROM,
};

/// The most heroes in a party.
pub const MOST: usize = 6;
/// The hottest Heat.
pub const MAX_HEAT: u8 = 3;
/// The rounds whose last turn's end step begins Phase 1 and Phase 2.
pub const PHASE_ROUNDS: [u32; 2] = [8, 14];
/// A boss's life against one hero.
pub const BOSS_LIFE: i32 = 40;
/// What each hero after the first adds to a boss's life.
pub const BOSS_LIFE_PER_HERO: i32 = 15;
/// The card a boss brings once for each hero after the first, from [`BOSS_SCALE_FROM`].
pub const REINFORCEMENT: &str = "reinforcements";
/// The half reinforcement (an additional card each draw step) a theme may add from [`THEME_SUPPLIES_FROM`].
pub const SUPPLIES: &str = "supplies";
/// The card a boss brings for each hero after [`PARTY_BASE`], from [`PARTY_SCALE_FROM`].
pub const WARBAND: &str = "warband";
/// The most heroes that each bring the boss a reinforcement from [`PARTY_SCALE_FROM`]; each further one brings a
/// [`WARBAND`] and [`BOSS_LIFE_BEYOND`] life.
pub const PARTY_BASE: usize = 3;
/// What each hero after [`PARTY_BASE`] adds to a boss's life from [`PARTY_SCALE_FROM`].
pub const BOSS_LIFE_BEYOND: i32 = 15;
/// The relic count from which a boss brings no reinforcement, from [`REINFORCEMENTS_FADE_FROM`].
pub const FADE_RELICS: usize = 6;
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

    /// A boss's starting life against `heroes` heroes (at least one) in a run of content `generation`: [`BOSS_LIFE`]
    /// and [`BOSS_LIFE_PER_HERO`] for each hero after the first, from [`PARTY_SCALE_FROM`] [`BOSS_LIFE_BEYOND`] more
    /// for each hero after [`PARTY_BASE`].
    pub fn boss_life(heroes: usize, generation: u32) -> i32 {
        let heroes = heroes.clamp(1, MOST);
        let extra = i32::try_from(heroes - 1).unwrap_or(0);
        let beyond = Self::beyond(heroes, generation);
        BOSS_LIFE
            + BOSS_LIFE_PER_HERO * extra
            + BOSS_LIFE_BEYOND * i32::try_from(beyond).unwrap_or(0)
    }

    /// The heroes after [`PARTY_BASE`] in a party of `heroes` from [`PARTY_SCALE_FROM`]; none before.
    fn beyond(heroes: usize, generation: u32) -> usize {
        if generation >= PARTY_SCALE_FROM {
            heroes.clamp(1, MOST).saturating_sub(PARTY_BASE)
        } else {
            0
        }
    }

    /// The reinforcements a boss brings against `heroes` heroes in a run of content `generation`: one
    /// [`REINFORCEMENT`] for each hero after the first, none before [`BOSS_SCALE_FROM`], and from
    /// [`PARTY_SCALE_FROM`] none for the heroes after [`PARTY_BASE`].
    pub fn reinforcements(heroes: usize, generation: u32) -> Vec<String> {
        let extra = if generation >= BOSS_SCALE_FROM {
            heroes.clamp(1, MOST) - 1 - Self::beyond(heroes, generation)
        } else {
            0
        };
        vec![REINFORCEMENT.to_owned(); extra]
    }

    /// The reinforcements a boss of a theme that adds `supplies` (`Theme::boss_supplies`) and brings `relics` relics
    /// brings against `heroes` heroes in a run of content `generation`: [`Scale::reinforcements`], but
    /// none from [`REINFORCEMENTS_FADE_FROM`] with [`FADE_RELICS`] relics or more, and from
    /// [`THEME_SUPPLIES_FROM`] the theme's [`SUPPLIES`] against two or more heroes; from
    /// [`THEME_REINFORCEMENTS_FROM`] at most `most` (`Theme::boss_reinforcements_most`) reinforcements, each further one
    /// coming as [`SUPPLIES`]; from [`PARTY_SCALE_FROM`] a [`WARBAND`] for each hero after [`PARTY_BASE`], faded or not.
    pub fn theme_reinforcements(
        heroes: usize,
        generation: u32,
        (supplies, most): (usize, Option<usize>),
        relics: usize,
    ) -> Vec<String> {
        let mut all = Self::reinforcements(heroes, generation);
        if generation >= REINFORCEMENTS_FADE_FROM && relics >= FADE_RELICS {
            all.clear();
        }
        if let Some(most) = most.filter(|_| generation >= THEME_REINFORCEMENTS_FROM) {
            for beyond in all.iter_mut().skip(most) {
                SUPPLIES.clone_into(beyond);
            }
        }
        if generation >= THEME_SUPPLIES_FROM && heroes >= 2 {
            all.extend(std::iter::repeat_n(SUPPLIES.to_owned(), supplies));
        }
        all.extend(std::iter::repeat_n(
            WARBAND.to_owned(),
            Self::beyond(heroes, generation),
        ));
        all
    }
}
