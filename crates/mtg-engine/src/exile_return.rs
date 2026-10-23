//! Cards exiled until a permanent leaves the battlefield (CR 610.3): each returns to the battlefield under its owner's
//! control, as a new object, once that permanent has left.

use crate::attach;
use crate::events::{Event, ZoneCause};
use crate::loyalty;
use crate::object::{GameObject, ObjectRef};
use crate::saga;
use crate::state::GameState;
use crate::zones::Zone;

/// Whether `source` is still the permanent it names: on the battlefield and not a new object since (CR 400.7).
pub(crate) fn holds(state: &GameState, source: ObjectRef) -> bool {
    state.object(source.id).is_some_and(|object| {
        object.counter() == source.counter && object.zone() == Zone::Battlefield
    })
}

/// The events that return every card in exile whose permanent has left the battlefield, all together. The duration is
/// not a triggered ability (CR 610.3a): the control loop runs this before anything else is checked, so nothing happens
/// between the permanent leaving and the card returning. An Aura with nothing to enchant stays in exile (CR 303.4g).
pub(crate) fn returns(state: &GameState) -> Vec<Event> {
    if !state.possible.linked_exile {
        return Vec::new();
    }
    state
        .zone(Zone::Exile)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|card| {
            card.monarch_released() || card.exiled_until().is_some_and(|by| !holds(state, by))
        })
        .flat_map(|card| back(state, card))
        .collect()
}

/// The events of one card coming back: the move, what it enters with and, for an Aura, the permanent it enchants.
fn back(state: &GameState, card: &GameObject) -> Vec<Event> {
    let id = card.id();
    let host = match attach::hosts_when_entering(state, card, card.owner()) {
        None => None,
        Some(hosts) => match hosts.first() {
            Some(host) => Some(*host),
            None => return Vec::new(),
        },
    };
    let mut events = vec![Event::ZoneChanged {
        object: id,
        from: Zone::Exile,
        to: Zone::Battlefield,
        cause: ZoneCause::Moved,
    }];
    events.extend(loyalty::entering(state, id));
    events.extend(saga::entering(state, id));
    events.extend(host.map(|to| Event::Attached { object: id, to }));
    events
}

#[cfg(test)]
mod tests {
    //! Goal: a card in exile linked to a permanent returns once that permanent is gone (left the battlefield, or is a new object after a flicker), not while it stays, only the card of the permanent that left returns, a card with no link or whose link was cleared by another move stays, nothing is watched in a game where no card was exiled until, and a card exiled until an opponent becomes the monarch returns once any opponent of its exiler has become it (also by the monarch leaving the game), not when its exiler holds it or leaves the battlefield.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    /// Seat 0 controls Source A and Source B; seat 1 controls Bear and Ogre. Source A holds Bear in exile when `held`.
    fn game(held: bool) -> (GameState, [ObjectRef; 4]) {
        let mut game = state(
            vec![vec![], vec![]],
            vec![
                vec![creature("A", 1, 1), creature("B", 1, 1)],
                vec![creature("Bear", 2, 2), creature("Ogre", 3, 3)],
            ],
        );
        let battlefield = game.zone(Zone::Battlefield).to_vec();
        let refs: Vec<ObjectRef> = battlefield
            .iter()
            .map(|id| game.object(*id).unwrap().reference())
            .collect();
        let [a, b, bear, ogre] = [refs[0], refs[1], refs[2], refs[3]];
        if held {
            for (card, by) in [(bear, a), (ogre, b)] {
                apply_event(
                    &mut game,
                    &Event::ZoneChanged {
                        object: card.id,
                        from: Zone::Battlefield,
                        to: Zone::Exile,
                        cause: ZoneCause::ExiledUntil { by },
                    },
                )
                .unwrap();
            }
        }
        (game, [a, b, bear, ogre])
    }

