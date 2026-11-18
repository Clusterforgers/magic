//! How a boss scales with the party, its place on the ladder and the Heat: its life, opening hand, brain and boss
//! cards, extra combats, its phases with the Last Bulwark for a big party, and the minions it fields. One table by
//! party size, so every size from one to six heroes meets the same challenge, and one by ladder rank and Heat, so the
//! first boss is the gentlest, each later one harder and each Heat a clear step up; from content generation 4 that
//! table also picks the bracket of the boss's deck, which carries most of the step.

use crate::content::{Boss, Brain, Minion, Theme};

/// The boss card that gives a boss a card and a 2/2 Zombie each upkeep, once or more against a big party.
pub const REINFORCEMENTS: &str = "reinforcements";
/// The boss card that gives a boss a 2/2 Zombie each upkeep against a small party.
pub const RESERVES: &str = "reserves";
/// The boss card that gives a boss an extra combat each turn.
pub const WARPATH: &str = "warpath";
/// The boss card whose end-step drain grows with the boss's turns: every boss's one life loss with no way out.
pub const ESCALATION: &str = "escalation";
/// The phase that caps the attackers at the boss in its last phase.
pub const BULWARK: &str = "bulwark";
/// The most heroes a table is scaled for.
pub const MOST: usize = 6;
/// The highest Heat.
pub const MAX_HEAT: u8 = 2;
/// The ladder ranks the tier table has rows for; a later boss plays the last row.
pub const RANKS: usize = 4;
/// The heroes' turn each boss's Enrage card is written for (Heat 1).
pub const ENRAGE_WRITTEN: u32 = 7;
/// The heroes' turn the Enrage begins on, by Heat.
pub const ENRAGE_TURN: [u32; MAX_HEAT as usize + 1] = [9, ENRAGE_WRITTEN, 6];
/// The earliest heroes' turn an Intensified Enrage begins on.
pub const ENRAGE_EARLIEST: u32 = 4;
/// Every heroes' turn an Enrage may begin on: the Heats' and the earlier ones of Intensify.
pub const ENRAGE_TURNS: std::ops::RangeInclusive<u32> = ENRAGE_EARLIEST..=9;
/// Life in percent a boss gets for each bracket step a deeper floor would take it beyond [`MAX_BRACKET`].
pub const OVERFLOW_LIFE: i32 = 10;

/// The boss's life by party size, in percent of its base and the life it gets for each hero after the first.
const LIFE: [i32; MOST + 1] = [100, 100, 100, 100, 85, 100, 90];
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

/// How hard a boss plays at one ladder rank and Heat, on top of the party-size table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tier {
    /// The bracket of its deck ([`BRACKETS`]); 0 for the deck as written before brackets.
    pub bracket: u8,
    /// Its life and its minions', in percent of the party-size table's.
    pub life: i32,
    /// Cards its opening hand has more (or fewer) than the party-size table's.
    pub hand: i32,
    /// Brain levels above (or below) its own, for it and its minions.
    pub brain: i8,
    /// Minions it fields more (or fewer) than the party-size table's.
    pub minions: i8,
    /// Whether it brings its own relic.
    pub relic: bool,
    /// Whether it draws a modifier from the theme's pool.
    pub modifier: bool,
}

const fn tier(life: i32, hand: i32, brain: i8, minions: i8, relic: bool, modifier: bool) -> Tier {
    bracketed(0, life, hand, brain, minions, relic, modifier)
}

const fn bracketed(
    bracket: u8,
    life: i32,
    hand: i32,
    brain: i8,
    minions: i8,
    relic: bool,
    modifier: bool,
) -> Tier {
    Tier {
        bracket,
        life,
        hand,
        brain,
        minions,
        relic,
        modifier,
    }
}

