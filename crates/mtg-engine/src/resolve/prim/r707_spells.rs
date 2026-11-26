//! `Effect::R707ExileSpell` and `Effect::R707PutBack`: a spell's controller exiles it and its card keeps the spell, later
//! put back onto the stack as a copy of that original spell (CR 707.10); `Effect::R406TurnUpExiled`: face-down cards in
//! exile are turned face up (CR 406.3).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::characteristics::controller_of_object;
use crate::choose::Target;
use crate::events::{Event, ZoneCause};
use crate::object::{ObjectKind, ObjectRef};
use crate::r707_spells::{R707Kept, keeps_spell};
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::snapshot::snapshot;
use crate::zones::Zone;

/// Each spell the effect names that is still on the stack is exiled; a spell's card keeps the spell (a copy of a spell,
/// which has no card, ceases to exist in exile and keeps nothing, CR 707.10a). The resolving spell and abilities are
/// passed over. The cards are bound to `as` as the objects they become in exile, the controllers of the spells to
/// `player_as`.
pub(super) fn run_exile(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R707ExileSpell {
        what,
        bind,
        player_as,
    } = effect
    else {
        return Err(wrong_effect("R707ExileSpell"));
    };
    let state = cx.state();
    let mut events = Vec::new();
    let mut kept = Vec::new();
    let mut snapshots = Vec::new();
    let mut cards = Vec::new();
    let mut players = Vec::new();
    for spell in cx.scope.live_objects(what)? {
        if spell.zone() != Zone::Stack
            || spell.kind() == ObjectKind::Ability
            || spell.id() == cx.resolving.object()
        {
            continue;
        }
        let controller = controller_of_object(state, spell);
        snapshots.extend(snapshot(state, spell.id()));
        events.push(Event::ZoneChanged {
            object: spell.id(),
            from: Zone::Stack,
            to: Zone::Exile,
            cause: ZoneCause::Exiled,
        });
        if spell.kind() == ObjectKind::Card && spell.stack_copy.is_none() {
            kept.push(Event::R707Kept(Box::new(R707Kept {
                object: spell.id(),
                choices: spell.choices.clone(),
                controller,
            })));
            cards.push(Target::Object(ObjectRef {
                id: spell.id(),
                counter: spell.counter() + 1,
            }));
        }
        if !players.contains(&Target::Player(controller)) {
            players.push(Target::Player(controller));
        }
    }
    events.extend(kept);
    Ok(Run::Done {
        events,
        bound: vec![(bind.clone(), cards), (player_as.clone(), players)],
        snapshots,
        next: Next::Sibling,
    })
}

/// Each card in exile the effect names that still keeps a spell goes onto the stack as a copy of it, under the control
/// of the first player `who` names who is still in the game; a card that left exile since, or keeps nothing, is passed
/// over.
pub(super) fn run_put_back(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R707PutBack { who, what } = effect else {
        return Err(wrong_effect("R707PutBack"));
    };
    let state = cx.state();
    let Some(player) = cx
        .scope
        .players(who)?
        .into_iter()
        .find(|player| state.is_alive(*player))
    else {
        return Ok(Run::events(Vec::new()));
    };
    let events = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|card| keeps_spell(state, card.id()))
        .map(|card| Event::R707PutBack {
            object: card.id(),
            player,
        })
        .collect();
    Ok(Run::events(events))
}

/// Each card the effect names that lies face down in exile is turned face up, all together; the cards turned are bound
/// to `as` (the same objects: turning a card in exile face up is no zone change).
pub(super) fn r406_run_turn_up(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R406TurnUpExiled { what, bind } = effect else {
        return Err(wrong_effect("R406TurnUpExiled"));
    };
    let mut events = Vec::new();
    let mut turned = Vec::new();
    for card in cx.scope.live_objects(what)? {
        if card.zone() != Zone::Exile || card.face_down().is_none() {
            continue;
        }
        events.push(Event::R406TurnedUp { object: card.id() });
        turned.push(Target::Object(card.reference()));
    }
    Ok(Run::Done {
        events,
        bound: vec![(bind.clone(), turned)],
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}
