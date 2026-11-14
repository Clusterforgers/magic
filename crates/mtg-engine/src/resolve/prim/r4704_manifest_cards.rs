//! `Effect::ManifestCards`: named cards, a face-down pile shuffled first, are put onto the battlefield face down as 2/2
//! creatures under the control of the player who acts (manifest, cloak: CR 701.40a, 701.58a).

use mtg_cards::effect::Effect;
use mtg_core::ids::ObjectId;

use super::wrong_effect;
use crate::choose::Target;
use crate::events::{Event, ZoneCause};
use crate::mulligan::shuffled;
use crate::object::ObjectRef;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::zones::Zone;

/// The first player `who` names who is in the game takes every card `what` names that is a card outside the
/// battlefield and the stack, in an order drawn from the next substream of the generator (logged as `ChosenAtRandom`),
/// and puts each onto the battlefield face down under their control: one `ZoneChanged` whose cause says who manifested
/// or cloaked it, and an `Entered` after it when it enters tapped or under a player who does not own it. The cards are
/// bound to the name, when the effect has one, as the permanents they become (CR 400.7).
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::ManifestCards {
        who,
        what,
        cloak,
        tapped,
        bind,
    } = effect
    else {
        return Err(wrong_effect("ManifestCards"));
    };
    let state = cx.state();
    let Some(player) = cx
        .scope
        .players(who)?
        .into_iter()
        .find(|player| state.is_alive(*player))
    else {
        return Ok(Run::Done {
            events: Vec::new(),
            bound: Vec::new(),
            snapshots: Vec::new(),
            next: Next::Sibling,
        });
    };
    let mut cards: Vec<ObjectId> = Vec::new();
    for target in cx.scope.objects(what)? {
        let Some(object) = state.object(target.id) else {
            continue;
        };
        let away = matches!(object.zone(), Zone::Battlefield | Zone::Stack);
        if object.kind().is_card() && !away && !cards.contains(&target.id) {
            cards.push(target.id);
        }
    }
    let mut events = Vec::new();
    if cards.len() > 1 {
        cards = shuffled(state, 0, cards);
        events.push(Event::ChosenAtRandom {
            cards: cards.clone(),
        });
    }
    let mut made = Vec::new();
    for id in cards {
        let Some(card) = state.object(id) else {
            continue;
        };
        events.push(Event::ZoneChanged {
            object: id,
            from: card.zone(),
            to: Zone::Battlefield,
            cause: if *cloak {
                ZoneCause::Cloaked { by: player }
            } else {
                ZoneCause::Manifested { by: player }
            },
        });
        if *tapped || card.owner() != player {
            events.push(Event::Entered {
                object: id,
                tapped: *tapped,
                controller: player,
                chosen: None,
            });
        }
        made.push(Target::Object(ObjectRef {
            id,
            counter: card.reference().counter + 1,
        }));
    }
    Ok(Run::Done {
        events,
        bound: bind
            .iter()
            .map(|name| (name.clone(), made.clone()))
            .collect(),
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: manifesting or cloaking named cards puts every card among them that is outside the battlefield and the stack onto the battlefield face down under the acting player, in an order drawn at random and logged first, tapped when the effect says so, with an `Entered` for a card another player owns, passes over permanents, and binds what they became; at two to four seats.
    use rstest::rstest;

    use super::*;
    use crate::characteristics::{characteristics, controller_of};
    use crate::fixtures::{creature, state};
    use crate::object::FaceDown;
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use mtg_core::ids::PlayerId;

    const ME: PlayerId = PlayerId(0);

    fn game(seats: usize) -> GameState {
        let hand = |names: &[&str]| names.iter().map(|n| creature(n, 1, 1)).collect();
        let mut hands = vec![hand(&["A", "B"])];
        hands.resize_with(seats, || hand(&["D"]));
        let mut battlefields = vec![vec![creature("Source", 3, 3)]];
        battlefields.resize(seats, vec![]);
        state(hands, battlefields)
    }

    fn run_json(game: &GameState, json: &str) -> (Vec<Event>, Vec<(String, Vec<Target>)>) {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        let effect: Effect = serde_json::from_str(json).unwrap();
        let Run::Done { events, bound, .. } = run(&cx, &effect).unwrap() else {
            panic!("manifesting cards asks nothing");
        };
        (events, bound)
    }

    #[rstest]
    #[case::manifested(
        r#"{"ManifestCards":{"what":{"Each":[{"In":"Hand"}]},"as":"made"}}"#,
        FaceDown::Manifest,
        false
    )]
    #[case::cloaked_tapped(r#"{"ManifestCards":{"what":{"Each":[{"In":"Hand"}]},"cloak":true,"tapped":true,"as":"made"}}"#, FaceDown::Cloak, true)]
    fn the_named_cards_come_face_down_under_the_acting_player(
        #[case] json: &str,
        #[case] kind: FaceDown,
        #[case] tapped: bool,
        #[values(2, 3, 4)] seats: usize,
    ) {
        let mut game = game(seats);
        let source = game.zone(Zone::Battlefield)[0];
        let mut cards: Vec<ObjectId> = Vec::new();
        for seat in 0..seats {
            cards.extend(
                game.zone(Zone::Hand(PlayerId::from_index(seat)))
                    .iter()
                    .copied(),
            );
        }
        let (events, bound) = run_json(&game, json);
        let Some(Event::ChosenAtRandom { cards: order }) = events.first() else {
            panic!("the order is drawn first: {events:?}");
        };
        let mut drawn = order.clone();
        drawn.sort_unstable();
        let mut wanted = cards.clone();
        wanted.sort_unstable();
        assert_eq!(drawn, wanted, "every card in a hand once");
        for event in &events {
            crate::fold::apply_event(&mut game, event).unwrap();
        }
        for card in &cards {
            let object = game.object(*card).unwrap();
            assert_eq!(object.zone(), Zone::Battlefield);
            assert_eq!(object.face_down(), Some(kind));
            assert_eq!(controller_of(&game, *card), Some(ME));
            assert_eq!(object.permanent().unwrap().tapped, tapped);
            let chars = characteristics(&game, *card).unwrap();
            assert_eq!((chars.power, chars.toughness), (Some(2), Some(2)));
        }
        assert_eq!(game.object(source).unwrap().face_down(), None);
        assert_eq!(bound[0].0, "made");
        assert_eq!(bound[0].1.len(), cards.len());
    }
}
