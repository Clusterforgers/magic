//! Cipher links (CR 702.99): the cards in exile that are encoded on creatures.

use mtg_core::ids::ObjectId;

use crate::object::GameObject;
use crate::state::GameState;
use crate::zones::Zone;

impl GameState {
    /// The creature `card` is encoded on, while the link holds: the card lies in exile and the creature is still the
    /// permanent it was when the card was encoded (a creature that left the battlefield is a new object, CR 400.7).
    pub fn encoded_creature(&self, card: ObjectId) -> Option<ObjectId> {
        let card = self
            .object(card)
            .filter(|card| card.zone() == Zone::Exile)?;
        let carrier = card.encoded_on()?;
        self.object(carrier.id)
            .filter(|creature| {
                creature.zone() == Zone::Battlefield && creature.counter() == carrier.counter
            })
            .map(GameObject::id)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a card is encoded on a creature only while it lies in exile and the creature is still the permanent it was, so any zone change of the card, and any departure of the creature (it comes back as a new object), ends the link, and the fold of the event that encodes records it and nothing else does.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::card::CardType;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::events::{Event, ZoneCause};
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;

    const ME: PlayerId = PlayerId(0);

    /// Seat 0 holds a sorcery and controls a creature; the sorcery is exiled encoded on it.
    fn encoded() -> (GameState, ObjectId, ObjectId) {
        let sorcery = Arc::new(CardDefinition::new("Veil").with_type(CardType::Sorcery));
        let mut game = state(
            vec![vec![sorcery], vec![]],
            vec![vec![creature("Carrier", 2, 2)], vec![]],
        );
        let card = game.zone(Zone::Hand(ME))[0];
        let carrier = game.zone(Zone::Battlefield)[0];
        let by = game.object(carrier).unwrap().reference();
        apply_event(
            &mut game,
            &Event::ZoneChanged {
                object: card,
                from: Zone::Hand(ME),
                to: Zone::Exile,
                cause: ZoneCause::Encoded { by },
            },
        )
        .unwrap();
        (game, card, carrier)
    }

    fn moved(game: &mut GameState, object: ObjectId, from: Zone, to: Zone) {
        let event = Event::ZoneChanged {
            object,
            from,
            to,
            cause: ZoneCause::Moved,
        };
        apply_event(game, &event).unwrap();
    }

    #[test]
    fn the_exile_of_a_card_for_cipher_links_it_to_the_creature() {
        let (game, card, carrier) = encoded();
        assert_eq!(game.encoded_creature(card), Some(carrier));
        assert_eq!(
            game.object(card).unwrap().encoded_on(),
            Some(game.object(carrier).unwrap().reference())
        );
    }

    #[rstest]
    #[case::to_a_graveyard(Zone::Graveyard(ME))]
    #[case::to_a_hand(Zone::Hand(ME))]
    #[case::to_a_library(Zone::Library(ME))]
    fn the_link_ends_when_the_card_leaves_exile(#[case] to: Zone) {
        let (mut game, card, _) = encoded();
        moved(&mut game, card, Zone::Exile, to);
        assert_eq!(game.encoded_creature(card), None);
        assert_eq!(game.object(card).unwrap().encoded_on(), None);
    }

    #[test]
    fn the_card_exiled_again_is_not_encoded_any_more() {
        let (mut game, card, _) = encoded();
        moved(&mut game, card, Zone::Exile, Zone::Hand(ME));
        moved(&mut game, card, Zone::Hand(ME), Zone::Exile);
        assert_eq!(game.encoded_creature(card), None);
    }

    #[rstest]
    #[case::it_dies(Zone::Graveyard(ME), None)]
    #[case::it_is_exiled(Zone::Exile, None)]
    #[case::it_returns_to_a_hand(Zone::Hand(ME), None)]
    #[case::it_is_put_back_and_replayed(Zone::Hand(ME), Some(Zone::Battlefield))]
    fn the_link_ends_when_the_creature_leaves_the_battlefield(
        #[case] to: Zone,
        #[case] then: Option<Zone>,
    ) {
        let (mut game, card, carrier) = encoded();
        moved(&mut game, carrier, Zone::Battlefield, to);
        if let Some(back) = then {
            moved(&mut game, carrier, to, back);
        }
        assert_eq!(game.encoded_creature(card), None);
        assert_eq!(game.object(card).unwrap().zone(), Zone::Exile);
    }
}
