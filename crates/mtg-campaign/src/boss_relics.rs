//! The relics a boss brings: `depth − offset` of them by Heat (at most [`MOST_BOSS_RELICS`]), drawn from the heroes'
//! whole relic pool, risky relics included, never one that names teammates and only ones the boss's deck can use. Each
//! is Rare or Mythic by chance (the odds grow with the count), otherwise Common; copies may repeat and stack. The draw
//! is keyed by floor and room, so a retried fight keeps it.

use crate::content::{Content, CustomCard, Rarity};
use crate::relics::{DeckMakeup, usable};
use crate::rng::Stream;
use crate::scale::MAX_HEAT;

/// The most relics a boss brings.
pub const MOST_BOSS_RELICS: usize = 6;
/// The depth below which a boss brings no relic, by Heat.
pub const RELIC_OFFSET: [u32; MAX_HEAT as usize + 1] = [4, 3, 2];
/// The Rare chance of a boss's relic at Heat 0 to 2 with one relic; each more relic adds 10 points.
pub const RARE_BASE: [u32; MAX_HEAT as usize + 1] = [10, 15, 20];
/// The highest Rare chance.
pub const RARE_CAP: u32 = 60;
/// The relic count from which a Mythic can come (Heat 1 and above), 5 points more for each relic beyond the one before.
pub const MYTHIC_FROM: usize = 4;

/// How many relics a boss at `depth` brings at `heat`.
pub fn count(depth: u32, heat: u8) -> usize {
    let offset = RELIC_OFFSET[usize::from(heat.min(MAX_HEAT))];
    usize::try_from(depth.saturating_sub(offset))
        .unwrap_or(MOST_BOSS_RELICS)
        .min(MOST_BOSS_RELICS)
}

/// The Rare and Mythic chances, in percent, of each relic of a boss that brings `count` relics at `heat`.
pub fn odds(count: usize, heat: u8) -> (u32, u32) {
    let heat = heat.min(MAX_HEAT);
    let more = u32::try_from(count.saturating_sub(1)).unwrap_or(u32::MAX);
    let rare = RARE_BASE[usize::from(heat)]
        .saturating_add(more.saturating_mul(10))
        .min(RARE_CAP);
    let mythic = if heat == 0 || count < MYTHIC_FROM {
        0
    } else {
        5 * u32::try_from(count + 1 - MYTHIC_FROM).unwrap_or(0)
    };
    (if count == 0 { 0 } else { rare }, mythic)
}

/// Whether a boss may bring `relic`: never one that names teammates, and only one its deck (of `makeup`) can use.
pub fn fits(relic: &CustomCard, makeup: &DeckMakeup) -> bool {
    !relic.team && usable(relic, makeup)
}

/// The relics, by id, of the boss in `room` of `floor` at `depth` and `heat`, whose deck has `makeup`, drawn by the
/// run's `seed` from the relics of content generation `content` and earlier.
pub fn draw(
    content: &Content,
    makeup: &DeckMakeup,
    generation: u32,
    (depth, heat): (u32, u8),
    (seed, floor, room): (u64, u8, u8),
) -> Vec<String> {
    let pool: Vec<&CustomCard> = content
        .relics()
        .into_iter()
        .filter(|c| c.since <= generation && fits(c, makeup))
        .collect();
    let n = count(depth, heat);
    let (rare, mythic) = odds(n, heat);
    let mut rng = Stream::new(seed, "boss relics", u64::from(floor), u64::from(room));
    (0..n)
        .filter_map(|_| {
            let roll = u32::try_from(rng.below(100)).unwrap_or(0);
            let mut tier = Some(if roll < mythic {
                Rarity::Mythic
            } else if roll < mythic + rare {
                Rarity::Rare
            } else {
                Rarity::Common
            });
            // A rarity with no fitting relic falls to the one below.
            while let Some(t) = tier {
                let of: Vec<&&CustomCard> = pool.iter().filter(|c| c.rarity == t).collect();
                if let Some(relic) = of.get(rng.below(of.len())) {
                    return Some(relic.id.clone());
                }
                tier = t.lower();
            }
            None
        })
        .collect()
}