    fn leave(game: &mut GameState, object: ObjectRef) {
        apply_event(
            game,
            &Event::ZoneChanged {
                object: object.id,
                from: Zone::Battlefield,
                to: Zone::Graveyard(ME),
                cause: ZoneCause::Destroyed,
            },
        )
        .unwrap();
    }

    #[test]
    fn a_card_waits_while_its_permanent_stays() {
        let (game, [_, _, bear, ogre]) = game(true);
        assert_eq!(returns(&game).len(), 0);
        assert_eq!(game.object(bear.id).unwrap().zone(), Zone::Exile);
        assert!(game.object(ogre.id).unwrap().exiled_until().is_some());
    }

    #[rstest]
    #[case::source_a(0, 2, 3)]
    #[case::source_b(1, 3, 2)]
    fn only_the_card_of_the_permanent_that_left_returns(
        #[case] leaving: usize,
        #[case] returns_at: usize,
        #[case] stays_at: usize,
    ) {
        let (mut game, refs) = game(true);
        leave(&mut game, refs[leaving]);
        let events = returns(&game);
        assert!(
            matches!(
                events.as_slice(),
                [Event::ZoneChanged { object, from: Zone::Exile, to: Zone::Battlefield, cause: ZoneCause::Moved }]
                    if *object == refs[returns_at].id
            ),
            "{events:?}"
        );
        for event in &events {
            apply_event(&mut game, event).unwrap();
        }
        let back = game.object(refs[returns_at].id).unwrap();
        assert_eq!(back.zone(), Zone::Battlefield);
        assert_eq!(back.owner(), FOE);
        assert_eq!(back.controller(), FOE);
        assert_eq!(back.exiled_until(), None);
        assert!(back.counter() > refs[returns_at].counter + 1);
        assert_eq!(game.object(refs[stays_at].id).unwrap().zone(), Zone::Exile);
        assert_eq!(returns(&game).len(), 0, "nothing returns twice");
    }

    #[test]
    fn a_permanent_that_came_back_as_a_new_object_is_not_the_one_the_card_waits_for() {
        let (mut game, [a, ..]) = game(true);
        leave(&mut game, a);
        apply_event(
            &mut game,
            &Event::ZoneChanged {
                object: a.id,
                from: Zone::Graveyard(ME),
                to: Zone::Battlefield,
                cause: ZoneCause::Moved,
            },
        )
        .unwrap();
        assert_eq!(returns(&game).len(), 1, "the new object is not the source");
    }

    #[test]
    fn a_card_moved_from_exile_by_another_effect_is_no_longer_watched() {
        let (mut game, [a, _, bear, _]) = game(true);
        apply_event(
            &mut game,
            &Event::ZoneChanged {
                object: bear.id,
                from: Zone::Exile,
                to: Zone::Graveyard(FOE),
                cause: ZoneCause::Moved,
            },
        )
        .unwrap();
        leave(&mut game, a);
        assert_eq!(returns(&game).len(), 0);
    }

    #[test]
    fn nothing_is_watched_in_a_game_that_exiled_nothing_until() {
        let (mut game, [a, ..]) = game(false);
        leave(&mut game, a);
        assert_eq!(returns(&game).len(), 0);
        assert!(!game.possible.linked_exile);
    }

    /// `seats` seats; seat 1's Bear was exiled until an opponent of seat 0 becomes the monarch, and seat 0 is the monarch.
    fn jailed(seats: usize) -> (GameState, ObjectRef) {
        let mut battlefields = vec![vec![creature("Jailer", 1, 1)], vec![creature("Bear", 2, 2)]];
        battlefields.resize(seats, Vec::new());
        let mut game = state(vec![Vec::new(); seats], battlefields);
        let bear = game
            .object(game.zone(Zone::Battlefield)[1])
            .unwrap()
            .reference();
        apply_event(&mut game, &Event::MonarchChanged { player: ME }).unwrap();
        apply_event(
            &mut game,
            &Event::ZoneChanged {
                object: bear.id,
                from: Zone::Battlefield,
                to: Zone::Exile,
                cause: ZoneCause::ExiledUntilMonarch { by: ME },
            },
        )
        .unwrap();
        (game, bear)
    }

