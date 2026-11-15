//! The spotlight's binding focus: each seat of the boss's side aims its attacks and targeted effects at the hero it eyes
//! and leaves that hero only for a reason; the log names it ([`Event::FocusShifted`], `docs/delves.md`, Spotlight).

use std::cmp::Reverse;

use mtg_cards::keyword::Keyword;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::characteristics::{characteristics, controller_of, sidelined};
use crate::combat::{AttackTarget, defending_player};
use crate::decision::AttackOptions;
use crate::events::Event;
use crate::keywords::keywords_of;
use crate::spotlight;
use crate::state::GameState;
use crate::zones::Zone;

/// Why a seat of the boss's side aims at another hero than the one it eyes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum AimWhy {
    /// The eyed hero can't be attacked or targeted: no legal way, protection, hexproof, or gone.
    Unreachable,
    /// The hero it aims at is an overwhelming threat ([`spotlight::overwhelming`]).
    Overwhelming,
    /// The hero it aims at has creatures whose power reaches the life of the boss.
    Deadly {
        /// The boss they could kill.
        boss: PlayerId,
    },
    /// Every attacker that could go at the eyed hero would be blocked and killed without dealing damage.
    Futile,
    /// The attack kills the hero it aims at.
    Lethal,
}

/// The hero a seat aims at, and why that is not the one it eyes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aim {
    /// The hero the seat aims at.
    pub hero: PlayerId,
    /// The hero the seat eyes ([`spotlight::focus`]).
    pub eyed: PlayerId,
    /// Why `hero` is not `eyed`; `None` while they are the same.
    pub why: Option<AimWhy>,
}

/// The boss a hero could kill: the boss seat in the game with the least life, when the power of the hero's creatures
/// reaches it.
pub fn deadly(state: &GameState, hero: PlayerId) -> Option<PlayerId> {
    let (boss, life) = state
        .players()
        .iter()
        .filter(|p| p.phases().0 > 0 && p.is_in_game())
        .map(|p| (p.id(), p.life()))
        .min_by_key(|(_, life)| *life)?;
    let power: i32 = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| controller_of(state, **id) == Some(hero))
        .filter_map(|id| characteristics(state, *id))
        .filter(crate::characteristics::Characteristics::is_creature)
        .map(|chars| chars.power.unwrap_or(0).max(0))
        .sum();
    (power >= life).then_some(boss)
}

/// What makes `hero` a threat the boss's side turns to whatever it eyes.
fn threat(state: &GameState, hero: PlayerId) -> Option<AimWhy> {
    if spotlight::overwhelming(state, hero) {
        Some(AimWhy::Overwhelming)
    } else {
        deadly(state, hero).map(|boss| AimWhy::Deadly { boss })
    }
}

/// The aim among the heroes `reach` (in the order the boss's side eyes them): the eyed hero unless it is out of reach,
/// another hero's attack kills (`lethal`), another hero is a threat while the eyed one is none, or an attack at the eyed
/// hero is `futile` and another's is not. `None` while no hero is in reach.
fn pick(
    state: &GameState,
    seat: PlayerId,
    reach: &[PlayerId],
    lethal: impl Fn(PlayerId) -> bool,
    futile: impl Fn(PlayerId) -> bool,
) -> Option<Aim> {
    let eyed = spotlight::focus(state, seat)?;
    let first = *reach.first()?;
    let aim = |hero, why| {
        Some(Aim {
            hero,
            eyed,
            why: Some(why),
        })
    };
    let keep = Some(Aim {
        hero: eyed,
        eyed,
        why: None,
    });
    let others = reach.iter().copied().filter(|hero| *hero != eyed);
    let kill = reach.iter().copied().find(|hero| lethal(*hero));
    let big = others
        .clone()
        .find_map(|hero| threat(state, hero).map(|why| (hero, why)));
    if !reach.contains(&eyed) {
        let hero = kill
            .or(big.map(|(hero, _)| hero))
            .or_else(|| reach.iter().copied().find(|hero| !futile(*hero)))
            .unwrap_or(first);
        return aim(hero, AimWhy::Unreachable);
    }
    if lethal(eyed) {
        return keep;
    }
    if let Some(hero) = kill {
        return aim(hero, AimWhy::Lethal);
    }
    if threat(state, eyed).is_none()
        && let Some((hero, why)) = big
    {
        return aim(hero, why);
    }
    if futile(eyed)
        && let Some(hero) = others.clone().find(|hero| !futile(*hero))
    {
        return aim(hero, AimWhy::Futile);
    }
    keep
}