/// The tier table, by Heat and then ladder rank: life %, hand, brain, minions, relic, modifier (`docs/delves.md`,
/// Difficulty). No row is easier than the one before it or the one at the Heat below.
pub const TIERS: [[Tier; RANKS]; MAX_HEAT as usize + 1] = [
    [
        tier(60, -2, -1, -1, false, false),
        tier(70, -2, -1, -1, false, true),
        tier(80, -1, 0, 0, true, true),
        tier(90, 0, 0, 0, true, true),
    ],
    [
        tier(75, -1, -1, -1, false, true),
        tier(90, 0, 0, 0, true, true),
        tier(100, 0, 0, 0, true, true),
        tier(110, 1, 0, 0, true, true),
    ],
    [
        tier(100, 0, 0, 0, false, true),
        tier(115, 1, 1, 0, true, true),
        tier(125, 1, 1, 0, true, true),
        tier(140, 2, 1, 0, true, true),
    ],
];

/// The content generation from which bosses play bracket decks ([`BRACKET_TIERS`]) with their commander card; a run of
/// an earlier one plays [`TIERS`] and the decks as written.
pub const BRACKETS_FROM: u32 = 4;
/// The content generation from which the two-colour bosses after Vex, the later upgraded forms and the uber boss join the
/// ladder; a run of an earlier generation never meets them, so its ladder stays as it was drawn.
pub const ROSTER_FROM: u32 = 5;
/// The content generation from which the last two-colour bosses (UR, WR, BG, UG) join the ladder; a run of an earlier
/// generation never meets them.
pub const PAIRS_FROM: u32 = 6;
/// The lowest Heat at which the uber boss may stand as the final boss.
pub const UBER_HEAT: u8 = 2;
/// The chance, one in this many, that a ladder at [`UBER_HEAT`] or above meets the uber boss as its final boss.
pub const UBER_ODDS: usize = 4;
/// The highest bracket.
pub const MAX_BRACKET: u8 = 5;
/// A boss deck's size by bracket, from bracket 1.
pub const DECK_SIZE: [u32; MAX_BRACKET as usize] = [40, 50, 60, 60, 60];

/// The bracket table, by Heat and then ladder rank (`docs/delves.md`, Difficulty): the first boss, the middle two, the
/// final boss.
pub const BRACKETS: [[u8; RANKS]; MAX_HEAT as usize + 1] =
    [[1, 2, 2, 3], [2, 3, 3, 4], [3, 4, 4, 5]];

/// The tier table of the bracket generation, by Heat and then ladder rank: bracket, life %, hand, brain, minions, relic,
/// modifier. The bracket carries most of the step; life, hand and brain move less than in [`TIERS`]. No row is easier
/// than the one before it or the one at the Heat below.
pub const BRACKET_TIERS: [[Tier; RANKS]; MAX_HEAT as usize + 1] = [
    [
        bracketed(BRACKETS[0][0], 75, -1, -1, -1, false, false),
        bracketed(BRACKETS[0][1], 80, -1, -1, -1, false, true),
        bracketed(BRACKETS[0][2], 85, 0, 0, 0, true, true),
        bracketed(BRACKETS[0][3], 90, 0, 0, 0, true, true),
    ],
    [
        bracketed(BRACKETS[1][0], 85, -1, -1, -1, false, true),
        bracketed(BRACKETS[1][1], 90, 0, 0, 0, true, true),
        bracketed(BRACKETS[1][2], 95, 0, 0, 0, true, true),
        bracketed(BRACKETS[1][3], 100, 0, 0, 0, true, true),
    ],
    [
        bracketed(BRACKETS[2][0], 95, 0, 0, 0, false, true),
        bracketed(BRACKETS[2][1], 105, 0, 0, 0, true, true),
        bracketed(BRACKETS[2][2], 110, 0, 0, 0, true, true),
        bracketed(BRACKETS[2][3], 120, 1, 1, 0, true, true),
    ],
];

/// The id of the Enrage card `id` made to begin on the heroes' turn `turn`: `id` itself on the turn it is written for.
pub fn enrage_id(id: &str, turn: u32) -> String {
    if turn == ENRAGE_WRITTEN {
        id.to_owned()
    } else {
        format!("{id}_at_{turn}")
    }
}

