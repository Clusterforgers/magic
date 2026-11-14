//! The spotlight, a campaign rule that spreads a boss's pressure over the party: every hero gathers attention as the
//! boss's side hurts them (damage, life loss, permanents removed), the boss eyes the hero whose presence on the table
//! minus attention is highest, a hero past their share of the recent pressure is left alone, and no hero falls to the
//! boss's side before every hero has had three turns. The lull opens a fight: the boss's side attacks and sets schemes
//! in motion from the boss's third turn (`docs/delves.md`, Spotlight and ramp-up).

use std::collections::BTreeMap;

use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::combat::DamageTarget;
use crate::events::Event;
use crate::object::ObjectKind;
use crate::state::GameState;
use crate::zones::Zone;

/// The fewest rounds of the table the share of pressure is measured over.
pub const WINDOW_ROUNDS: u32 = 3;
/// The attention a removed nontoken permanent adds; a token adds 1.
pub const REMOVED: u32 = 3;
/// The pressure in the window below which no hero is capped.
pub const LEAST_PRESSURE: u32 = 6;
/// The turns every hero has before the boss's side can make one fall.
pub const GRACE_TURNS: u32 = 3;
/// Above this ratio of the next hero's presence a hero is an overwhelming threat, eyed whatever their attention.
pub const OVERWHELMING: i64 = 2;
/// The least presence of an overwhelming threat.
pub const OVERWHELMING_LEAST: i64 = 15;
/// The boss's turn from which its side's creatures can attack and its schemes are set in motion: the lull before it
/// gives the heroes their opening turns.
pub const LULL_TURNS: u32 = 3;

/// What the boss's side did to each hero: the pressure of each turn, kept for [`WINDOW_ROUNDS`], and the whole fight's.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Spotlight {
    /// (turn number, hero, pressure) in the window.
    recent: Vec<(u32, PlayerId, u32)>,
    /// The pressure each hero received this fight.
    total: BTreeMap<PlayerId, u32>,
}

impl Spotlight {
    /// Whether nothing was recorded.
    pub fn is_empty(&self) -> bool {
        self.total.is_empty()
    }
}

/// Whether `player` is a hero of a campaign fight: a seat that becomes a Spirit.
pub fn is_hero(state: &GameState, player: PlayerId) -> bool {
    state
        .player(player)
        .is_some_and(|p| p.spirit_form.is_some())
}

/// Whether `player` is on a boss's side: a seat with phases or schemes, or a minion with a leader.
pub fn is_boss_side(state: &GameState, player: PlayerId) -> bool {
    state
        .player(player)
        .is_some_and(|p| !p.phases.is_empty() || !p.schemes.is_empty() || p.leader.is_some())
}

/// Whether the game is a campaign fight with a spotlight: it has heroes and a boss's side.
pub fn active(state: &GameState) -> bool {
    let players = state.players();
    players.iter().any(|p| p.spirit_form.is_some())
        && players.iter().any(|p| is_boss_side(state, p.id()))
}

/// The heroes still standing (in the game, no Spirit), in seat order.
pub fn standing(state: &GameState) -> Vec<PlayerId> {
    state
        .players()
        .iter()
        .filter(|p| p.spirit_form.is_some() && p.is_in_game() && !p.spirit)
        .map(crate::player::Player::id)
        .collect()
}

/// The first turn number inside the window: the last [`WINDOW_ROUNDS`] rounds of the table, or one round for each
/// hero when there are more, so every hero can take a turn in the spotlight before the first one's attention fades.
fn window_start(state: &GameState) -> u32 {
    let seats = u32::try_from(state.players().len()).unwrap_or(u32::MAX);
    let heroes = state
        .players()
        .iter()
        .filter(|p| p.spirit_form.is_some())
        .count();
    let rounds = WINDOW_ROUNDS.max(u32::try_from(heroes).unwrap_or(u32::MAX));
    state
        .turn
        .number
        .saturating_sub(rounds.saturating_mul(seats))
        .saturating_add(1)
}

/// The pressure `hero` received from the boss's side in the window.
pub fn attention(state: &GameState, hero: PlayerId) -> u32 {
    let from = window_start(state);
    state
        .spotlight
        .recent
        .iter()
        .filter(|(turn, who, _)| *who == hero && *turn >= from)
        .map(|(_, _, points)| *points)
        .fold(0, u32::saturating_add)
}

/// The pressure `hero` received from the boss's side this fight.
pub fn received(state: &GameState, hero: PlayerId) -> u32 {
    state.spotlight.total.get(&hero).copied().unwrap_or(0)
}

