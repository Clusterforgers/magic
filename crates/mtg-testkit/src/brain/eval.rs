//! The board evaluation a brain searches with: one number for how well a position stands for the seat's team, read only
//! from what the seat may see ([`Seen`]).

use std::collections::BTreeMap;

use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;

use super::seen::{Body, Seen};

/// What a decided game is worth: far above any position still in play.
pub const DECIDED: f64 = 1_000_000.0;

/// What a card in hand is worth.
const CARD: f64 = 2.2;

/// Mana sources counted at the higher rate; past them a source is worth [`LATE_SOURCE`].
const RAMP_SOURCES: usize = 6;
/// What each of the first [`RAMP_SOURCES`] mana sources is worth: high, so ramp is played first.
const EARLY_SOURCE: f64 = 3.0;
/// What each mana source past the first [`RAMP_SOURCES`] is worth.
const LATE_SOURCE: f64 = 0.6;

/// What `life` is worth: each point counts more the lower the total, and none is the loss of the game.
pub fn life_value(life: i32) -> f64 {
    if life <= 0 {
        return -40.0;
    }
    9.0 * f64::from(life).sqrt()
}

/// What losing `damage` of `life` costs.
pub fn life_loss(life: i32, damage: i32) -> f64 {
    life_value(life) - life_value(life - damage)
}

/// What a creature is worth: its body, its combat keywords, its other rules and its cost.
pub fn creature_value(body: &Body) -> f64 {
    let power = f64::from(body.power());
    let toughness = f64::from(body.toughness());
    let keys = body.keys;
    let mut value = 1.0 + 1.5 * power + toughness;
    let mut per_power = 0.0;
    if keys.flying {
        per_power += 0.6;
    }
    if keys.double_strike {
        per_power += 1.0;
    } else if keys.first_strike {
        per_power += 0.5;
    }
    if keys.lifelink {
        per_power += 0.5;
    }
    if keys.trample {
        per_power += 0.25;
    }
    if keys.menace {
        per_power += 0.3;
    }
    if keys.vigilance {
        per_power += 0.3;
    }
    if keys.defender || body.cant_attack {
        per_power -= 1.2;
    }
    value += per_power * power;
    if keys.deathtouch {
        value += 2.0;
    }
    if keys.indestructible {
        value += 3.0;
    }
    if keys.hexproof {
        value += 1.0;
    }
    if keys.reach {
        value += 0.3;
    }
    if body.cant_attack && body.cant_block {
        // It only stands there: what is left is its rules.
        return 0.5 + rules_value(body) * 0.5;
    }
    if body.cant_block {
        value -= 0.5 * toughness;
    }
    value + rules_value(body)
}

/// What a permanent's printed rules and cost add: a rough measure of what it does beyond its body.
fn rules_value(body: &Body) -> f64 {
    let rules = u32::try_from(body.rules.min(4)).unwrap_or(4);
    0.5 * f64::from(rules) + 0.3 * f64::from(body.mana_value)
}

/// What any permanent is worth to its controller, its mana aside ([`mana_value_of`]).
pub fn permanent_value(body: &Body) -> f64 {
    if body.is_creature() {
        return creature_value(body);
    }
    if body.is_land() {
        return 0.3;
    }
    if body.chars.types.contains(CardType::Planeswalker) {
        return 2.0 + f64::from(body.loyalty) + 0.4 * f64::from(body.mana_value);
    }
    1.0 + 0.6 * f64::from(body.mana_value)
        + 0.3 * f64::from(u32::try_from(body.rules.min(4)).unwrap_or(4))
}

/// What `sources` mana sources are worth.
pub fn mana_value_of(sources: usize) -> f64 {
    let early = sources.min(RAMP_SOURCES);
    let late = sources.saturating_sub(RAMP_SOURCES);
    EARLY_SOURCE * count(early) + LATE_SOURCE * count(late)
}

/// A small count as a float.
fn count(n: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
}

/// How an evaluation weighs the damage each side's creatures threaten next turn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weights {
    /// The share of the life the seat's team stands to lose that counts against it.
    pub defend: f64,
    /// The share of the life an opposing team stands to lose that counts for the seat.
    pub press: f64,
    /// What a card in hand is worth.
    pub card: f64,
    /// How much life counts against the rest of a team's standing.
    pub life: f64,
    /// How much permanents count against the rest of a team's standing.
    pub board: f64,
}

impl Weights {
    /// The weights of [`evaluate`].
    pub const NORMAL: Weights = Weights {
        defend: 0.5,
        press: 0.25,
        card: CARD,
        life: 1.0,
        board: 1.0,
    };
}