/// A creature as combat reads it: independent facts read off its characteristics and keywords.
#[allow(clippy::struct_excessive_bools)]
struct Fighter {
    id: ObjectId,
    power: i32,
    toughness: i32,
    flying: bool,
    reach: bool,
    first_strike: bool,
    double_strike: bool,
    deathtouch: bool,
    indestructible: bool,
    menace: bool,
}

impl Fighter {
    fn of(state: &GameState, id: ObjectId) -> Option<Fighter> {
        let chars = characteristics(state, id)
            .filter(crate::characteristics::Characteristics::is_creature)?;
        let mut fighter = Fighter {
            id,
            power: chars.power.unwrap_or(0).max(0),
            toughness: chars.toughness.unwrap_or(0).max(0),
            flying: false,
            reach: false,
            first_strike: false,
            double_strike: false,
            deathtouch: false,
            indestructible: false,
            menace: false,
        };
        for keyword in keywords_of(state, id) {
            match keyword {
                Keyword::Flying => fighter.flying = true,
                Keyword::Reach => fighter.reach = true,
                Keyword::FirstStrike => fighter.first_strike = true,
                Keyword::DoubleStrike => fighter.double_strike = true,
                Keyword::Deathtouch => fighter.deathtouch = true,
                Keyword::Indestructible => fighter.indestructible = true,
                Keyword::Menace => fighter.menace = true,
                _ => {}
            }
        }
        Some(fighter)
    }

    /// Whether it kills `other` in a fight between the two.
    fn kills(&self, other: &Fighter) -> bool {
        !other.indestructible
            && self.power > 0
            && (self.deathtouch || self.power >= other.toughness)
    }

    /// Whether it, blocking `attacker`, kills it and lives (or strikes first).
    fn punishes(&self, attacker: &Fighter) -> bool {
        let strikes_first = |f: &Fighter| f.first_strike || f.double_strike;
        (!attacker.flying || self.flying || self.reach)
            && self.kills(attacker)
            && (strikes_first(self) && !strikes_first(attacker) || !attacker.kills(self))
    }
}

/// How attackers fare against the blockers of a hero: the damage that gets through, and whether every attacker is
/// blocked and killed. The biggest attackers pick first; a blocker that kills its attacker and lives is preferred; a
/// flyer needs a flyer or reach and a menace attacker two blockers.
struct Clash {
    through: i32,
    futile: bool,
}

fn clash(attackers: &[Fighter], blockers: &[Fighter]) -> Clash {
    let mut order: Vec<&Fighter> = attackers.iter().collect();
    order.sort_by_key(|attacker| (Reverse(attacker.power), attacker.id));
    let mut free: Vec<&Fighter> = blockers.iter().collect();
    let mut result = Clash {
        through: 0,
        futile: !order.is_empty(),
    };
    for attacker in order {
        let needed = if attacker.menace { 2 } else { 1 };
        let mut able: Vec<usize> = (0..free.len())
            .filter(|at| !attacker.flying || free[*at].flying || free[*at].reach)
            .collect();
        able.sort_by_key(|at| !free[*at].punishes(attacker));
        able.truncate(needed);
        if able.len() < needed {
            result.through += attacker.power * if attacker.double_strike { 2 } else { 1 };
            result.futile = false;
            continue;
        }
        if !able.iter().any(|at| free[*at].punishes(attacker)) {
            result.futile = false;
        }
        able.sort_unstable_by(|a, b| b.cmp(a));
        for at in able {
            free.remove(at);
        }
    }
    result
}

/// The untapped creatures `hero` can block with.
fn blockers_of(state: &GameState, hero: PlayerId) -> Vec<Fighter> {
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| controller_of(state, **id) == Some(hero))
        .filter(|id| {
            state
                .object(**id)
                .and_then(|o| o.permanent())
                .is_some_and(|p| !p.tapped())
                && !sidelined(state, **id).1
        })
        .filter_map(|id| Fighter::of(state, *id))
        .collect()
}

