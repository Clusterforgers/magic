//! How a boss scales with the party and the Heat: its life, opening hand, brain and boss cards, extra combats, its
//! phases with the Last Bulwark for a big party, and the minions it fields. One table, so every party size from one to
//! six heroes meets the same challenge.

use crate::content::{Boss, Brain, Minion, Theme};

/// The boss card that gives a boss a card and a 2/2 Zombie each upkeep, once or more against a big party.
pub const REINFORCEMENTS: &str = "reinforcements";
/// The boss card that gives a boss a 2/2 Zombie each upkeep against a small party.
pub const RESERVES: &str = "reserves";
/// The boss card that gives a boss an extra combat each turn.
pub const WARPATH: &str = "warpath";
/// The boss card whose anthem grows with the boss's turns.
pub const ESCALATION: &str = "escalation";
/// The boss card that drains the party more each turn from the boss's tenth on.
pub const ENRAGE: &str = "enrage";
/// The phase that caps the attackers at the boss in its last phase.
pub const BULWARK: &str = "bulwark";
/// The most heroes a table is scaled for.
pub const MOST: usize = 6;

/// The boss's life by party size, in percent of its base and the life it gets for each hero after the first.
const LIFE: [i32; MOST + 1] = [100, 100, 115, 100, 85, 100, 90];
/// Reserves a boss gets, by party size.
const RESERVED: [usize; MOST + 1] = [0, 0, 1, 0, 0, 0, 0];
/// Reinforcements a boss gets, by party size.
const REINFORCED: [usize; MOST + 1] = [0, 0, 0, 1, 1, 2, 2];
/// Extra combats a boss gets each turn, by party size.
const COMBATS: [usize; MOST + 1] = [0, 0, 0, 0, 0, 0, 1];
/// Minions a boss fields, by party size.
const MINIONS: [usize; MOST + 1] = [0, 0, 0, 0, 1, 1, 2];
/// The smallest party a boss raises the Last Bulwark against.
const BULWARK_FROM: usize = 3;
/// Cards a boss's opening hand has fewer than its base against a single hero; it gets one more for each hero after the
/// first.
const SOLO_HAND: u32 = 5;
/// Life each Heat adds, in percent.
const HEAT_LIFE: i32 = 20;

/// The scaling for a party of `heroes` at `heat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scale {
    /// The heroes in the fight, 1 to [`MOST`].
    pub heroes: usize,
    /// The run's Heat.
    pub heat: u8,
}

impl Scale {
    /// The scaling for `heroes` (clamped to 1..=[`MOST`]) at `heat`.
    pub fn new(heroes: usize, heat: u8) -> Scale {
        Scale {
            heroes: heroes.clamp(1, MOST),
            heat,
        }
    }

    fn extra_heroes(self) -> i32 {
        i32::try_from(self.heroes - 1).unwrap_or(0)
    }

    /// `life` raised by the Heat.
    fn heated(self, life: i32) -> i32 {
        life + life * HEAT_LIFE * i32::from(self.heat) / 100
    }

    /// The boss's starting life: its base and more for each hero after the first, scaled by party size and raised by
    /// the Heat.
    pub fn life(self, boss: &Boss) -> i32 {
        let base = boss.life + boss.life_per_hero * self.extra_heroes();
        self.heated(base * LIFE[self.heroes] / 100)
    }

    /// The boss's opening hand: its base less [`SOLO_HAND`], and one card for each hero after the first.
    pub fn hand(self, boss: &Boss) -> u32 {
        let extra = u32::try_from(self.heroes - 1).unwrap_or(0);
        boss.hand.saturating_sub(SOLO_HAND).max(1) + extra
    }

    /// The brain of a boss or minion: one level stronger from Heat 2.
    pub fn brain(self, brain: Brain) -> Brain {
        if self.heat >= 2 {
            brain.stronger()
        } else {
            brain
        }
    }

    /// The extra combats the boss gets each turn.
    pub fn extra_combats(self) -> usize {
        COMBATS[self.heroes]
    }

    /// The custom cards in the boss's command zone from the start: its own, Escalation and Enrage, and by party size
    /// Reinforcements, Reserves and Warpath for each extra combat.
    pub fn rules(self, boss: &Boss) -> Vec<String> {
        let mut rules = boss.rules.clone();
        rules.push(ESCALATION.to_owned());
        rules.push(ENRAGE.to_owned());
        rules.extend(std::iter::repeat_n(
            REINFORCEMENTS.to_owned(),
            REINFORCED[self.heroes],
        ));
        rules.extend(std::iter::repeat_n(
            RESERVES.to_owned(),
            RESERVED[self.heroes],
        ));
        rules.extend(std::iter::repeat_n(
            WARPATH.to_owned(),
            self.extra_combats(),
        ));
        rules
    }

    /// The boss's phases at their life thresholds out of `life`; against a party of [`BULWARK_FROM`] or more the Last
    /// Bulwark begins with the last phase.
    pub fn phases(self, boss: &Boss, life: i32) -> Vec<(String, i32)> {
        let at = |percent: u32| (life * i32::try_from(percent).unwrap_or(0) / 100).max(1);
        let mut phases: Vec<(String, i32)> = boss
            .phases
            .iter()
            .map(|(id, percent)| (id.clone(), at(*percent)))
            .collect();
        if self.heroes >= BULWARK_FROM
            && let Some((_, last)) = phases.last().cloned()
        {
            phases.push((BULWARK.to_owned(), last));
        }
        phases
    }

    /// The minions the boss fields: the theme's first ones that do not play the boss's own deck.
    pub fn minions<'a>(self, theme: &'a Theme, boss: &Boss) -> Vec<&'a Minion> {
        theme
            .minions
            .iter()
            .filter(|m| m.deck_of != boss.id)
            .take(MINIONS[self.heroes])
            .collect()
    }

    /// A minion's starting life, raised by the Heat.
    pub fn minion_life(self, minion: &Minion) -> i32 {
        self.heated(minion.life)
    }
}