/// The position as `seen`'s seat reads it, from its team's side: its team's standing less the opposing teams', the
/// strongest of them counted half again, and the damage each side's creatures threaten the other with next turn.
pub fn evaluate(seen: &Seen<'_>) -> f64 {
    evaluate_with(seen, Weights::NORMAL)
}

/// [`evaluate`] with the threats of the coming turn weighed by `weights`.
pub fn evaluate_with(seen: &Seen<'_>, weights: Weights) -> f64 {
    if let Some(won) = seen.over() {
        return if won { DECIDED } else { -DECIDED };
    }
    if !seen.is_alive(seen.seat()) && seen.alive().iter().all(|p| !seen.is_teammate(*p)) {
        return -DECIDED;
    }
    let bodies = seen.bodies();
    let teams = team_scores(seen, &bodies, weights);
    let own_team = seen.team_of(seen.seat());
    let mut own = 0.0;
    let mut others = 0.0;
    let mut strongest = f64::MIN;
    for (team, score) in &teams {
        if *team == own_team {
            own = *score;
        } else {
            others += score;
            strongest = strongest.max(*score);
        }
    }
    if strongest > f64::MIN {
        others += 0.5 * strongest;
    }
    own - others + pressure_balance(seen, &bodies, weights)
}

/// Each team still in the game and its standing: life, cards in hand, permanents and mana.
fn team_scores(seen: &Seen<'_>, bodies: &[Body], weights: Weights) -> BTreeMap<u32, f64> {
    let mut scores: BTreeMap<u32, f64> = BTreeMap::new();
    let mut life_counted: Vec<u32> = Vec::new();
    for player in seen.alive() {
        let team = seen.team_of(player);
        let mut score = standing_with(seen, player, bodies, weights);
        // Two-Headed Giant: one life total for the team, counted once.
        if !seen.shares_life() || !life_counted.contains(&team) {
            score +=
                weights.life * life_value(seen.life(player)) - 4.0 * f64::from(seen.poison(player));
            life_counted.push(team);
        }
        *scores.entry(team).or_default() += score;
    }
    scores
}

/// What `player` has apart from life: cards in hand, permanents and mana sources.
pub fn player_standing(seen: &Seen<'_>, player: PlayerId, bodies: &[Body]) -> f64 {
    standing_with(seen, player, bodies, Weights::NORMAL)
}

/// [`player_standing`] with cards and permanents weighed by `weights`.
fn standing_with(seen: &Seen<'_>, player: PlayerId, bodies: &[Body], weights: Weights) -> f64 {
    let hand = seen.hand_size(player).min(8);
    let mut score = weights.card * count(hand);
    if seen.library_size(player) == 0 {
        score -= 30.0;
    }
    let mut sources = 0;
    for body in bodies.iter().filter(|body| body.controller == player) {
        score += weights.board * permanent_value(body);
        if body.is_land() || body.makes_mana {
            sources += 1;
        }
    }
    score + mana_value_of(sources)
}

/// How much `player` threatens the seat's team: their board, their hand and how far ahead they stand.
pub fn threat(seen: &Seen<'_>, player: PlayerId) -> f64 {
    let bodies = seen.bodies();
    let attack: f64 = bodies
        .iter()
        .filter(|body| body.controller == player && body.is_creature())
        .map(creature_value)
        .sum();
    attack + player_standing(seen, player, &bodies) * 0.3 + 0.2 * f64::from(seen.life(player))
}

/// The damage the creatures of `attackers` could deal to `defender` on their next attack, after `defender`'s team blocks
/// one attacker with each creature able to: the biggest attackers are blocked first, a flyer only by a flyer or reach, a
/// menace attacker only by two. A team whose turn it is now blocks with its untapped creatures only (its attackers stay
/// tapped through the other team's turn); otherwise its creatures untap first.
pub fn pressure(seen: &Seen<'_>, bodies: &[Body], attackers: u32, defender: u32) -> i32 {
    let mut attacking: Vec<&Body> = bodies
        .iter()
        .filter(|body| {
            body.is_creature()
                && !body.keys.defender
                && !body.cant_attack
                && seen.team_of(body.controller) == attackers
        })
        .collect();
    attacking.sort_by_key(|body| std::cmp::Reverse(body.power()));
    let defending_now = bodies
        .iter()
        .find(|body| seen.team_of(body.controller) == defender)
        .is_some_and(|body| seen.is_active(body.controller));
    let mut blockers: Vec<&Body> = bodies
        .iter()
        .filter(|body| {
            body.is_creature()
                && !body.cant_block
                && seen.team_of(body.controller) == defender
                && !(defending_now && body.tapped)
        })
        .collect();
    let mut through = 0;
    for attacker in attacking {
        let needed = if attacker.keys.menace { 2 } else { 1 };
        let able: Vec<usize> = blockers
            .iter()
            .enumerate()
            .filter(|(_, blocker)| attacker.blockable_by(blocker))
            .map(|(at, _)| at)
            .take(needed)
            .collect();
        if able.len() == needed {
            for at in able.into_iter().rev() {
                blockers.remove(at);
            }
        } else {
            through += attacker.power() * if attacker.keys.double_strike { 2 } else { 1 };
        }
    }
    through
}

