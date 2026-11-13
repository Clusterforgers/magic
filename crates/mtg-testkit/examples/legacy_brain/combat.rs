//! Attacks and blocks: blocks chosen greedily by the engine's combat forecast, quick attacks by what the battlefield
//! shows, and planned attacks by simulating each candidate through the defenders' likely blocks.

use std::collections::BTreeMap;

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::Engine;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, AttackOptions, BlockOptions, DamageOptions};
use mtg_engine::forecast::CombatForecast;

use super::eval::{creature_value, life_value, pressure};
use super::seen::{Body, Seen};
use super::sim::{Horizon, Search};
use super::threat;

/// A declaration of attackers.
pub type Attack = Vec<(ObjectId, AttackTarget)>;
/// A declaration of blockers.
pub type Blocks = Vec<(ObjectId, ObjectId)>;

/// What a forecast lethal hit on the seat's team costs a block.
const LETHAL_HIT: f64 = 500.0;

/// Combat damage divided as the rules ask for least: each blocker in order gets lethal damage while the power lasts;
/// what is left goes past them with trample, else to the first blocker.
pub fn assign_lethal(options: &DamageOptions) -> Vec<(ObjectId, u32)> {
    let mut left = options.power;
    let mut shares: Vec<(ObjectId, u32)> = Vec::new();
    for (blocker, lethal) in options.blockers.iter().zip(&options.lethal) {
        let share = left.min(*lethal);
        left -= share;
        shares.push((*blocker, share));
    }
    if left > 0
        && options.excess.is_none()
        && let Some(first) = shares.first_mut()
    {
        first.1 += left;
    }
    shares
}

/// Whether `attacker` kills `blocker` in a fight between the two.
fn kills(attacker: &Body, blocker: &Body) -> bool {
    !blocker.keys.indestructible
        && attacker.power() > 0
        && (attacker.keys.deathtouch || attacker.power() >= blocker.toughness())
}

/// Whether `blocker` can block `attacker`, kill it and live: the block that makes an attack a loss.
fn punishes(blocker: &Body, attacker: &Body) -> bool {
    if !attacker.blockable_by(blocker) || !kills(blocker, attacker) {
        return false;
    }
    let blocker_first = (blocker.keys.first_strike || blocker.keys.double_strike)
        && !(attacker.keys.first_strike || attacker.keys.double_strike);
    // A blocker that strikes first kills the attacker before it deals its damage; a cheap one (a deathtouch chump)
    // trades far up.
    blocker_first
        || !kills(attacker, blocker)
        || creature_value(blocker) < TRADE_UP * creature_value(attacker)
}

/// The share of an attacker's value below which a blocker that dies killing it still makes the attack a loss.
const TRADE_UP: f64 = 0.6;

/// Every permanent `seen` reads, by id.
fn bodies_by_id(seen: &Seen<'_>) -> BTreeMap<ObjectId, Body> {
    seen.bodies()
        .into_iter()
        .map(|body| (body.id, body))
        .collect()
}

/// What each untapped creature that would punish the seat's attackers takes off a player's appeal as a target.
const WALL: f64 = 8.0;

/// The opponent the seat's attacks go at among `offered`: one its creatures can kill this turn, else the one who
/// threatens its team most ([`threat::player_threat`], the archenemy), less for each untapped creature of theirs that punishes the seat's attackers. Teammates choose the same way, so they focus on one opponent.
pub fn focus(
    seen: &Seen<'_>,
    bodies: &BTreeMap<ObjectId, Body>,
    offered: &[PlayerId],
) -> Option<PlayerId> {
    let power: i32 = bodies
        .values()
        .filter(|body| body.is_creature() && body.controller == seen.seat() && !body.tapped)
        .map(Body::power)
        .sum();
    let all: Vec<Body> = bodies.values().cloned().collect();
    let mut best: Option<(f64, PlayerId)> = None;
    for player in offered.iter().copied().filter(|p| !seen.is_teammate(*p)) {
        let life = seen.life(player);
        let board = threat::player_threat(seen, player, &all);
        // Untapped creatures that punish the seat's attackers make a player a worse target.
        let walls = bodies
            .values()
            .filter(|blocker| {
                blocker.controller == player && blocker.is_creature() && !blocker.tapped
            })
            .filter(|blocker| {
                bodies.values().any(|attacker| {
                    attacker.controller == seen.seat()
                        && attacker.is_creature()
                        && punishes(blocker, attacker)
                })
            })
            .count();
        let score = if life <= power {
            1_000.0 - f64::from(life)
        } else {
            board + 0.5 * f64::from(40 - life.min(40))
                - WALL * f64::from(u32::try_from(walls).unwrap_or(u32::MAX))
        };
        if best.is_none_or(|(held, _)| score > held) {
            best = Some((score, player));
        }
    }
    best.map(|(_, player)| player)
}

