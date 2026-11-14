//! How a boss's side aims under the spotlight (`mtg_engine::spotlight`, `docs/delves.md`): it weighs a hero by threat
//! minus the attention they already got, leans to the hero it eyes, and leaves a hero past their share of the recent
//! pressure alone unless they are an overwhelming threat or the only hero standing.

use mtg_core::ids::PlayerId;
use mtg_engine::combat::AttackTarget;
use mtg_engine::spotlight;

use super::seen::Seen;

/// How much a point of attention lowers a hero's worth as a target.
pub const ATTENTION_WEIGHT: f64 = 1.0;
/// What being the hero the seat eyes adds to a hero's worth as a target.
pub const FOCUS_BONUS: f64 = 6.0;

/// Whether the seat plays for a boss's side in a campaign fight.
pub fn boss_seat(seen: &Seen<'_>) -> bool {
    spotlight::active(seen.state()) && spotlight::is_boss_side(seen.state(), seen.seat())
}

/// Whether `hero` threatens to end the fight: the engine's overwhelming presence, or creatures whose power together
/// reaches the life of the boss's side's leader.
pub fn overwhelming(seen: &Seen<'_>, hero: PlayerId) -> bool {
    let state = seen.state();
    if spotlight::overwhelming(state, hero) {
        return true;
    }
    let boss_life = state
        .players()
        .iter()
        .filter(|p| p.phases().0 > 0 && p.is_in_game())
        .map(mtg_engine::player::Player::life)
        .min();
    let power: i32 = seen
        .bodies()
        .iter()
        .filter(|body| body.controller == hero && body.is_creature())
        .map(super::seen::Body::power)
        .sum();
    boss_life.is_some_and(|life| power >= life)
}

/// Whether the seat may aim at `player`: always outside a boss's side or at a non-hero; a boss's side spares a capped
/// hero who is no overwhelming threat.
pub fn may_aim(seen: &Seen<'_>, player: PlayerId) -> bool {
    !boss_seat(seen)
        || !spotlight::is_hero(seen.state(), player)
        || !spotlight::capped(seen.state(), player)
        || overwhelming(seen, player)
}

/// What the spotlight adds to `hero`'s worth as a target of the seat: the focus bonus less the attention weight.
pub fn lean(seen: &Seen<'_>, hero: PlayerId) -> f64 {
    if !boss_seat(seen) || !spotlight::is_hero(seen.state(), hero) {
        return 0.0;
    }
    let focus = spotlight::focus(seen.state(), seen.seat()) == Some(hero);
    let attention = f64::from(spotlight::attention(seen.state(), hero));
    let bonus = if focus { FOCUS_BONUS } else { 0.0 };
    bonus - ATTENTION_WEIGHT * attention
}

/// The share by which attention cuts what removing a permanent of `controller` is worth to the seat.
pub fn object_factor(seen: &Seen<'_>, controller: PlayerId) -> f64 {
    if !boss_seat(seen) || !spotlight::is_hero(seen.state(), controller) {
        return 1.0;
    }
    let attention = f64::from(spotlight::attention(seen.state(), controller));
    let focus = spotlight::focus(seen.state(), seen.seat()) == Some(controller);
    (if focus { 1.25 } else { 1.0 }) / (1.0 + attention / 15.0)
}

/// Whether an attack's defenders are all fair for the seat ([`may_aim`]).
pub fn fair_attack(seen: &Seen<'_>, attack: &[(mtg_core::ids::ObjectId, AttackTarget)]) -> bool {
    attack.iter().all(|(_, target)| match target {
        AttackTarget::Player(player) => may_aim(seen, *player),
        AttackTarget::Planeswalker(object) => seen
            .controller(*object)
            .is_none_or(|player| may_aim(seen, player)),
    })
}

/// The boss's turns in which its side holds back sweepers: wipes and damage or sacrifice to each player or creature.
pub const RESTRAINT_TURNS: u32 = 3;

/// Whether the rule at `index` of `card` sweeps: no target, and it destroys, exiles, damages or sacrifices for each.
fn sweeps(card: &mtg_cards::CardDefinition, index: Option<usize>) -> bool {
    use mtg_cards::ability::Ability;
    card.rules.iter().enumerate().any(|(at, rule)| {
        if index.is_some_and(|wanted| wanted != at) {
            return false;
        }
        let untargeted = match rule.as_ref() {
            Ability::Spell { targets, .. } | Ability::Activated { targets, .. } => {
                targets.is_empty()
            }
            _ => false,
        };
        let text = format!("{:?}", rule.effects());
        untargeted
            && text.contains("Each")
            && ["Destroy {", "Exile {", "DealDamage", "Sacrifice"]
                .iter()
                .any(|word| text.contains(word))
    })
}

/// Whether the seat holds `play` back: a boss's side keeps its sweepers in its first [`RESTRAINT_TURNS`] turns, so the
/// heroes can build before the fight turns.
pub fn held_back(seen: &Seen<'_>, play: &mtg_engine::decision::PriorityAction) -> bool {
    use mtg_engine::decision::PriorityAction;
    if !boss_seat(seen) || spotlight::boss_turns(seen.state()) > RESTRAINT_TURNS {
        return false;
    }
    match play {
        PriorityAction::CastSpell(id) => seen.card(*id).is_some_and(|card| sweeps(card, None)),
        PriorityAction::Activate(ability) => seen
            .card(ability.object)
            .is_some_and(|card| sweeps(card, Some(ability.ability))),
        _ => false,
    }
}
