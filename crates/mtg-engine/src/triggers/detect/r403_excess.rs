//! Excess damage dealt at once (CR 120.10): the damage of one batch to each permanent added up, less the damage that was
//! lethal to it before the batch; "whenever a creature an opponent controls is dealt excess noncombat damage".

use mtg_core::ids::ObjectId;

use super::Happened;
use crate::combat::DamageTarget;
use crate::events::Event;
use crate::state::GameState;

/// One `DealtExcess` for each permanent the damage of `events` dealt more than lethal damage to, as `pre` stood.
pub(super) fn r403_dealt_excess(
    pre: &GameState,
    events: &[Event],
    post: &GameState,
) -> Vec<Happened> {
    let mut dealt: Vec<(ObjectId, u32, bool)> = Vec::new();
    for event in events {
        let Event::DamageDealt {
            target: DamageTarget::Object(hit),
            amount,
            deathtouch,
            ..
        } = event
        else {
            continue;
        };
        match dealt.iter_mut().find(|(id, ..)| id == hit) {
            Some((_, total, any_deathtouch)) => {
                *total = total.saturating_add(*amount);
                *any_deathtouch |= *deathtouch;
            }
            None => dealt.push((*hit, *amount, *deathtouch)),
        }
    }
    if dealt.is_empty() {
        return Vec::new();
    }
    let combat = crate::replace::in_combat_damage(post);
    dealt
        .into_iter()
        .filter_map(|(object, total, deathtouch)| {
            let lethal = crate::damage::lethal_for_excess(pre, object, deathtouch)?;
            (total > lethal).then(|| Happened::DealtExcess {
                object,
                amount: total - lethal,
                combat,
            })
        })
        .collect()
}