/// The share of the window's pressure one hero may take: 60 % with two heroes standing, 40 % with more.
pub fn cap_percent(standing: usize) -> u32 {
    if standing <= 2 { 60 } else { 40 }
}

/// Whether `hero` took more than their share of the window's pressure while another hero stands: the boss's side
/// leaves them alone unless they are an overwhelming threat.
pub fn capped(state: &GameState, hero: PlayerId) -> bool {
    let heroes = standing(state);
    if heroes.len() < 2 || !heroes.contains(&hero) {
        return false;
    }
    let total: u32 = heroes
        .iter()
        .map(|h| attention(state, *h))
        .fold(0, u32::saturating_add);
    total >= LEAST_PRESSURE
        && attention(state, hero).saturating_mul(100)
            > total.saturating_mul(cap_percent(heroes.len()))
}

/// A hero's presence on the table as the boss reads it: their creatures' power, 2 for each other nonland permanent
/// and the cards in their hand.
pub fn presence(state: &GameState, hero: PlayerId) -> i64 {
    presences(state, &[hero])[0]
}

/// The [`presence`] of each of `heroes`, read in one pass over the battlefield.
fn presences(state: &GameState, heroes: &[PlayerId]) -> Vec<i64> {
    let mut found: Vec<i64> = heroes
        .iter()
        .map(|hero| i64::try_from(state.zone(Zone::Hand(*hero)).len()).unwrap_or(0))
        .collect();
    for id in state.zone(Zone::Battlefield) {
        let Some(at) = crate::characteristics::controller_of(state, *id)
            .and_then(|c| heroes.iter().position(|h| *h == c))
        else {
            continue;
        };
        let Some(chars) = crate::characteristics::characteristics(state, *id) else {
            continue;
        };
        if chars.types.contains(CardType::Creature) {
            found[at] += i64::from(chars.power.unwrap_or(0).max(0));
        } else if !chars.types.contains(CardType::Land) {
            found[at] += 2;
        }
    }
    found
}

/// Whether the presence at `at` of `presences` is overwhelming beside the others.
fn towers(presences: &[i64], at: usize) -> bool {
    let own = presences[at];
    own >= OVERWHELMING_LEAST
        && presences
            .iter()
            .enumerate()
            .all(|(other, p)| other == at || own >= OVERWHELMING * p)
}

/// Whether `hero` is an overwhelming threat: their presence is at least [`OVERWHELMING`] times every other standing
/// hero's and at least [`OVERWHELMING_LEAST`].
pub fn overwhelming(state: &GameState, hero: PlayerId) -> bool {
    let heroes = standing(state);
    let Some(at) = heroes.iter().position(|h| *h == hero) else {
        return false;
    };
    towers(&presences(state, &heroes), at)
}

/// The standing heroes in the order the boss's side eyes them: those not capped (or overwhelming) first, then by
/// presence minus attention, ties going round the table with the boss's turns.
pub fn ranking(state: &GameState) -> Vec<PlayerId> {
    let heroes = standing(state);
    let count = heroes.len().max(1);
    let rotation = usize::try_from(boss_turns(state)).unwrap_or(0) % count;
    let presences = presences(state, &heroes);
    let attentions: Vec<u32> = heroes.iter().map(|h| attention(state, *h)).collect();
    let total = attentions.iter().copied().fold(0, u32::saturating_add);
    let cap = cap_percent(heroes.len());
    let mut keyed: Vec<(bool, i64, usize, PlayerId)> = heroes
        .iter()
        .enumerate()
        .map(|(at, hero)| {
            let capped = heroes.len() >= 2
                && total >= LEAST_PRESSURE
                && attentions[at].saturating_mul(100) > total.saturating_mul(cap);
            let held = capped && !towers(&presences, at);
            let score = presences[at] - i64::from(attentions[at]);
            (held, -score, (at + count - rotation) % count, *hero)
        })
        .collect();
    keyed.sort();
    keyed.into_iter().map(|(_, _, _, hero)| hero).collect()
}

/// The hero `seat` of the boss's side eyes next: the boss the first of the [`ranking`], each minion the next, so the
/// boss's side splits its targets; `None` outside a campaign fight or with no hero standing.
pub fn focus(state: &GameState, seat: PlayerId) -> Option<PlayerId> {
    if !is_boss_side(state, seat) {
        return None;
    }
    let ranking = ranking(state);
    if ranking.is_empty() {
        return None;
    }
    let minion_at = state
        .players()
        .iter()
        .filter(|p| p.leader.is_some())
        .position(|p| p.id() == seat)
        .map_or(0, |at| at + 1);
    ranking.get(minion_at % ranking.len()).copied()
}

