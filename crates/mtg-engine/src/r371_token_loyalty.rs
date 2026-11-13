//! A token planeswalker enters with loyalty counters equal to its loyalty (CR 306.5b): a token copy of a planeswalker,
//! with the starting loyalty an exception of the copy may set (CR 707.9b).

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;

use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::object::ObjectRef;

/// The loyalty counters the token `id` of `definition` enters with, proposed with the event that creates it; `None` for a
/// token that is no planeswalker or has no loyalty.
pub(crate) fn entering(id: ObjectId, definition: &CardDefinition) -> Option<Event> {
    if !definition.types.contains(&CardType::Planeswalker) {
        return None;
    }
    let loyalty = definition.loyalty.filter(|loyalty| *loyalty > 0)?;
    Some(Event::CountersChanged(CountersChanged {
        on: Target::Object(ObjectRef { id, counter: 0 }),
        counter: CounterKind::Loyalty,
        delta: i32::try_from(loyalty).ok()?,
    }))
}

#[cfg(test)]
mod tests {
    //! Goal: a token planeswalker enters with its loyalty as counters, and no other token gets any.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::a_planeswalker(CardType::Planeswalker, Some(3), Some(3))]
    #[case::no_loyalty(CardType::Planeswalker, None, None)]
    #[case::zero_loyalty(CardType::Planeswalker, Some(0), None)]
    #[case::a_creature(CardType::Creature, Some(3), None)]
    fn a_token_planeswalker_enters_with_its_loyalty(
        #[case] kind: CardType,
        #[case] loyalty: Option<u32>,
        #[case] counters: Option<i32>,
    ) {
        let mut definition = CardDefinition::new("Walker").with_type(kind);
        definition.loyalty = loyalty;
        let found = entering(ObjectId(7), &definition).map(|event| match event {
            Event::CountersChanged(changed) => changed.delta,
            _ => 0,
        });
        assert_eq!(found, counters);
    }
}