/// Each attacker the seat may declare with the defender it attacks: `at` when offered, else an opposing player, else
/// an opposing planeswalker; a creature with only teammates to attack is left out unless it must attack.
fn aimed(seen: &Seen<'_>, options: &AttackOptions, at: Option<PlayerId>) -> Attack {
    let opposing = |target: &AttackTarget| match target {
        AttackTarget::Player(player) => !seen.is_teammate(*player),
        AttackTarget::Planeswalker(object) => seen
            .controller(*object)
            .is_some_and(|p| !seen.is_teammate(p)),
    };
    let mut attack = Vec::new();
    for option in &options.attackers {
        let target = at
            .map(AttackTarget::Player)
            .filter(|target| option.defenders.contains(target))
            .or_else(|| {
                option
                    .defenders
                    .iter()
                    .copied()
                    .find(|t| matches!(t, AttackTarget::Player(_)) && opposing(t))
            })
            .or_else(|| option.defenders.iter().copied().find(opposing))
            .or_else(|| {
                options
                    .required
                    .contains(&option.object)
                    .then(|| option.defenders.first().copied())
                    .flatten()
            });
        if let Some(target) = target {
            attack.push((option.object, target));
        }
    }
    attack
}

/// The players `options` lets the seat attack.
fn offered_players(options: &AttackOptions) -> Vec<PlayerId> {
    let mut players: Vec<PlayerId> = Vec::new();
    for option in &options.attackers {
        for target in &option.defenders {
            if let AttackTarget::Player(player) = target
                && !players.contains(player)
            {
                players.push(*player);
            }
        }
    }
    players
}

/// The attack read off the battlefield alone: every creature that no untapped defender can block, kill and survive,
/// and every creature when together they deal lethal damage past the defenders' blockers.
pub fn quick_attack(seen: &Seen<'_>, options: &AttackOptions) -> Attack {
    let bodies = bodies_by_id(seen);
    let at = focus(seen, &bodies, &offered_players(options));
    let all = aimed(seen, options, at);
    let defenders: Vec<&Body> = bodies
        .values()
        .filter(|body| body.is_creature() && !body.tapped && !seen.is_teammate(body.controller))
        .collect();
    let attacking: Vec<&Body> = all.iter().filter_map(|(id, _)| bodies.get(id)).collect();
    if let Some(target) = at {
        let mut powers: Vec<i32> = attacking
            .iter()
            .map(|body| body.power() * if body.keys.double_strike { 2 } else { 1 })
            .collect();
        powers.sort_unstable_by(|a, b| b.cmp(a));
        let through: i32 = powers.iter().skip(defenders.len()).sum();
        if through >= seen.life(target) && options.accepts(&all) {
            return all;
        }
    }
    let attack: Attack = all
        .iter()
        .filter(|(id, _)| {
            options.required.contains(id)
                || bodies.get(id).is_some_and(|attacker| {
                    attacker.power() > 0
                        && !defenders.iter().any(|blocker| punishes(blocker, attacker))
                })
        })
        .copied()
        .collect();
    if options.accepts(&attack) {
        attack
    } else {
        options.legal_attack()
    }
}

/// The damage the opposing teams could deal the seat's team on their next attacks if `attack` is declared: its
/// attackers without vigilance stay tapped through those turns and cannot block.
pub fn crack_back(seen: &Seen<'_>, attack: &Attack) -> i32 {
    let mut bodies = seen.bodies();
    for body in &mut bodies {
        if attack.iter().any(|(id, _)| *id == body.id) && !body.keys.vigilance {
            body.tapped = true;
        }
    }
    let own = seen.team_of(seen.seat());
    let mut teams: Vec<u32> = Vec::new();
    for player in seen.alive() {
        let team = seen.team_of(player);
        if team != own && !teams.contains(&team) {
            teams.push(team);
        }
    }
    teams
        .into_iter()
        .map(|team| pressure(seen, &bodies, team, own))
        .sum()
}

