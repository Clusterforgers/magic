//! `Effect::Manifest` and `Effect::Cloak`: cards from the top of a library are put onto the battlefield face down as 2/2
//! creatures under the control of the player who acts (CR 701.40a, 701.58a).

use mtg_cards::effect::Effect;
use mtg_cards::refs::Ref;
use mtg_core::ids::PlayerId;

use super::wrong_effect;
use crate::choose::Target;
use crate::events::{Event, ZoneCause};
use crate::library::top_cards;
use crate::object::ObjectRef;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::zones::Zone;

/// What the effect says, whichever of the two it is.
struct Fields<'a> {
    cloak: bool,
    who: &'a Ref,
    count: &'a mtg_cards::value::Amount,
    from: &'a Option<Ref>,
    bind: &'a Option<String>,
}

fn fields(effect: &Effect) -> Result<Fields<'_>, ResolveError> {
    match effect {
        Effect::Manifest {
            who,
            count,
            from,
            bind,
        } => Ok(Fields {
            cloak: false,
            who,
            count,
            from,
            bind,
        }),
        Effect::Cloak {
            who,
            count,
            from,
            bind,
        } => Ok(Fields {
            cloak: true,
            who,
            count,
            from,
            bind,
        }),
        _ => Err(wrong_effect("Manifest")),
    }
}

/// Each player the effect names who is in the game takes `count` cards from the top of their own library, or of each
/// library `from` names, the top card first, and puts them onto the battlefield face down under their control: one
/// `ZoneChanged` for each card whose cause says who manifested it, and an `Entered` after it when that player is not the
/// card's owner. A library shorter than the count gives what it has and an empty one nothing. The cards are bound to the
/// name, when the effect has one, as the permanents they become (CR 400.7).
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Fields {
        cloak,
        who,
        count,
        from,
        bind,
    } = fields(effect)?;
    let wanted = usize::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(usize::MAX);
    let state = cx.state();
    let mut events = Vec::new();
    let mut made = Vec::new();
    for player in cx.scope.players(who)? {
        if !state.is_alive(player) {
            continue;
        }
        let libraries: Vec<PlayerId> = match from {
            Some(whose) => cx.scope.players(whose)?,
            None => vec![player],
        };
        for owner in libraries.into_iter().filter(|owner| state.is_alive(*owner)) {
            for id in top_cards(state, owner, wanted) {
                let Some(card) = state.object(id) else {
                    continue;
                };
                events.push(Event::ZoneChanged {
                    object: id,
                    from: Zone::Library(owner),
                    to: Zone::Battlefield,
                    cause: if cloak {
                        ZoneCause::Cloaked { by: player }
                    } else {
                        ZoneCause::Manifested { by: player }
                    },
                });
                if owner != player {
                    events.push(Event::Entered {
                        object: id,
                        tapped: false,
                        controller: player,
                        chosen: None,
                    });
                }
                made.push(Target::Object(ObjectRef {
                    id,
                    counter: card.reference().counter + 1,
                }));
            }
        }
    }
    Ok(Run::Done {
        events,
        bound: bind
            .iter()
            .map(|name| (name.clone(), made.clone()))
            .collect(),
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}