/// The scaling for a party of `heroes` at `heat` against the boss at `rank` on the ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scale {
    /// The heroes in the fight, 1 to [`MOST`].
    pub heroes: usize,
    /// The run's Heat, 0 to [`MAX_HEAT`].
    pub heat: u8,
    /// The boss's place on the ladder: 0 for the first.
    pub rank: usize,
    /// Whether the run plays the bracket generation ([`BRACKET_TIERS`]).
    pub bracketed: bool,
    /// The run's floor, from 1; a deeper floor changes the boss only in the bracket generation (`docs/delves.md`,
    /// Endless).
    pub floor: u8,
}

impl Scale {
    /// The scaling for `heroes` (clamped to 1..=[`MOST`]) at `heat` (at most [`MAX_HEAT`]) against the boss at `rank`,
    /// before brackets ([`TIERS`]).
    pub fn new(heroes: usize, heat: u8, rank: usize) -> Scale {
        Scale {
            heroes: heroes.clamp(1, MOST),
            heat: heat.min(MAX_HEAT),
            rank,
            bracketed: false,
            floor: 1,
        }
    }

    /// This scale on `floor` (at least 1).
    #[must_use]
    pub fn on_floor(self, floor: u8) -> Scale {
        Scale {
            floor: floor.max(1),
            ..self
        }
    }

    /// Floors below the first that change the boss: 0 on the first floor and before brackets.
    pub fn deeper(self) -> u8 {
        if self.bracketed {
            self.floor.saturating_sub(1)
        } else {
            0
        }
    }

    /// The bracket the shifted table asks for, before the cap: one more per floor below the first.
    fn wanted_bracket(self) -> u8 {
        BRACKETS[usize::from(self.heat)][self.rank.min(RANKS - 1)].saturating_add(self.deeper())
    }

    /// Bracket steps beyond [`MAX_BRACKET`] a deeper floor asks for; each adds life, a card and a brain level instead.
    pub fn overflow(self) -> u8 {
        self.wanted_bracket().saturating_sub(MAX_BRACKET)
    }

    /// [`Scale::new`] for a run of content generation `content`: bracketed from [`BRACKETS_FROM`].
    pub fn of_content(heroes: usize, heat: u8, rank: usize, content: u32) -> Scale {
        Scale {
            bracketed: content >= BRACKETS_FROM,
            ..Scale::new(heroes, heat, rank)
        }
    }

    /// The tier of this rank and Heat. Below the first floor every rank plays the Heat's top row (no early handicap)
    /// at the shifted bracket, capped at [`MAX_BRACKET`]; each step beyond the cap adds [`OVERFLOW_LIFE`] % life, a card
    /// and a brain level.
    pub fn tier(self) -> Tier {
        let table = if self.bracketed {
            &BRACKET_TIERS
        } else {
            &TIERS
        };
        let row = &table[usize::from(self.heat)];
        if self.deeper() == 0 {
            return row[self.rank.min(RANKS - 1)];
        }
        let top = row[RANKS - 1];
        let over = self.overflow();
        Tier {
            bracket: self.wanted_bracket().min(MAX_BRACKET),
            life: top.life + OVERFLOW_LIFE * i32::from(over),
            hand: top.hand + i32::from(over),
            brain: top
                .brain
                .saturating_add(i8::try_from(over).unwrap_or(i8::MAX)),
            ..top
        }
    }

    /// The bracket of the boss's deck: 0 for the deck as written before brackets.
    pub fn bracket(self) -> u8 {
        self.tier().bracket
    }

    /// The boss's deck at this scale.
    pub fn deck(self, boss: &Boss) -> Vec<(String, u32)> {
        boss.deck_at(self.bracket())
    }

    fn extra_heroes(self) -> i32 {
        i32::try_from(self.heroes - 1).unwrap_or(0)
    }

    /// `life` at the tier's share, at least 1.
    fn tiered(self, life: i32) -> i32 {
        (life * self.tier().life / 100).max(1)
    }

    /// The boss's starting life: its base and more for each hero after the first, scaled by party size and the tier.
    pub fn life(self, boss: &Boss) -> i32 {
        let base = boss.life + boss.life_per_hero * self.extra_heroes();
        self.tiered(base * LIFE[self.heroes] / 100)
    }

