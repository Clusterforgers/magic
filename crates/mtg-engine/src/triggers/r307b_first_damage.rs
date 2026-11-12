//! The time of the turn a source dealt damage to a player ("whenever ~ deals combat damage to a player for the first time
//! each turn"), counted from the turn's log of damage.

use mtg_cards::value::TallyEvent;
use mtg_core::ids::ObjectId;

use crate::choose::Target;
use crate::combat::DamageTarget;
use crate::events::Event;
use crate::state::GameState;

/// Which time of the turn `source` dealt damage of this kind (combat or not) to a player with this event: the records of
/// the log after the batch (`post`), less the same damage later in the batch.
pub(super) fn to_players_ordinal(
    post: &GameState,
    later: &[Event],
    source: ObjectId,
    combat: bool,
) -> u32 {
    let after = post
        .tallies
        .records
        .iter()
        .filter(|record| {
            record.event == TallyEvent::Damaged
                && record.combat == combat
                && record.subject.is_some_and(|subject| subject.id == source)
                && matches!(record.other, Some(Target::Player(_)))
        })
        .count();
    let later = later
        .iter()
        .filter(|event| {
            matches!(event, Event::DamageDealt {
                source: dealer,
                target: DamageTarget::Player(_),
                amount,
                ..
            } if *dealer == source && *amount > 0)
        })
        .count();
    u32::try_from(after.saturating_sub(later)).unwrap_or(u32::MAX)
}
