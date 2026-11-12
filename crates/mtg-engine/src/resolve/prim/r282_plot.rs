//! `Effect::Plot`: cards are exiled face up and become plotted (CR 702.170a, 702.170d): their owner may cast each one
//! without paying its mana cost as a sorcery on a later turn, whether or not the card has plot.

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::{Event, ZoneCause};
use crate::object::ObjectKind;
use crate::resolve::{Cx, ResolveError, Run};
use crate::snapshot::snapshot;
use crate::zones::Zone;

/// Each card the effect names that is still where it was named, in a hand, a graveyard, a library or on the stack, moves
/// to exile as a plotted card, all together. An ability, a token or a copy (which is no card), the resolving spell, a
/// permanent and a card already in exile are passed over.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Plot { what } = effect else {
        return Err(wrong_effect("Plot"));
    };
    let state = cx.state();
    let mut events = Vec::new();
    let mut snapshots = Vec::new();
    for object in cx.scope.live_objects(what)? {
        let movable = matches!(
            object.zone(),
            Zone::Hand(_) | Zone::Graveyard(_) | Zone::Library(_) | Zone::Stack
        );
        let resolving = object.zone() == Zone::Stack && object.id() == cx.resolving.object();
        if object.kind() != ObjectKind::Card || !movable || resolving {
            continue;
        }
        snapshots.extend(snapshot(state, object.id()));
        events.push(Event::ZoneChanged {
            object: object.id(),
            from: object.zone(),
            to: Zone::Exile,
            cause: ZoneCause::Plotted,
        });
    }
    Ok(Run::Done {
        events,
        bound: Vec::new(),
        snapshots,
        next: crate::resolve::Next::Sibling,
    })
}
