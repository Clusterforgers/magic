//! The board evaluation a brain searches with: one number for how well a position stands for the seat's team, read only
//! from what the seat may see ([`Seen`]).

use std::collections::BTreeMap;

use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;

use super::seen::{Body, Seen};
use super::threat::{engine_value, held_interaction};

/// The share of its [`super::threat::hold_value`] a piece of interaction in the seat's hand counts for.
const HELD_SHARE: f64 = 1.0;

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

/// What an opponent's `life` is worth at a table of several opponents: [`life_value`]'s slope at 20 life all the way,
/// so a point of damage is worth the same at any opponent and the strongest team's extra weight decides whom to hurt.
pub fn flat_life_value(life: i32) -> f64 {
    if life <= 0 {
        return -40.0;
    }
    let at = 20.0_f64;
    9.0 * at.sqrt() + (f64::from(life) - at) * 4.5 / at.sqrt()
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

/// The weights of the evaluation's terms ([`Terms`]); [`Weights::NORMAL`] is tuned by self-play (`docs/bots.md`).
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
    /// What each of a player's first [`RAMP_SOURCES`] mana sources is worth.
    pub early_source: f64,
    /// What each mana source past those is worth.
    pub late_source: f64,
    /// The share of its hold value interaction in the seat's hand counts for.
    pub held: f64,
    /// How much more the strongest opposing team counts than the others.
    pub strongest: f64,
    /// What a lethal attack threatened against the seat's team costs.
    pub lethal_against: f64,
    /// What a lethal attack the seat's team threatens is worth.
    pub lethal_for: f64,
    /// What each point of a team's creature power adds beyond the bodies.
    pub power: f64,
    /// What each point of evasive (flying, menace, trample) power adds.
    pub evasion: f64,
}

impl Weights {
    /// The hand-set weights before tuning, kept as a yardstick.
    pub const HAND_SET: Weights = Weights {
        defend: 0.5,
        press: 0.25,
        card: CARD,
        life: 1.0,
        board: 1.0,
        early_source: EARLY_SOURCE,
        late_source: LATE_SOURCE,
        held: HELD_SHARE,
        strongest: 0.5,
        lethal_against: 30.0,
        lethal_for: 8.0,
        power: 0.0,
        evasion: 0.0,
    };

    /// The weights of [`evaluate`].
    pub const NORMAL: Weights = Weights::HAND_SET;
}

/// One team's standing as raw sums, before weighting.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TeamTerms {
    /// Cards in hand (up to eight a player).
    pub cards: f64,
    /// Permanents' values with their engine values ([`permanent_value`], [`engine_value`]).
    pub board: f64,
    /// Each player's first mana sources.
    pub early: f64,
    /// The mana sources past those.
    pub late: f64,
    /// [`life_value`] of the team's life (once for a shared total).
    pub life: f64,
    /// Fixed terms: an empty library, poison counters.
    pub fixed: f64,
    /// Creature power.
    pub power: f64,
    /// Evasive creature power.
    pub evasion: f64,
}

impl TeamTerms {
    /// The weighted standing.
    pub fn score(&self, w: &Weights) -> f64 {
        w.card * self.cards
            + w.board * self.board
            + w.early_source * self.early
            + w.late_source * self.late
            + w.life * self.life
            + self.fixed
            + w.power * self.power
            + w.evasion * self.evasion
    }
}

/// The next attacks between the seat's team and one opposing team.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Threats {
    /// What the life the seat's team stands to lose is worth, unless lethal.
    pub against: f64,
    /// Whether that attack is lethal.
    pub against_lethal: bool,
    /// What the life the opposing team stands to lose is worth, unless lethal.
    pub ours: f64,
    /// Whether that attack is lethal.
    pub ours_lethal: bool,
}

/// A position's evaluation as raw terms: [`Terms::score`] with [`Weights`] is [`evaluate_with`], so the weights can be
/// fitted to game results (Texel tuning, `examples/eval_tune.rs`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Terms {
    /// A decided game: `Some(true)` won, `Some(false)` lost.
    pub decided: Option<bool>,
    /// The seat's team.
    pub own: TeamTerms,
    /// Each opposing team still in the game.
    pub others: Vec<TeamTerms>,
    /// Interaction held in the seat's hand at its full hold value.
    pub held: f64,
    /// The next attacks with each opposing team.
    pub threats: Vec<Threats>,
}

impl Terms {
    /// The evaluation with `w`.
    pub fn score(&self, w: &Weights) -> f64 {
        if let Some(won) = self.decided {
            return if won { DECIDED } else { -DECIDED };
        }
        let mut others = 0.0;
        let mut strongest = f64::MIN;
        for team in &self.others {
            let score = team.score(w);
            others += score;
            strongest = strongest.max(score);
        }
        if strongest > f64::MIN {
            others += w.strongest * strongest;
        }
        let mut balance = 0.0;
        for threat in &self.threats {
            balance -= if threat.against_lethal {
                w.lethal_against
            } else {
                w.defend * threat.against
            };
            balance += if threat.ours_lethal {
                w.lethal_for
            } else {
                w.press * threat.ours
            };
        }
        self.own.score(w) + w.held * self.held - others + balance
    }
}