/// [`quick_attack`] that keeps enough creatures home to survive the opposing teams' next attacks
/// ([`crack_back`]), its best blockers first, unless the attack is lethal itself.
pub fn safe_attack(seen: &Seen<'_>, options: &AttackOptions) -> Attack {
    let mut attack = quick_attack(seen, options);
    let life = seen.life(seen.seat());
    if lethal(seen, &attack) || crack_back(seen, &attack) < life {
        return attack;
    }
    let bodies = bodies_by_id(seen);
    let mut keep: Vec<(i32, ObjectId)> = attack
        .iter()
        .filter(|(id, _)| !options.required.contains(id))
        .filter_map(|(id, _)| {
            bodies
                .get(id)
                .map(|body| (body.toughness() + body.power(), *id))
        })
        .collect();
    keep.sort_by_key(|(worth, id)| (std::cmp::Reverse(*worth), *id));
    for (_, id) in keep {
        attack.retain(|(attacker, _)| *attacker != id);
        if crack_back(seen, &attack) < life {
            break;
        }
    }
    if options.accepts(&attack) {
        attack
    } else {
        options.legal_attack()
    }
}

/// Whether `attack` kills a defending player outright if each of that player's team's untapped creatures blocks one of
/// its biggest attackers.
fn lethal(seen: &Seen<'_>, attack: &Attack) -> bool {
    let bodies = bodies_by_id(seen);
    let mut players: Vec<PlayerId> = Vec::new();
    for (_, target) in attack {
        if let AttackTarget::Player(player) = target
            && !players.contains(player)
        {
            players.push(*player);
        }
    }
    players.into_iter().any(|player| {
        let blockers = bodies
            .values()
            .filter(|body| {
                body.is_creature() && !body.tapped && seen.same_team(body.controller, player)
            })
            .count();
        let mut powers: Vec<i32> = attack
            .iter()
            .filter(|(_, target)| *target == AttackTarget::Player(player))
            .filter_map(|(id, _)| bodies.get(id))
            .map(|body| body.power() * if body.keys.double_strike { 2 } else { 1 })
            .collect();
        powers.sort_unstable_by(|a, b| b.cmp(a));
        powers.iter().skip(blockers).sum::<i32>() >= seen.life(player)
    })
}

/// What a combat forecast is worth to `seen`'s team: the values of opposing creatures that die less its own, the life
/// its players lose (a lethal hit weighing like a lost game) and half of what its opponents lose.
fn forecast_score(
    seen: &Seen<'_>,
    values: &BTreeMap<ObjectId, f64>,
    forecast: &CombatForecast,
) -> f64 {
    let mut score = 0.0;
    for creature in forecast.creatures.iter().filter(|c| c.dies) {
        let value = values.get(&creature.object).copied().unwrap_or(2.0);
        let ours = seen
            .controller(creature.object)
            .is_some_and(|p| seen.is_teammate(p));
        score += if ours { -value } else { value };
    }
    for player in &forecast.players {
        let change = life_value(player.life_after) - life_value(player.life);
        if seen.is_teammate(player.player) {
            if player.lethal || player.life_after <= 0 {
                score -= LETHAL_HIT;
            } else {
                score += change;
            }
        } else {
            score -= 0.5 * change;
        }
    }
    score
}

