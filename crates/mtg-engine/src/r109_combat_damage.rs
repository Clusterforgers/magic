//! Combat damage beyond the usual recipients and assigner (CR 510.1): trample over planeswalkers (CR 702.19c), a division
//! among the defending player and their creatures, an unblocked attacker's damage to one of their creatures, the defending
//! player assigning it, and lethal damage that counts only when one source dealt it (CR 704.5g).

use mtg_cards::modify::CombatQuirk;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::characteristics::{characteristics, controller_of, controller_of_object};
use crate::combat::{AttackTarget, Attacker, DamageTarget, defending_player};
use crate::damage::{Division, default_shares, defender_of, has_choice, lethal_damage};
use crate::decision::DamageOptions;
use crate::keywords::DamageRules;
use crate::state::GameState;
use crate::zones::Zone;

/// Whether a combat rule of r109 is on the creature.
fn has(state: &GameState, id: ObjectId, quirk: &CombatQuirk) -> bool {
    state.rules().quirks.has(id, |q| q == quirk)
}

/// The creatures `player` controls now, in the order of the battlefield.
fn creatures_of(state: &GameState, player: PlayerId) -> Vec<ObjectId> {
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| controller_of_object(state, object) == player)
        .filter(|object| characteristics(state, object.id()).is_some_and(|c| c.is_creature()))
        .map(crate::object::GameObject::id)
        .collect()
}

/// The division the attacker's damage offers when a rule or keyword widens its recipients; `None` under the usual rules.
///
/// - Unblocked with "may assign its combat damage to a creature defending player controls": all of it to one of those
///   creatures, or all of it as usual.
/// - "Divided as you choose among defending player and/or any number of creatures they control": any division among them,
///   blocked or not; the planeswalker it attacks takes all of it or none, as it would unblocked.
/// - Trample over planeswalkers attacking a planeswalker: lethal damage to each blocker and to the planeswalker, and the
///   rest to that planeswalker's controller (CR 702.19c).
pub(crate) fn r109_options(
    state: &GameState,
    attacker: &Attacker,
    power: u32,
    rules: DamageRules,
) -> Option<DamageOptions> {
    let id = attacker.object();
    if power == 0 {
        return None;
    }
    let defending = defending_player(state, attacker.target())?;
    let attacked = match attacker.target() {
        AttackTarget::Planeswalker(walker) if !attacker.attacks_nothing() => Some(walker),
        _ => None,
    };
    let options = |blockers: Vec<ObjectId>, lethal: Vec<u32>, excess, whole| DamageOptions {
        attacker: id,
        power,
        blockers,
        excess,
        lethal,
        unblocked: false,
        whole,
    };
    if !attacker.blocked() && has(state, id, &CombatQuirk::R109UnblockedToCreature) {
        let creatures = creatures_of(state, defending);
        if !creatures.is_empty() {
            let excess = defender_of(attacker);
            return Some(DamageOptions {
                // With nothing left to attack, giving no creature damage deals none.
                unblocked: excess.is_none(),
                ..options(
                    creatures.clone(),
                    vec![0; creatures.len()],
                    excess,
                    creatures,
                )
            });
        }
    }
    if has(state, id, &CombatQuirk::R109AssignsAmongDefenders) {
        let mut recipients: Vec<ObjectId> = attacker.blockers().to_vec();
        for creature in creatures_of(state, defending) {
            if !recipients.contains(&creature) {
                recipients.push(creature);
            }
        }
        let mut whole = Vec::new();
        if let Some(walker) = attacked
            && !recipients.contains(&walker)
        {
            recipients.push(walker);
            whole.push(walker);
        }
        let lethal = vec![0; recipients.len()];
        return Some(options(
            recipients,
            lethal,
            Some(DamageTarget::Player(defending)),
            whole,
        ));
    }
    if rules.over_planeswalkers
        && let Some(walker) = attacked
        && !crate::battle::is_battle(state, walker)
    {
        let controller = controller_of(state, walker)?;
        let mut recipients: Vec<ObjectId> = if attacker.blocked() {
            attacker.blockers().to_vec()
        } else {
            Vec::new()
        };
        let mut lethal: Vec<u32> = recipients
            .iter()
            .map(|blocker| lethal_damage(state, *blocker, rules.deathtouch))
            .collect();
        recipients.push(walker);
        lethal.push(crate::damage::lethal_for_excess(state, walker, rules.deathtouch).unwrap_or(0));
        return Some(options(
            recipients,
            lethal,
            Some(DamageTarget::Player(controller)),
            Vec::new(),
        ));
    }
    None
}

/// The division of widened `options`: as recorded once divided, else a choice, or the only way when there is none.
pub(crate) fn r109_division(attacker: &Attacker, options: DamageOptions) -> Division {
    let Some(recorded) = &attacker.assignment else {
        return if has_choice(&options) {
            Division::Choose(options)
        } else {
            Division::Fixed(default_shares(&options))
        };
    };
    let mut shares: Vec<(DamageTarget, u32)> = options
        .blockers
        .iter()
        .map(|to| {
            let share = recorded
                .iter()
                .filter(|(given, _)| given == to)
                .fold(0u32, |sum, (_, n)| sum.saturating_add(*n));
            (DamageTarget::Object(*to), share)
        })
        .filter(|(_, share)| *share > 0)
        .collect();
    let assigned = shares
        .iter()
        .fold(0u32, |sum, (_, n)| sum.saturating_add(*n));
    if let Some(excess) = options.excess
        && options.power > assigned
    {
        shares.push((excess, options.power - assigned));
    }
    Division::Fixed(shares)
}

/// The player who assigns the attacker's combat damage in place of its controller: the one it attacks, under "Rather than
/// the attacking player, you assign the combat damage of each creature attacking you" (CR 510.1).
pub(crate) fn r109_divider(state: &GameState, attacker: &Attacker) -> Option<PlayerId> {
    if !has(state, attacker.object(), &CombatQuirk::R109DefenderAssigns) {
        return None;
    }
    defending_player(state, attacker.target())
}

/// The damage marked on `id` that counts as lethal: the most one source dealt when lethal damage destroys it only from a
/// single source, else all of it (`marked`).
pub(crate) fn r109_lethal_marked(state: &GameState, id: ObjectId, marked: u32) -> u32 {
    if !has(state, id, &CombatQuirk::R109LethalFromOneSource) {
        return marked;
    }
    state
        .object(id)
        .and_then(crate::object::GameObject::permanent)
        .map_or(0, crate::object::PermanentState::r109_most_from_one_source)
}