/// The attackers of `options` that may attack `hero`.
fn attackers_at(state: &GameState, options: &AttackOptions, hero: PlayerId) -> Vec<Fighter> {
    options
        .attackers
        .iter()
        .filter(|option| {
            option
                .defenders
                .iter()
                .any(|target| defending_player(state, *target) == Some(hero))
        })
        .filter_map(|option| Fighter::of(state, option.object))
        .collect()
}

/// The hero the attacks of `options` by `seat` go at, or `None` outside the boss's side and while no hero can be
/// attacked. Each of the other cases names its reason:
///
/// - the eyed hero can't be attacked;
/// - another hero is an overwhelming threat, or its creatures could kill the boss, and the eyed hero is neither;
/// - an attack at the eyed hero is futile (each attacker is blocked and killed, none gets damage through) and an attack
///   at another hero is not;
/// - the attack kills another hero (and not the eyed one): the kill is always taken.
pub fn attack_aim(state: &GameState, seat: PlayerId, options: &AttackOptions) -> Option<Aim> {
    if !spotlight::active(state) || !spotlight::is_boss_side(state, seat) {
        return None;
    }
    let clashes: Vec<(PlayerId, Clash)> = spotlight::ranking(state)
        .into_iter()
        .filter_map(|hero| {
            let attackers = attackers_at(state, options, hero);
            (!attackers.is_empty()).then(|| (hero, clash(&attackers, &blockers_of(state, hero))))
        })
        .collect();
    let reach: Vec<PlayerId> = clashes.iter().map(|(hero, _)| *hero).collect();
    let found = |hero: PlayerId| clashes.iter().find(|(h, _)| *h == hero).map(|(_, c)| c);
    pick(
        state,
        seat,
        &reach,
        |hero| {
            let life = state
                .player(hero)
                .map_or(i32::MAX, crate::player::Player::life);
            !spotlight::in_grace(state) && found(hero).is_some_and(|c| c.through >= life)
        },
        |hero| found(hero).is_some_and(|c| c.futile),
    )
}

/// The hero a targeted spell or ability of `seat` goes at among `offered`, the heroes it can target (a hero, or a
/// permanent of theirs): the eyed hero unless it is not offered, or another is a threat while the eyed one is none.
pub fn target_aim(state: &GameState, seat: PlayerId, offered: &[PlayerId]) -> Option<Aim> {
    if !spotlight::active(state) || !spotlight::is_boss_side(state, seat) {
        return None;
    }
    let reach: Vec<PlayerId> = spotlight::ranking(state)
        .into_iter()
        .filter(|hero| offered.contains(hero))
        .collect();
    pick(state, seat, &reach, |_| false, |_| false)
}

/// The log's note that the attack `seat` declares goes at a hero other than the one it eyes, with the reason; `None`
/// when it goes at the eyed hero or the engine finds no reason.
pub fn shifted(
    state: &GameState,
    seat: PlayerId,
    options: &AttackOptions,
    attackers: &[(ObjectId, AttackTarget)],
) -> Option<Event> {
    let aim = attack_aim(state, seat, options)?;
    let (object, instead) = attackers.iter().find_map(|(object, target)| {
        let hero = defending_player(state, *target)?;
        (hero != aim.eyed && spotlight::is_hero(state, hero)).then_some((*object, hero))
    })?;
    let cannot = || {
        options
            .attackers
            .iter()
            .find(|option| option.object == object)
            .is_some_and(|option| {
                !option
                    .defenders
                    .iter()
                    .any(|target| defending_player(state, *target) == Some(aim.eyed))
            })
            .then_some(AimWhy::Unreachable)
    };
    let why = aim.why.filter(|_| aim.hero == instead).or_else(cannot)?;
    Some(Event::FocusShifted {
        player: seat,
        eyed: aim.eyed,
        instead,
        why,
    })
}

/// `outcome` with the note ahead of the events it folds, when it folds events.
pub(crate) fn noted(
    outcome: crate::answers::Outcome,
    note: Option<Event>,
) -> crate::answers::Outcome {
    match (outcome, note) {
        (crate::answers::Outcome::Events(mut events), Some(note)) => {
            events.insert(0, note);
            crate::answers::Outcome::Events(events)
        }
        (outcome, _) => outcome,
    }
}
