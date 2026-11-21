//! Blocks an effect changes after blockers are declared (CR 506.4, 509.1h): two blockers trade what they block
//! (`Effect::R115ExchangeBlocks`, Sorrow's Path), two attackers trade their blockers (`Effect::R115ExchangeBlockers`,
//! General Jarkeld), and a creature that just entered joins the combat blocking an attacker (`Effect::R115JoinBlock`,
//! Aetherplasm).

use mtg_cards::effect::Effect;
use mtg_core::ids::ObjectId;

use super::wrong_effect;
use crate::combat::r6400_could_block;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::state::GameState;
use crate::zones::Zone;

/// The attackers `blocker` blocks.
fn blocked_by(state: &GameState, blocker: ObjectId) -> Vec<ObjectId> {
    state
        .combat()
        .attackers()
        .iter()
        .filter(|attacker| attacker.blockers.contains(&blocker))
        .map(crate::combat::Attacker::object)
        .collect()
}

/// The creatures blocking `attacker`.
fn blockers_of(state: &GameState, attacker: ObjectId) -> Vec<ObjectId> {
    state
        .combat()
        .attackers()
        .iter()
        .find(|attack| attack.object() == attacker)
        .map(|attack| attack.blockers.clone())
        .unwrap_or_default()
}

/// Whether `blocker` could block every attacker of `attackers` (those it blocks already included), all at once: its
/// evasion and protection checks, its being untapped and how many creatures it may block (CR 509.1a, 509.1b).
fn could_block_all(state: &GameState, blocker: ObjectId, attackers: &[ObjectId]) -> bool {
    let blocking = blocked_by(state, blocker);
    let open = r6400_could_block(state, blocker);
    let most = crate::w14c_block_capacity::w14c_extra_blocks(state, blocker).saturating_add(1);
    attackers.len() <= most
        && attackers
            .iter()
            .all(|attacker| blocking.contains(attacker) || open.contains(attacker))
}

/// The two creatures on the battlefield `what` names, when it names exactly two.
fn two(
    cx: &Cx<'_>,
    what: &mtg_cards::refs::Ref,
) -> Result<Option<(ObjectId, ObjectId)>, ResolveError> {
    let named: Vec<ObjectId> = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| object.zone() == Zone::Battlefield)
        .map(crate::object::GameObject::id)
        .collect();
    Ok(match named.as_slice() {
        [first, second] if first != second => Some((*first, *second)),
        _ => None,
    })
}

/// Sorrow's Path: when each of the two blockers could block all the attackers the other blocks, both leave combat and
/// each then blocks the other's attackers. Each new block is `R6400Blocks`, so "blocks" abilities trigger and an attacker
/// that stayed blocked does not become blocked again. Otherwise nothing happens.
pub(super) fn run_exchange_blocks(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R115ExchangeBlocks { what } = effect else {
        return Err(wrong_effect("R115ExchangeBlocks"));
    };
    let state = cx.state();
    let Some((first, second)) = two(cx, what)? else {
        return Ok(Run::events(Vec::new()));
    };
    let (of_first, of_second) = (blocked_by(state, first), blocked_by(state, second));
    if !could_block_all(state, first, &of_second) || !could_block_all(state, second, &of_first) {
        return Ok(Run::events(Vec::new()));
    }
    let mut events = vec![
        Event::RemovedFromCombat { object: first },
        Event::RemovedFromCombat { object: second },
    ];
    let blocks = |blocker: ObjectId, attackers: &[ObjectId]| {
        attackers
            .iter()
            .map(move |attacker| Event::R6400Blocks {
                blocker,
                attacker: *attacker,
            })
            .collect::<Vec<_>>()
    };
    events.extend(blocks(first, &of_second));
    events.extend(blocks(second, &of_first));
    Ok(Run::events(events))
}

/// General Jarkeld: when each of the two attackers could be blocked by all the creatures blocking the other, each
/// creature blocking exactly one of them stops blocking it and blocks the other (`R115BlockMoved`, nothing triggers).
/// Otherwise nothing happens.
pub(super) fn run_exchange_blockers(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R115ExchangeBlockers { what } = effect else {
        return Err(wrong_effect("R115ExchangeBlockers"));
    };
    let state = cx.state();
    let Some((first, second)) = two(cx, what)? else {
        return Ok(Run::events(Vec::new()));
    };
    let attacking = |id| state.combat().attackers().iter().any(|a| a.object() == id);
    if !attacking(first) || !attacking(second) {
        return Ok(Run::events(Vec::new()));
    }
    let (of_first, of_second) = (blockers_of(state, first), blockers_of(state, second));
    let could = |blockers: &[ObjectId], attacker: ObjectId, own: &[ObjectId]| {
        blockers.iter().all(|blocker| {
            own.contains(blocker) || r6400_could_block(state, *blocker).contains(&attacker)
        })
    };
    if !could(&of_second, first, &of_first) || !could(&of_first, second, &of_second) {
        return Ok(Run::events(Vec::new()));
    }
    let moved = |blockers: &[ObjectId], other: &[ObjectId], from: ObjectId, to: ObjectId| {
        blockers
            .iter()
            .filter(|blocker| !other.contains(blocker))
            .map(move |blocker| Event::R115BlockMoved {
                blocker: *blocker,
                from: Some(from),
                to,
            })
            .collect::<Vec<_>>()
    };
    let mut events = moved(&of_first, &of_second, first, second);
    events.extend(moved(&of_second, &of_first, second, first));
    Ok(Run::events(events))
}

/// Aetherplasm: each creature `what` names that is on the battlefield and not yet blocking the attacker blocks it
/// (`R115BlockMoved` from nothing), whether or not it could have been declared as its blocker. Nothing joins once the
/// attacker no longer attacks.
pub(super) fn run_join_block(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R115JoinBlock { what, blocking } = effect else {
        return Err(wrong_effect("R115JoinBlock"));
    };
    let state = cx.state();
    let Some(attacker) = cx
        .scope
        .live_objects(blocking)?
        .into_iter()
        .map(crate::object::GameObject::id)
        .find(|id| state.combat().attackers().iter().any(|a| a.object() == *id))
    else {
        return Ok(Run::events(Vec::new()));
    };
    let already = blockers_of(state, attacker);
    let events = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| object.zone() == Zone::Battlefield)
        .filter(|object| {
            crate::characteristics::characteristics(state, object.id())
                .is_some_and(|seen| seen.is_creature())
        })
        .map(crate::object::GameObject::id)
        .filter(|id| !already.contains(id))
        .map(|blocker| Event::R115BlockMoved {
            blocker,
            from: None,
            to: attacker,
        })
        .collect();
    Ok(Run::events(events))
}