    #[rstest]
    #[case::the_exiler_takes_it_again(2, 0, false)]
    #[case::the_only_opponent(2, 1, true)]
    #[case::an_opponent_among_three(3, 2, true)]
    #[case::the_first_opponent_among_three(3, 1, true)]
    #[case::the_last_of_four(4, 3, true)]
    #[case::the_exiler_among_four(4, 0, false)]
    fn a_card_exiled_until_an_opponent_becomes_the_monarch_returns_when_one_does(
        #[case] seats: usize,
        #[case] becomes: usize,
        #[case] returns_now: bool,
    ) {
        let (mut game, bear) = jailed(seats);
        assert_eq!(
            returns(&game).len(),
            0,
            "it waits while the exiler holds it"
        );
        assert_eq!(game.object(bear.id).unwrap().zone(), Zone::Exile);
        apply_event(
            &mut game,
            &Event::MonarchChanged {
                player: PlayerId::from_index(becomes),
            },
        )
        .unwrap();
        let events = returns(&game);
        assert_eq!(events.len(), usize::from(returns_now), "{events:?}");
        for event in &events {
            apply_event(&mut game, event).unwrap();
        }
        let back = game.object(bear.id).unwrap();
        assert_eq!(back.zone() == Zone::Battlefield, returns_now);
        assert_eq!(returns(&game).len(), 0, "nothing returns twice");
    }

    #[test]
    fn the_exiler_leaving_the_battlefield_does_not_return_the_card() {
        let (mut game, bear) = jailed(3);
        let jailer = game.zone(Zone::Battlefield)[0];
        let jailer = game.object(jailer).unwrap().reference();
        leave(&mut game, jailer);
        assert_eq!(returns(&game).len(), 0);
        assert_eq!(game.object(bear.id).unwrap().zone(), Zone::Exile);
    }

    #[test]
    fn a_released_card_still_returns_after_the_designation_moves_on() {
        let (mut game, bear) = jailed(2);
        apply_event(&mut game, &Event::MonarchChanged { player: FOE }).unwrap();
        assert_eq!(returns(&game).len(), 1);
        apply_event(&mut game, &Event::MonarchChanged { player: ME }).unwrap();
        // The designation back with its exiler does not take the release back.
        assert_eq!(returns(&game).len(), 1);
        assert_eq!(game.object(bear.id).unwrap().zone(), Zone::Exile);
    }

    #[test]
    fn a_monarch_who_leaves_the_game_passes_it_to_an_opponent_who_frees_the_card() {
        let (mut game, bear) = jailed(3);
        let lost = Event::PlayerLost {
            player: ME,
            reason: crate::player::LossReason::Conceded,
        };
        apply_event(&mut game, &lost).unwrap();
        assert_eq!(game.monarch(), Some(FOE));
        assert_eq!(returns(&game).len(), 1);
        assert_eq!(game.object(bear.id).unwrap().zone(), Zone::Exile);
    }

    #[test]
    fn a_card_moved_on_before_the_designation_moves_stays_where_it_is() {
        let (mut game, bear) = jailed(2);
        apply_event(
            &mut game,
            &Event::ZoneChanged {
                object: bear.id,
                from: Zone::Exile,
                to: Zone::Graveyard(FOE),
                cause: ZoneCause::Moved,
            },
        )
        .unwrap();
        apply_event(&mut game, &Event::MonarchChanged { player: FOE }).unwrap();
        assert_eq!(returns(&game).len(), 0);
    }

    #[test]
    fn the_wait_is_part_of_the_state_hash() {
        let (waiting, _) = jailed(2);
        let (mut released, _) = jailed(2);
        assert_eq!(waiting.state_hash(), released.state_hash());
        apply_event(&mut released, &Event::MonarchChanged { player: FOE }).unwrap();
        assert_ne!(waiting.state_hash(), released.state_hash());
    }
}
