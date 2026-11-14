//! `Effect::R6007KeepTotalPower`: each player keeps any number of their permanents of a kind with total power at most a
//! number and sacrifices the rest, all together (Destined Confrontation, Slaughter the Strong).
//!
//! Each player picks the permanents they keep one at a time, in turn order from the active player and seeing the picks
//! before theirs; a pick offers only what keeps the total within the most, and choosing nothing ends the player's picks.
//! Every set within the most is reachable this way (a permanent with power below 0 is offered whatever the total), and
//! no other set is.

use mtg_cards::effect::Effect;
use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::characteristics::{characteristics, controller_of};
use crate::choose::{CardsOptions, Choice};
use crate::decision::{Decision, DecisionKind};
use crate::events::{Event, ZoneCause};
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::snapshot::snapshot;
use crate::zones::Zone;

/// The power an object counts for: 0 when it has none.
fn r6007_power(state: &crate::state::GameState, id: ObjectId) -> i64 {
    characteristics(state, id)
        .and_then(|seen| seen.power)
        .map_or(0, i64::from)
}

/// The players' picks are read back from the journal in order; the next one is asked, or, once every player has picked,
/// every permanent of the kind that no player kept goes to its owner's graveyard as sacrificed (one that can't be
/// sacrificed stays).
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R6007KeepTotalPower { what, most } = effect else {
        return Err(wrong_effect("R6007KeepTotalPower"));
    };
    let state = cx.state();
    let most = cx.scope.amount(most)?;
    let matching: Vec<ObjectId> = cx
        .scope
        .matching(what)?
        .iter()
        .map(|named| named.id)
        .filter(|id| {
            state
                .object(*id)
                .is_some_and(|object| object.zone() == Zone::Battlefield)
        })
        .collect();
    let players = state.turn_order_from(state.active_or_next());
    let owned = |player: PlayerId| -> Vec<ObjectId> {
        matching
            .iter()
            .copied()
            .filter(|id| controller_of(state, *id) == Some(player))
            .collect()
    };
    let mut answers = cx.journal().iter();
    let mut kept: Vec<ObjectId> = Vec::new();
    for player in players.iter().copied() {
        let mine = owned(player);
        let mut total: i64 = 0;
        loop {
            let offered: Vec<ObjectId> = mine
                .iter()
                .copied()
                .filter(|id| !kept.contains(id))
                .filter(|id| total + r6007_power(state, *id).max(0) <= most)
                .collect();
            if offered.is_empty() {
                break;
            }
            let options = CardsOptions {
                source: cx.scope.source.id,
                from: offered.clone(),
                min: 0,
                max: 1,
                text: format!(
                    "Choose a creature to keep (total power {total} of {most}); choose none to sacrifice the rest."
                ),
                can_cancel: false,
                one_per_name: false,
                groups: Vec::new(),
            };
            match answers.next() {
                None => {
                    return Ok(Run::Ask(Decision {
                        player,
                        kind: DecisionKind::ChooseCards(options),
                    }));
                }
                Some((who, Choice::Cards(cards))) if *who == player && cards.is_empty() => break,
                Some((who, Choice::Cards(cards)))
                    if *who == player && cards.len() == 1 && offered.contains(&cards[0]) =>
                {
                    total += r6007_power(state, cards[0]);
                    kept.push(cards[0]);
                }
                Some(other) => {
                    return Err(ResolveError::Invalid(format!(
                        "{other:?} is no answer to the choice of what {player:?} keeps"
                    )));
                }
            }
        }
    }
    if answers.next().is_some() {
        return Err(ResolveError::Invalid(
            "an answer was given to a choice of what to keep that was not asked".into(),
        ));
    }
    let mut events = Vec::new();
    let mut snapshots = Vec::new();
    for object in matching.iter().copied().filter(|id| !kept.contains(id)) {
        let (Some(owner), Some(controller)) = (
            state.object(object).map(crate::object::GameObject::owner),
            controller_of(state, object),
        ) else {
            continue;
        };
        if !players.contains(&controller)
            || !crate::prohibit::may_sacrifice(state, controller, object)
        {
            continue;
        }
        snapshots.extend(snapshot(state, object));
        events.push(Event::ZoneChanged {
            object,
            from: Zone::Battlefield,
            to: Zone::Graveyard(owner),
            cause: ZoneCause::Sacrificed,
        });
    }
    Ok(Run::Done {
        events,
        bound: Vec::new(),
        snapshots,
        next: Next::Sibling,
    })
}
