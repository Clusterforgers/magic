//! An activated ability keeps the mana its cost spent, as a spell does (CR 601.2h, 602.2b), so "if exactly three colors
//! of mana were spent to activate this ability" and "if mana from a Treasure was spent to activate this ability" read
//! the activation's own mana (`Amount::ColorsSpent`, `Amount::ManaSpent`); the sources are kept by `record_sources`.

use crate::events::Event;

/// Gives each ability the payment puts on the stack the mana of the payment: the last spending of the batch, the earlier
/// ones paying for mana abilities.
pub(crate) fn r300c_keep_spent(events: &mut [Event]) {
    let Some(spent) = events.iter().rev().find_map(|event| match event {
        Event::ManaSpent { mana, .. } if !mana.is_empty() => Some(mana.clone()),
        _ => None,
    }) else {
        return;
    };
    for event in events.iter_mut() {
        if let Event::AbilityPutOnStack(put) = event
            && put.choices.spent.is_none()
        {
            put.choices.spent = Some(Box::new(spent.clone()));
        }
    }
}
