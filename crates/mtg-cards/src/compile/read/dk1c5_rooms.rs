//! The Room wordings (CR 709.5): "when you unlock this door", "whenever you fully unlock a Room" and the effect "unlock a
//! locked door of a Room you control".

use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::trigger::TriggerPattern;

/// The event after the player and its first word ("unlock", "fully"): "you unlock this door", "you fully unlock a Room".
pub(super) fn dk1c5_door_event(
    r: &mut Reader<'_>,
    verb: &str,
    by: Ref,
    third: bool,
) -> Option<TriggerPattern> {
    let unlock = if third { "unlocks" } else { "unlock" };
    let fully = verb == "fully";
    if fully {
        r.need(unlock)?;
    } else if verb != unlock {
        return None;
    }
    let door = if fully {
        r.need("a")?;
        room(r)?;
        None
    } else {
        // The face the ability is printed on is set as the card is compiled; the reader names the first.
        r.need("this door")?;
        Some(0)
    };
    Some(TriggerPattern::DoorUnlocked {
        who: by,
        door,
        fully,
    })
}

/// The pattern `dk1c5_door_event` reads, written back.
pub(super) fn dk1c5_door_text(pattern: &TriggerPattern) -> Option<String> {
    let TriggerPattern::DoorUnlocked { who, door, fully } = pattern else {
        return None;
    };
    match (door, fully) {
        (Some(_), false) => Some(format!(
            "When {} this door",
            super::triggers::player_verb(who, "unlock")?
        )),
        (None, true) => Some(format!(
            "Whenever {} a Room",
            super::triggers::player_verb(who, "fully unlock")?
        )),
        _ => None,
    }
}

/// The rest of "unlock a locked door of a Room you control" after the verb.
pub(super) fn dk1c5_unlock(r: &mut Reader<'_>, who: Ref) -> Option<Effect> {
    (who == Ref::You).then_some(())?;
    r.need("a locked door of a")?;
    room(r)?;
    r.need("you control")?;
    Some(Effect::UnlockDoor { who })
}

/// The effect `dk1c5_unlock` reads, written back.
pub(super) fn dk1c5_unlock_text(who: &Ref) -> Option<String> {
    (*who == Ref::You).then(|| "unlock a locked door of a Room you control".to_owned())
}

fn room(r: &mut Reader<'_>) -> Option<()> {
    (r.name() == Some("Room")).then_some(())
}

/// "return a card from your graveyard to your hand or unlock a locked door of a Room you control": two sentences joined
/// by "or" are a choice of the controller between them as the effect resolves. Tried after every sentence shape, so
/// a shape that reads the whole sentence (a target of "artifact or enchantment") wins.
pub(super) fn dk1c5_either(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let first = super::effects::templates().find_map(|template| {
        r.attempt(|r| {
            let found = template(r)?;
            r.need("or")?;
            Some(found)
        })
    })?;
    let second = super::effects::one_sentence(r)?;
    // "You may draw a card or discard a card" does not say what the player may do: it stays with the table.
    let optional = |steps: &[Effect]| steps.iter().any(|step| matches!(step, Effect::May { .. }));
    (!optional(&first) && !optional(&second)).then_some(())?;
    Some(vec![Effect::Either { first, second }])
}
