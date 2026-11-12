//! `Effect::Balance`: each player keeps as many of their permanents of a kind, or of the cards in their hand, as the
//! player with the fewest has, and sacrifices or discards the rest (Restore Balance, Magus of the Balance).

use mtg_cards::effect::{Balanced, Effect};
use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::characteristics::controller_of;
use crate::choose::{Picked, Selection, pick_selections};
use crate::events::{Event, ZoneCause};
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::snapshot::snapshot;
use crate::wording;
use crate::zones::Zone;

/// The count is the fewest any player in the game has now; each player who has more chooses that many to keep, in
/// turn order from the active player, before anything moves. Then every rest goes together: permanents to their
/// owners' graveyards as sacrificed (one that can't be sacrificed stays), cards in hand discarded.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Balance { what } = effect else {
        return Err(wrong_effect("Balance"));
    };
    let state = cx.state();
    let players = state.turn_order_from(state.active_or_next());
    let matching = match what {
        Balanced::Permanents(filter) => Some(cx.scope.matching(filter)?),
        Balanced::Hand => None,
    };
    let owned: Vec<(PlayerId, Vec<ObjectId>)> = players
        .iter()
        .map(|player| {
            let objects = match &matching {
                Some(matching) => matching
                    .iter()
                    .map(|permanent| permanent.id)
                    .filter(|id| {
                        state.object(*id).is_some_and(|object| {
                            object.zone() == Zone::Battlefield
                                && controller_of(state, *id) == Some(*player)
                        })
                    })
                    .collect(),
                None => state.zone(Zone::Hand(*player)).to_vec(),
            };
            (*player, objects)
        })
        .collect();
    let fewest = owned
        .iter()
        .map(|(_, objects)| objects.len())
        .min()
        .unwrap_or(0);
    let selections: Vec<Selection> = owned
        .iter()
        .map(|(player, objects)| Selection {
            one_per_name: false,
            player: *player,
            from: objects.clone(),
            min: fewest,
            max: fewest,
            text: match what {
                Balanced::Permanents(filter) => wording::r370_keep(filter, fewest),
                Balanced::Hand => {
                    format!("Choose {fewest} cards in your hand to keep; you discard the rest.")
                }
            },
        })
        .collect();
    let kept = match pick_selections(cx, &selections)? {
        Picked::Ask(decision) => return Ok(Run::Ask(decision)),
        Picked::Done(kept) => kept,
    };
    let mut events = Vec::new();
    let mut snapshots = Vec::new();
    for ((player, objects), (_, keep)) in owned.iter().zip(kept) {
        for object in objects.iter().copied().filter(|id| !keep.contains(id)) {
            match what {
                Balanced::Permanents(_) => {
                    let Some(graveyard_of) =
                        state.object(object).map(crate::object::GameObject::owner)
                    else {
                        continue;
                    };
                    if !crate::prohibit::may_sacrifice(state, *player, object) {
                        continue;
                    }
                    snapshots.extend(snapshot(state, object));
                    events.push(Event::ZoneChanged {
                        object,
                        from: Zone::Battlefield,
                        to: Zone::Graveyard(graveyard_of),
                        cause: ZoneCause::Sacrificed,
                    });
                }
                Balanced::Hand => {
                    snapshots.extend(snapshot(state, object));
                    events.push(Event::CardDiscarded {
                        player: *player,
                        object,
                    });
                }
            }
        }
    }
    Ok(Run::Done {
        events,
        bound: Vec::new(),
        snapshots,
        next: Next::Sibling,
    })
}