/// The position as `seen`'s seat reads it, from its team's side: its team's standing (engines and combo pieces counted
/// by the threat model, interaction in the seat's hand counted for what keeping it is worth) less the opposing
/// teams', the strongest of them counted half again, and the damage each side's creatures threaten the other with next
/// turn.
pub fn evaluate(seen: &Seen<'_>) -> f64 {
    evaluate_with(seen, Weights::NORMAL)
}

/// [`evaluate`] with `weights`.
pub fn evaluate_with(seen: &Seen<'_>, weights: Weights) -> f64 {
    terms(seen).score(&weights)
}

/// The raw terms of the position as `seen`'s seat reads it ([`Terms`]).
pub fn terms(seen: &Seen<'_>) -> Terms {
    if let Some(won) = seen.over() {
        return Terms {
            decided: Some(won),
            ..Terms::default()
        };
    }
    if !seen.is_alive(seen.seat()) && seen.alive().iter().all(|p| !seen.is_teammate(*p)) {
        return Terms {
            decided: Some(false),
            ..Terms::default()
        };
    }
    let bodies = seen.bodies();
    let own_team = seen.team_of(seen.seat());
    let mut own = TeamTerms::default();
    let mut others = Vec::new();
    for (team, terms) in team_terms(seen, &bodies) {
        if team == own_team {
            own = terms;
        } else {
            others.push(terms);
        }
    }
    Terms {
        decided: None,
        own,
        others,
        held: held_interaction(seen, 1.0),
        threats: threats(seen, &bodies),
    }
}

/// Each team still in the game and its standing's terms.
fn team_terms(seen: &Seen<'_>, bodies: &[Body]) -> BTreeMap<u32, TeamTerms> {
    let own_team = seen.team_of(seen.seat());
    let flat = super::knobs::knobs().archenemy > 0.0 && super::threat::many_opponents(seen);
    let mut teams: BTreeMap<u32, TeamTerms> = BTreeMap::new();
    let mut life_counted: Vec<u32> = Vec::new();
    for player in seen.alive() {
        let team = seen.team_of(player);
        let standing = teams.entry(team).or_default();
        add_standing(seen, player, bodies, standing);
        // Two-Headed Giant: one life total for the team, counted once.
        if !seen.shares_life() || !life_counted.contains(&team) {
            standing.life += if flat && team != own_team {
                flat_life_value(seen.life(player))
            } else {
                life_value(seen.life(player))
            };
            standing.fixed -= 4.0 * f64::from(seen.poison(player));
            life_counted.push(team);
        }
    }
    teams
}

/// What `player` has apart from life: cards in hand, permanents and mana sources.
pub fn player_standing(seen: &Seen<'_>, player: PlayerId, bodies: &[Body]) -> f64 {
    let mut terms = TeamTerms::default();
    add_standing(seen, player, bodies, &mut terms);
    terms.score(&Weights::NORMAL)
}

/// Adds `player`'s cards in hand, permanents and mana sources to `terms`.
fn add_standing(seen: &Seen<'_>, player: PlayerId, bodies: &[Body], terms: &mut TeamTerms) {
    terms.cards += count(seen.hand_size(player).min(8));
    if seen.library_size(player) == 0 {
        terms.fixed -= 30.0;
    }
    let mut sources = 0;
    for body in bodies.iter().filter(|body| body.controller == player) {
        terms.board += permanent_value(body) + engine_value(seen, body, bodies);
        if body.is_land() || body.makes_mana {
            sources += 1;
        }
        if body.is_creature() && !body.keys.defender && !body.cant_attack {
            let power = f64::from(body.power().max(0));
            terms.power += power;
            if body.keys.flying || body.keys.menace || body.keys.trample {
                terms.evasion += power;
            }
        }
    }
    terms.early += count(sources.min(RAMP_SOURCES));
    terms.late += count(sources.saturating_sub(RAMP_SOURCES));
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

/// The life the seat's team stands to lose to each opposing team's next attack, and what it threatens them with: the
/// losses at their life totals, or whether the attack is lethal.
fn threats(seen: &Seen<'_>, bodies: &[Body]) -> Vec<Threats> {
    let own_team = seen.team_of(seen.seat());
    let flat = super::knobs::knobs().archenemy > 0.0 && super::threat::many_opponents(seen);
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
    teams
        .iter()
        .filter(|(team, _)| *team != own_team)
        .map(|(team, life)| {
            let against_us = pressure(seen, bodies, *team, own_team);
            let ours = pressure(seen, bodies, own_team, *team);
            Threats {
                against: life_loss(own_life, against_us),
                against_lethal: against_us >= own_life,
                ours: if flat {
                    flat_life_value(*life) - flat_life_value(*life - ours)
                } else {
                    life_loss(*life, ours)
                },
                ours_lethal: ours >= *life,
            }
        })
        .collect()
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