/// The life the seat's team stands to lose to each opposing team's next attack, against what it threatens them with:
/// the losses at their life totals, a lethal threat against the team weighing like most of a lost game.
fn pressure_balance(seen: &Seen<'_>, bodies: &[Body], weights: Weights) -> f64 {
    let own_team = seen.team_of(seen.seat());
    let mut teams: Vec<(u32, i32)> = Vec::new();
    for player in seen.alive() {
        let team = seen.team_of(player);
        if !teams.iter().any(|(t, _)| *t == team) {
            teams.push((team, seen.life(player)));
        }
    }
    let own_life = teams
        .iter()
        .find(|(team, _)| *team == own_team)
        .map_or(0, |(_, life)| *life);
    let mut balance = 0.0;
    for (team, life) in teams.iter().filter(|(team, _)| *team != own_team) {
        let against_us = pressure(seen, bodies, *team, own_team);
        balance -= if against_us >= own_life {
            30.0
        } else {
            weights.defend * life_loss(own_life, against_us)
        };
        let ours = pressure(seen, bodies, own_team, *team);
        balance += if ours >= *life {
            8.0
        } else {
            weights.press * life_loss(*life, ours)
        };
    }
    balance
}

#[cfg(test)]
mod tests {
    //! Goal: the evaluation ranks bodies by size and keywords, prizes the first mana sources, and charges a team for the
    //! unblocked damage it faces.
    use mtg_core::ids::ObjectId;
    use rstest::rstest;

    use super::*;
    use crate::brain::seen::Keys;

    fn body(power: i32, toughness: i32, keys: Keys) -> Body {
        let mut chars = mtg_engine::characteristics::Characteristics {
            types: mtg_core::card::CardTypes::default().with(CardType::Creature),
            subtypes: Vec::new(),
            supertypes: Vec::new(),
            supertypes_removed: Vec::new(),
            creature_types_set: false,
            land_types_set: false,
            all_creature_types: false,
            colors: mtg_core::color::ColorSet::default(),
            power: Some(power),
            toughness: Some(toughness),
            abilities: Vec::new(),
            name_words: Vec::new(),
        };
        chars.power = Some(power);
        Body {
            id: ObjectId(1),
            controller: PlayerId(0),
            chars,
            tapped: false,
            mana_value: 2,
            rules: 0,
            makes_mana: false,
            loyalty: 0,
            keys,
            cant_attack: false,
            cant_block: false,
        }
    }

    #[rstest]
    #[case::bigger_is_better(body(3, 3, Keys::default()), body(2, 2, Keys::default()))]
    #[case::flying_beats_ground(body(2, 2, Keys { flying: true, ..Keys::default() }), body(2, 2, Keys::default()))]
    #[case::deathtouch_counts(body(1, 1, Keys { deathtouch: true, ..Keys::default() }), body(1, 1, Keys::default()))]
    #[case::defender_counts_less(body(2, 2, Keys::default()), body(2, 2, Keys { defender: true, ..Keys::default() }))]
    fn creatures_rank_by_body_and_keywords(#[case] better: Body, #[case] worse: Body) {
        assert!(creature_value(&better) > creature_value(&worse));
    }

    #[rstest]
    fn the_first_mana_sources_are_worth_more_than_later_ones(#[values(1, 3, 5)] sources: usize) {
        let early = mana_value_of(sources + 1) - mana_value_of(sources);
        let late = mana_value_of(sources + 10) - mana_value_of(sources + 9);
        assert!(early > late);
    }

    #[rstest]
    #[case(20, 3)]
    #[case(5, 3)]
    #[case(2, 1)]
    fn a_point_of_life_counts_more_the_lower_the_total(#[case] life: i32, #[case] damage: i32) {
        assert!(life_loss(life, damage) > 0.0);
        assert!(life_loss(life - damage, damage) > life_loss(life, damage));
    }
}