/// Blocks for `player`, built greedily on the engine's forecast of the pending declaration: start from what the rules
/// require and add the block that improves the forecast most until none does or `budget` forecasts are spent. With
/// `gangs`, two creatures may block one attacker together; a menace attacker always needs two.
pub fn greedy_blocks(
    engine: &Engine,
    player: PlayerId,
    options: &BlockOptions,
    budget: &mut u32,
    gangs: bool,
) -> Blocks {
    let seen = Seen::new(engine.state(), player);
    let values: BTreeMap<ObjectId, f64> = seen
        .bodies()
        .iter()
        .map(|body| (body.id, creature_value(body)))
        .collect();
    let mut score = |blocks: &Blocks| -> Option<f64> {
        if *budget == 0 || !options.accepts(blocks) {
            return None;
        }
        *budget -= 1;
        let forecast = engine
            .preview_forecast(player, &Answer::DeclareBlockers(blocks.clone()))
            .ok()?;
        Some(forecast_score(&seen, &values, &forecast))
    };
    let mut blocks = options.legal_blocks();
    let mut best = score(&blocks).unwrap_or(f64::MIN);
    loop {
        let mut improved: Option<(f64, Blocks)> = None;
        for option in &options.blockers {
            let used = blocks.iter().filter(|(b, _)| *b == option.object).count();
            if used > option.extra {
                continue;
            }
            for attacker in &option.can_block {
                if blocks.contains(&(option.object, *attacker)) {
                    continue;
                }
                let mut single = blocks.clone();
                single.push((option.object, *attacker));
                let needs_two = options
                    .min_blockers
                    .iter()
                    .any(|(a, n)| a == attacker && *n >= 2);
                let mut tries = vec![single.clone()];
                if needs_two || gangs {
                    for other in options
                        .blockers
                        .iter()
                        .filter(|o| o.object > option.object && o.can_block.contains(attacker))
                        .filter(|o| !blocks.iter().any(|(b, _)| *b == o.object))
                    {
                        let mut pair = single.clone();
                        pair.push((other.object, *attacker));
                        tries.push(pair);
                    }
                }
                for candidate in tries {
                    if let Some(value) = score(&candidate)
                        && value > best + 0.01
                        && improved.as_ref().is_none_or(|(held, _)| value > *held)
                    {
                        improved = Some((value, candidate));
                    }
                }
            }
        }
        match improved {
            Some((value, better)) => {
                best = value;
                blocks = better;
            }
            None => break,
        }
    }
    blocks
}

/// [`greedy_blocks`] without gangs, within `budget` forecasts: the blocks every seat makes in a simulation.
pub fn quick_blocks(
    engine: &Engine,
    player: PlayerId,
    options: &BlockOptions,
    budget: u32,
) -> Blocks {
    let mut budget = budget.max(4);
    greedy_blocks(engine, player, options, &mut budget, false)
}

/// Block sets worth comparing for `seat`, the greedy blocks with gangs first: those, the greedy blocks without gangs,
/// no blocks, and the greedy blocks less each one; `budget` forecasts are shared by the two greedy builds.
pub fn block_candidates(
    engine: &Engine,
    seat: PlayerId,
    options: &BlockOptions,
    budget: u32,
) -> Vec<Blocks> {
    let half = budget / 2;
    let mut first = half;
    let greedy = greedy_blocks(engine, seat, options, &mut first, true);
    let mut second = budget - half;
    let plain = greedy_blocks(engine, seat, options, &mut second, false);
    let mut candidates: Vec<Blocks> = vec![greedy.clone(), plain, options.legal_blocks()];
    for at in 0..greedy.len() {
        let mut fewer = greedy.clone();
        fewer.remove(at);
        candidates.push(fewer);
    }
    let mut unique: Vec<Blocks> = Vec::new();
    for blocks in candidates {
        if !unique.contains(&blocks) && options.accepts(&blocks) {
            unique.push(blocks);
        }
    }
    if unique.is_empty() {
        unique.push(greedy);
    }
    unique
}

/// The seat's blocks: [`greedy_blocks`] within the search's forecast budget. A search that looks ahead also blocks with
/// two creatures together and then simulates each of [`block_candidates`] through the next turn's combat, so what
/// survives to attack or block next turn counts too.
pub fn plan_blocks(search: &mut Search, engine: &Engine, options: &BlockOptions) -> Blocks {
    let mut budget = 0;
    while search.take_forecast() {
        budget += 1;
    }
    let seat = search.seat();
    if !search.settings().combat_search {
        return greedy_blocks(engine, seat, options, &mut budget, false);
    }
    let candidates = block_candidates(engine, seat, options, budget);
    let fallback = candidates[0].clone();
    let answers = candidates
        .into_iter()
        .map(Answer::DeclareBlockers)
        .collect();
    match search.best(engine, answers, Horizon::NextTurn) {
        Some((Answer::DeclareBlockers(blocks), _)) => blocks,
        _ => fallback,
    }
}

