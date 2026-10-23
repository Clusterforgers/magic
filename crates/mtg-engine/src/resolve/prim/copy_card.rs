//! `Effect::CopyCard`: a card in exile is copied into a token card the player may cast (CR 707.12).

use std::sync::Arc;

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::choose::Target;
use crate::copies::CardCopy;
use crate::events::Event;
use crate::object::{ObjectKind, ObjectRef};
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::zones::Zone;

/// Each card the effect names that still lies face up in exile is copied once, and the copies are bound as the objects
/// they become: token cards in exile, owned by the player `who` names (the controller by default). The copy is not
/// cast; the step that follows (`CastFromZone`) asks whether to cast it, and a copy that is not cast ceases to exist at
/// the next state-based check (CR 704.5e). A card that left exile, a card that lies face down (its owner may look at it,
/// the others may not) and a token are passed over.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::CopyCard { who, what, bind } = effect else {
        return Err(wrong_effect("CopyCard"));
    };
    let state = cx.state();
    let Some(owner) = cx
        .scope
        .players(who)?
        .into_iter()
        .find(|player| state.is_alive(*player))
    else {
        return Ok(Run::events(Vec::new()));
    };
    let cards: Vec<_> = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|card| {
            card.zone() == Zone::Exile
                && card.kind() == ObjectKind::Card
                && card.face_down().is_none()
        })
        .collect();
    let mut events = Vec::with_capacity(cards.len());
    let mut copies = Vec::with_capacity(cards.len());
    for (id, card) in (state.next_object().get()..).zip(cards) {
        let object = mtg_core::ids::ObjectId(id);
        events.push(Event::CardCopied(Box::new(CardCopy {
            object,
            of: card.reference(),
            owner,
            definition: Arc::new(card.card().copiable()),
        })));
        // A new object: its zone-change counter starts at 0.
        copies.push(Target::Object(ObjectRef {
            id: object,
            counter: 0,
        }));
    }
    Ok(Run::Done {
        events,
        bound: vec![(bind.clone(), copies)],
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: copying makes one token card in exile for each face-up card of exile the effect names and binds the copies as new objects, passes over what lies elsewhere or face down, and the fold makes a castable copy of the card that ceases to exist when it is anywhere but on the stack.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::card::CardType;
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::*;
    use crate::events::ZoneCause;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    /// Seat 0 holds a sorcery and controls a creature.
    fn game() -> GameState {
        let sorcery = Arc::new(CardDefinition::new("Veil").with_type(CardType::Sorcery));
        state(
            vec![vec![sorcery], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        )
    }

    fn sent(game: &mut GameState, to: Zone, cause: ZoneCause) -> ObjectId {
        let card = game.zone(Zone::Hand(ME))[0];
        let event = Event::ZoneChanged {
            object: card,
            from: Zone::Hand(ME),
            to,
            cause,
        };
        apply_event(game, &event).unwrap();
        card
    }

    fn copied(game: &GameState) -> (Vec<Event>, Vec<(String, Vec<Target>)>) {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        let json = r#"{"CopyCard":{"what":{"Each":[{"In":"Exile"}]},"as":"copy"}}"#;
        let effect: Effect = serde_json::from_str(json).unwrap();
        let Run::Done {
            events,
            bound,
            next,
            ..
        } = run(&cx, &effect).unwrap()
        else {
            panic!("copying asks nothing");
        };
        assert_eq!(next, Next::Sibling);
        (events, bound)
    }

    #[rstest]
    #[case::a_face_up_card_in_exile(Zone::Exile, ZoneCause::Exiled, 1)]
    #[case::a_card_face_down_in_exile(Zone::Exile, ZoneCause::Foretold, 0)]
    #[case::a_card_in_a_graveyard(Zone::Graveyard(ME), ZoneCause::Moved, 0)]
    fn only_a_face_up_card_in_exile_is_copied(
        #[case] to: Zone,
        #[case] cause: ZoneCause,
        #[case] copies: usize,
    ) {
        let mut game = game();
        sent(&mut game, to, cause);
        let (events, bound) = copied(&game);
        assert_eq!(events.len(), copies);
        assert_eq!(bound.len(), 1);
        assert_eq!(bound[0].0, "copy");
        assert_eq!(bound[0].1.len(), copies);
    }

    #[test]
    fn the_copy_is_a_castable_token_card_that_ceases_to_exist_elsewhere() {
        let mut game = game();
        let card = sent(&mut game, Zone::Exile, ZoneCause::Exiled);
        let (events, bound) = copied(&game);
        for event in &events {
            apply_event(&mut game, event).unwrap();
        }
        let Some(Target::Object(made)) = bound[0].1.first().copied() else {
            panic!("one copy is bound");
        };
        let copy = game.object(made.id).unwrap();
        assert_eq!(copy.counter(), made.counter);
        assert_eq!(copy.zone(), Zone::Exile);
        assert_eq!(copy.owner(), ME);
        assert!(copy.is_card_copy());
        assert_eq!(copy.card().name, "Veil");
        assert_eq!(
            copy.copied_from(),
            Some(game.object(card).unwrap().reference())
        );
        apply_event(&mut game, &Event::CopyCeased { object: made.id }).unwrap();
        assert!(game.object(made.id).is_none() && game.has_ceased(made.id));
        assert!(game.zone(Zone::Exile).contains(&card));
    }
}