    /// The boss's opening hand: its base less [`SOLO_HAND`], one card for each hero after the first and the tier's; at
    /// least one card.
    pub fn hand(self, boss: &Boss) -> u32 {
        let base = i64::from(boss.hand.saturating_sub(SOLO_HAND).max(1));
        let hand = base + i64::from(self.extra_heroes()) + i64::from(self.tier().hand);
        u32::try_from(hand.max(1)).unwrap_or(1)
    }

    /// The brain of a boss or minion, moved by the tier's levels.
    pub fn brain(self, brain: Brain) -> Brain {
        let shift = self.tier().brain;
        let step = if shift < 0 {
            Brain::weaker
        } else {
            Brain::stronger
        };
        (0..shift.unsigned_abs()).fold(brain, |b, _| step(b))
    }

    /// The extra combats the boss gets each turn.
    pub fn extra_combats(self) -> usize {
        COMBATS[self.heroes]
    }

    /// The heroes' turn the Enrage begins on at this Heat; Intensify brings it one turn earlier per floor below the
    /// first, never before [`ENRAGE_EARLIEST`].
    pub fn enrage_turn(self) -> u32 {
        let turn = ENRAGE_TURN[usize::from(self.heat)];
        if self.deeper() == 0 {
            return turn;
        }
        turn.saturating_sub(u32::from(self.deeper()))
            .max(ENRAGE_EARLIEST)
    }

    /// Risky relics the boss brings beyond its own: one per floor below the first.
    pub fn extra_relics(self) -> usize {
        usize::from(self.deeper())
    }

    /// What the floor changes, one line each, for the intro: nothing on the first floor.
    pub fn intensify(self) -> Vec<String> {
        let deeper = self.deeper();
        if deeper == 0 {
            return Vec::new();
        }
        let plural = |n: usize| if n == 1 { "" } else { "s" };
        let more = match deeper {
            1 => "once more".to_owned(),
            2 => "twice more".to_owned(),
            n => format!("{n} times more"),
        };
        let relics = self.extra_relics();
        let mut lines = vec![
            format!(
                "Floor {}: bracket {} and the Heat's top tier from the first fight",
                self.floor,
                self.bracket()
            ),
            format!(
                "Intensify: the Enrage begins on the heroes' turn {} and the passive acts {more}",
                self.enrage_turn()
            ),
            format!("{relics} extra risky relic{}", plural(relics)),
        ];
        let over = self.overflow();
        if over > 0 {
            lines.push(format!(
                "Beyond bracket {MAX_BRACKET}: +{} % life, +{over} card{} in hand, a stronger brain",
                OVERFLOW_LIFE * i32::from(over),
                plural(usize::from(over))
            ));
        }
        lines
    }

    /// The custom cards in the boss's command zone from the start: its passive (once more per floor below the first,
    /// Intensify), its Enrage for the Heat, Escalation, by party size Reinforcements, Reserves and Warpath for each
    /// extra combat, and its relic when the tier brings it.
    pub fn rules(self, boss: &Boss) -> Vec<String> {
        let mut rules: Vec<String> = (0..=self.deeper())
            .flat_map(|_| boss.rules.iter().cloned())
            .collect();
        if !boss.enrage.is_empty() {
            rules.push(enrage_id(&boss.enrage, self.enrage_turn()));
        }
        rules.push(ESCALATION.to_owned());
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
        if self.tier().relic && !boss.relic.is_empty() {
            rules.push(boss.relic.clone());
        }
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

    /// How many minions the boss fields: the party-size table's, moved by the tier.
    pub fn minion_count(self) -> usize {
        let count =
            i64::try_from(MINIONS[self.heroes]).unwrap_or(0) + i64::from(self.tier().minions);
        usize::try_from(count.max(0)).unwrap_or(0)
    }

    /// The minions the boss fields: the theme's first ones that do not play the boss's own deck.
    pub fn minions<'a>(self, theme: &'a Theme, boss: &Boss) -> Vec<&'a Minion> {
        theme
            .minions
            .iter()
            .filter(|m| m.deck_of != boss.identity())
            .take(self.minion_count())
            .collect()
    }

    /// A minion's starting life at the tier's share.
    pub fn minion_life(self, minion: &Minion) -> i32 {
        self.tiered(minion.life)
    }
}