/// How many turns the boss (the seat with phases) has begun; 0 outside a campaign fight.
pub fn boss_turns(state: &GameState) -> u32 {
    state
        .players()
        .iter()
        .filter(|p| !p.phases.is_empty())
        .map(|p| state.rmt8_turns_taken(p.id()))
        .max()
        .unwrap_or(0)
}

/// Whether the lull holds for `player`: a seat of a boss's side before the boss's [`LULL_TURNS`]th turn, whose
/// creatures can't attack and whose schemes wait.
pub fn lull(state: &GameState, player: PlayerId) -> bool {
    is_boss_side(state, player) && active(state) && boss_turns(state) < LULL_TURNS
}

/// Whether the lull still holds for the turn `player` is about to begin, read before it begins (schemes).
pub fn lull_ahead(state: &GameState, player: PlayerId) -> bool {
    is_boss_side(state, player) && active(state) && boss_turns(state).saturating_add(1) < LULL_TURNS
}

/// Whether the boss's side can't yet make a hero fall: some hero still standing has finished fewer than
/// [`GRACE_TURNS`] turns.
pub fn in_grace(state: &GameState) -> bool {
    active(state)
        && standing(state).into_iter().any(|hero| {
            let begun = state.rmt8_turns_taken(hero);
            let finished = if state.turn.active == hero {
                begun.saturating_sub(1)
            } else {
                begun
            };
            finished < GRACE_TURNS
        })
}

/// The life `hero` loses of `wanted` from the boss's side: during the grace no more than leaves them at 1.
pub(crate) fn graced_loss(state: &GameState, hero: PlayerId, wanted: i32, life: i32) -> i32 {
    if is_hero(state, hero) && in_grace(state) && life >= 1 {
        wanted.min(life - 1)
    } else {
        wanted
    }
}

/// The life `player` at `life` loses of `wanted` damage from `source`, read before the damage folds: the grace holds
/// damage from the boss's side.
pub fn graced_damage(
    state: &GameState,
    source: ObjectId,
    player: PlayerId,
    wanted: i32,
    life: i32,
) -> i32 {
    if boss_source(state, source) {
        graced_loss(state, player, wanted, life)
    } else {
        wanted
    }
}

/// The life `player` at `life` loses of a `LifeLost` of `wanted`, read before it folds: the grace holds life lost to
/// the boss's side's spells and abilities.
pub fn graced_life_loss(state: &GameState, player: PlayerId, wanted: i32, life: i32) -> i32 {
    if boss_resolving(state) {
        graced_loss(state, player, wanted, life)
    } else {
        wanted
    }
}

/// Whether the object `id` is controlled by the boss's side.
fn boss_controls(state: &GameState, id: ObjectId) -> bool {
    crate::characteristics::controller_of(state, id).is_some_and(|p| is_boss_side(state, p))
}

/// Whether the spell or ability resolving now is the boss's side's.
pub(crate) fn boss_resolving(state: &GameState) -> bool {
    state
        .resolving
        .as_ref()
        .is_some_and(|r| boss_controls(state, r.object))
}

/// Whether damage from `source` is the boss's side's.
pub(crate) fn boss_source(state: &GameState, source: ObjectId) -> bool {
    boss_controls(state, source)
}

/// Records the pressure `event` puts on a hero, read before it is folded: damage from the boss's side, life lost to a
/// spell or ability of theirs, a permanent of the hero's that one of theirs removed.
pub(crate) fn note(state: &mut GameState, event: &Event) {
    if !active(state) {
        return;
    }
    let (hero, points) = match event {
        Event::DamageDealt {
            source,
            target: DamageTarget::Player(hero),
            amount,
            ..
        } if boss_source(state, *source) => (*hero, *amount),
        Event::LifeLost { player, amount } if boss_resolving(state) => (*player, *amount),
        Event::ZoneChanged {
            object,
            from: Zone::Battlefield,
            to,
            ..
        } if *to != Zone::Battlefield && boss_resolving(state) => {
            let Some(owner) = crate::characteristics::controller_of(state, *object) else {
                return;
            };
            let token = state
                .object(*object)
                .is_some_and(|o| o.kind() == ObjectKind::Token);
            (owner, if token { 1 } else { REMOVED })
        }
        Event::TurnBegan { .. } => {
            let from = window_start(state);
            state.spotlight.recent.retain(|(turn, _, _)| *turn >= from);
            return;
        }
        _ => return,
    };
    if points == 0 || !is_hero(state, hero) {
        return;
    }
    let turn = state.turn.number;
    state.spotlight.recent.push((turn, hero, points));
    let total = state.spotlight.total.entry(hero).or_insert(0);
    *total = total.saturating_add(points);
}
