//! `Effect::Encode`: the card of the resolving spell is exiled encoded on a creature (cipher, CR 702.99a).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::characteristics::seen_as;
use crate::events::{Event, ZoneCause};
use crate::object::ObjectKind;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::zones::Zone;

/// The spell card the effect names moves from the stack to exile with `ZoneCause::Encoded` naming the first creature of
/// `on` that its controller controls on the battlefield, face up, and the fold links them. It happens as the spell
/// resolves: the last step leaves the stack (`resolve::finish` then gives priority and does not move the card again).
/// A spell that is not the one resolving, a copy of a spell (no card), a creature that left the battlefield or is no
/// creature any more, and one the controller no longer controls pass the step with nothing done, so the card goes to the
/// graveyard as an instant or sorcery does.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Encode { what, on } = effect else {
        return Err(wrong_effect("Encode"));
    };
    let state = cx.state();
    let spell = cx.scope.live_objects(what)?.into_iter().find(|card| {
        card.id() == cx.resolving.object()
            && card.zone() == Zone::Stack
            && card.kind() == ObjectKind::Card
    });
    let creature = cx.scope.live_objects(on)?.into_iter().find(|creature| {
        creature.zone() == Zone::Battlefield
            && seen_as(state, creature.id()).is_some_and(|(chars, controller)| {
                chars.is_creature() && controller == cx.controller()
            })
    });
    let events = match (spell, creature) {
        (Some(spell), Some(creature)) => vec![Event::ZoneChanged {
            object: spell.id(),
            from: Zone::Stack,
            to: Zone::Exile,
            cause: ZoneCause::Encoded {
                by: creature.reference(),
            },
        }],
        _ => Vec::new(),
    };
    Ok(Run::Done {
        events,
        bound: Vec::new(),
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: encoding moves the resolving spell card from the stack to exile linked to the creature the controller controls, and does nothing for a spell that is not the one resolving, for a creature of another player and for no creature at all.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::card::CardType;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::choose::Choices;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::resolve::{Resolving, Scope};

    const ME: PlayerId = PlayerId(0);

    /// What a cast of the sorcery on a table with these creatures (mine, the opponent's) encodes, and on which.
    fn encoded(mine: usize, theirs: usize, resolving_the_spell: bool) -> Vec<Event> {
        let sorcery = Arc::new(CardDefinition::new("Veil").with_type(CardType::Sorcery));
        let battlefield = |n: usize| (0..n).map(|i| creature(&format!("C{i}"), 1, 1)).collect();
        let mut game = state(
            vec![vec![sorcery], vec![]],
            vec![battlefield(mine), battlefield(theirs)],
        );
        let card = game.zone(Zone::Hand(ME))[0];
        let cast = Event::SpellCast {
            player: ME,
            object: card,
            choices: Choices::default(),
        };
        apply_event(&mut game, &cast).unwrap();
        let resolving = if resolving_the_spell {
            Resolving::begin(card)
        } else {
            Resolving::begin(game.zone(Zone::Battlefield)[0])
        };
        let cx = Cx {
            scope: Scope::announcing(&game, ME, game.object(card).unwrap().reference()),
            resolving: &resolving,
        };
        let json = r#"{"Encode":{"what":"This","on":{"Each":[{"Type":"Creature"}]}}}"#;
        let effect: Effect = serde_json::from_str(json).unwrap();
        let Run::Done { events, next, .. } = run(&cx, &effect).unwrap() else {
            panic!("encoding asks nothing");
        };
        assert_eq!(next, Next::Sibling);
        events
    }

    #[rstest]
    #[case::a_creature_of_mine(1, 0, true, 1)]
    #[case::several_creatures_of_mine(3, 2, true, 1)]
    #[case::only_the_opponents(0, 2, true, 0)]
    #[case::none(0, 0, true, 0)]
    #[case::not_the_resolving_spell(2, 0, false, 0)]
    fn the_resolving_card_is_encoded_on_a_creature_of_its_controller(
        #[case] mine: usize,
        #[case] theirs: usize,
        #[case] resolving_the_spell: bool,
        #[case] moves: usize,
    ) {
        let events = encoded(mine, theirs, resolving_the_spell);
        assert_eq!(events.len(), moves);
        if let Some(Event::ZoneChanged {
            from, to, cause, ..
        }) = events.first()
        {
            assert_eq!((*from, *to), (Zone::Stack, Zone::Exile));
            assert!(matches!(cause, ZoneCause::Encoded { .. }));
        }
    }
}
