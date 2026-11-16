//! `Effect::R6702Distribute`: the controller distributes counters among any number of permanents as the effect resolves
//! ("distribute three stun counters among any number of tapped creatures your opponents control").

use mtg_cards::effect::Effect;
use mtg_core::ids::ObjectId;

use super::put_counters::whole;
use super::wrong_effect;
use crate::choose::{Choice, NumberOptions, Picked, Selection, Target, pick_selections};
use crate::counters::CountersChanged;
use crate::decision::{Decision, DecisionKind};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// The controller chooses between one and as many permanents as there are counters among those the reference names on
/// the battlefield, then how many each of them but the last gets (at least one each, leaving at least one for each
/// after it); the last takes the rest. Every permanent gets its share at once. No counter, or no permanent, is no event.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R6702Distribute {
        among,
        counter,
        count,
    } = effect
    else {
        return Err(wrong_effect("R6702Distribute"));
    };
    let total = whole(cx.scope.amount(count)?);
    let from: Vec<ObjectId> = cx
        .scope
        .live_objects(among)?
        .into_iter()
        .filter(|object| object.zone() == Zone::Battlefield)
        .map(|object| object.id())
        .collect();
    let total = u32::try_from(total).unwrap_or(0);
    if total == 0 || from.is_empty() {
        return Ok(Run::events(Vec::new()));
    }
    let player = cx.controller();
    let most = usize::try_from(total).unwrap_or(usize::MAX);
    let selection = Selection {
        one_per_name: false,
        groups: Vec::new(),
        player,
        min: 1,
        max: most,
        text: format!("Choose the permanents to distribute {total} counters among."),
        from: from.clone(),
    };
    // The choice is asked unless the list holds no more than the fewest to take.
    let asked = from.len() > 1;
    let chosen = match pick_selections(cx, std::slice::from_ref(&selection))? {
        Picked::Ask(decision) => return Ok(Run::Ask(decision)),
        Picked::Done(taken) => taken
            .into_iter()
            .flat_map(|(_, cards)| cards)
            .collect::<Vec<_>>(),
    };
    let mut numbers = cx.journal().iter().skip(usize::from(asked));
    let mut shares: Vec<u32> = Vec::with_capacity(chosen.len());
    let mut left = total;
    for (at, object) in chosen.iter().enumerate() {
        let after = u32::try_from(chosen.len() - at - 1).unwrap_or(u32::MAX);
        if after == 0 {
            shares.push(left);
            break;
        }
        let max = left.saturating_sub(after);
        let share = if max <= 1 {
            1
        } else {
            match numbers.next() {
                Some((who, Choice::Number(share)))
                    if *who == player && (1..=max).contains(share) =>
                {
                    *share
                }
                Some(_) => return Err(ResolveError::Invalid("not a share of the counters".into())),
                None => {
                    let name = cx
                        .state()
                        .object(*object)
                        .map_or_else(String::new, |o| o.card().name.clone());
                    return Ok(Run::Ask(Decision {
                        player,
                        kind: DecisionKind::ChooseNumber(NumberOptions {
                            source: cx.scope.source.id,
                            min: 1,
                            max: Some(max),
                            except: Vec::new(),
                            text: format!("How many of the {left} counters left go on {name}?"),
                            can_cancel: false,
                            cost: None,
                            per_x: 0,
                            secret: false,
                        }),
                    }));
                }
            }
        };
        shares.push(share);
        left -= share;
    }
    let events = chosen
        .iter()
        .zip(shares)
        .filter_map(|(object, share)| {
            let reference = cx.state().object(*object)?.reference();
            Some(Event::CountersChanged(CountersChanged {
                on: Target::Object(reference),
                counter: counter.clone(),
                delta: i32::try_from(share).ok()?,
            }))
        })
        .collect();
    Ok(Run::events(events))
}