/// Attacks worth comparing for the seat, each accepted and different, the likeliest first: the safe attack
/// ([`safe_attack`]), the quick attack, every creature at the focused opponent, no attack, every creature at each
/// other opponent, and the quick attack with one creature more or less.
pub fn attack_candidates(seen: &Seen<'_>, options: &AttackOptions) -> Vec<Attack> {
    let bodies = bodies_by_id(seen);
    let offered = offered_players(options);
    let at = focus(seen, &bodies, &offered);
    let all = aimed(seen, options, at);
    let quick = quick_attack(seen, options);
    let mut candidates: Vec<Attack> = vec![
        safe_attack(seen, options),
        quick.clone(),
        all.clone(),
        options.legal_attack(),
    ];
    for other in offered
        .iter()
        .copied()
        .filter(|p| Some(*p) != at && !seen.is_teammate(*p))
    {
        candidates.push(aimed(seen, options, Some(other)));
    }
    for pick in &all {
        let toggled: Attack = if quick.iter().any(|(id, _)| *id == pick.0) {
            quick
                .iter()
                .copied()
                .filter(|(id, _)| *id != pick.0)
                .collect()
        } else {
            quick
                .iter()
                .copied()
                .chain(std::iter::once(*pick))
                .collect()
        };
        candidates.push(toggled);
    }
    let mut keys: Vec<Attack> = Vec::new();
    let mut unique: Vec<Attack> = Vec::new();
    for attack in candidates {
        let mut key = attack.clone();
        key.sort_unstable_by_key(|(id, _)| *id);
        if !keys.contains(&key) && options.accepts(&attack) {
            keys.push(key);
            unique.push(attack);
        }
    }
    if unique.is_empty() {
        unique.push(options.legal_attack());
    }
    unique
}

/// The seat's attack: candidates (no attack, every creature, the quick attack, and one creature more or less than the
/// best so far) simulated through the defenders' likely blocks to the end of combat, or through the next turn's combat
/// when the search looks ahead, the best kept. When it looks ahead at a table of several opponents it also tries
/// attacking each of them.
pub fn plan_attack(search: &mut Search, engine: &Engine, options: &AttackOptions) -> Attack {
    let seat = search.seat();
    let seen = Seen::new(engine.state(), seat);
    if !search.settings().combat_search {
        return if search.settings().careful {
            safe_attack(&seen, options)
        } else {
            quick_attack(&seen, options)
        };
    }
    let bodies = bodies_by_id(&seen);
    let offered = offered_players(options);
    let at = focus(&seen, &bodies, &offered);
    let all = aimed(&seen, options, at);
    let horizon = if search.settings().lookahead {
        Horizon::NextTurn
    } else {
        Horizon::EndOfCombat
    };
    let mut candidates: Vec<Attack> = vec![
        options.legal_attack(),
        all.clone(),
        quick_attack(&seen, options),
    ];
    if search.settings().careful {
        candidates.insert(0, safe_attack(&seen, options));
    }
    if search.settings().lookahead {
        for other in offered
            .iter()
            .copied()
            .filter(|p| Some(*p) != at && !seen.is_teammate(*p))
        {
            candidates.push(aimed(&seen, options, Some(other)));
        }
    }
    let mut tried: Vec<Attack> = Vec::new();
    let mut best: Option<(f64, Attack)> = None;
    let mut consider = |search: &mut Search, attack: Attack, best: &mut Option<(f64, Attack)>| {
        let mut key = attack.clone();
        key.sort_unstable_by_key(|(id, _)| *id);
        if tried.contains(&key) || !options.accepts(&attack) {
            return;
        }
        tried.push(key);
        if let Some(outcome) =
            search.outcome(engine, Answer::DeclareAttackers(attack.clone()), horizon)
            && best.as_ref().is_none_or(|(held, _)| outcome.score > *held)
        {
            *best = Some((outcome.score, attack));
        }
    };
    for attack in candidates {
        consider(search, attack, &mut best);
    }
    // One creature more or less than the best so far, the least valuable first.
    let mut order: Vec<(f64, (ObjectId, AttackTarget))> = all
        .iter()
        .map(|pick| (bodies.get(&pick.0).map_or(0.0, creature_value), *pick))
        .collect();
    order.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, pick) in order {
        if !search.can_simulate() {
            break;
        }
        let Some((_, current)) = best.clone() else {
            break;
        };
        let toggled: Attack = if current.iter().any(|(id, _)| *id == pick.0) {
            current
                .iter()
                .copied()
                .filter(|(id, _)| *id != pick.0)
                .collect()
        } else {
            current
                .iter()
                .copied()
                .chain(std::iter::once(pick))
                .collect()
        };
        consider(search, toggled, &mut best);
    }
    best.map_or_else(|| safe_attack(&seen, options), |(_, attack)| attack)
}
