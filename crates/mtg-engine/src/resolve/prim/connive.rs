//! `Effect::Connive`: creatures connive (CR 701.50).

use mtg_cards::effect::Effect;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};

use super::explore::{Actor, actors};
use super::wrong_effect;
use crate::choose::{Questions, Stop, Target};
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::replace::{Processed, process};
use crate::resolve::{Cx, ResolveError, Run};
use crate::steps::draw;
use crate::wording;
use crate::zones::Zone;

/// Each creature the effect names connives in turn, once each. Its controller (as it was, when the creature has left
/// the battlefield and the ability remembers it) draws N cards and discards N cards (asked only when the hand holds
/// more than N; N is one unless the effect says more), and a creature still on the battlefield gets one +1/+1 counter
/// for each nonland card discarded, as a proposed `CountersChanged` that a replacement effect sees. The effect is one
/// step: the cards drawn are the top cards of the library, so they can be among the cards discarded, unless a
/// replacement effect changes a draw (a commander drawn, a draw that is replaced), when the discard chooses among the
/// cards already in the hand. Each connive is a `Connived`, the draws, the `CardDiscarded`s and the counters.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Connive { what, count } = effect else {
        return Err(wrong_effect("Connive"));
    };
    let conniving = actors(cx, what)?;
    let count = usize::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(0);
    Stop::settle(connivances(cx, &conniving, count).map(Run::events))
}

/// The events of each connive N, asking in turn: N draws, then N discards (all the hand when it holds no more), and a
/// +1/+1 counter for each nonland card discarded (CR 701.50e).
fn connivances(cx: &Cx<'_>, conniving: &[Actor], count: usize) -> Result<Vec<Event>, Stop> {
    let state = cx.state();
    let mut questions = Questions::new(cx);
    let mut events = Vec::new();
    // The players who drew so far (one entry for each draw), the cards drawn that the discard may take, and the
    // cards discarded.
    let mut attempts: Vec<PlayerId> = Vec::new();
    let mut drawn: Vec<(PlayerId, ObjectId)> = Vec::new();
    let mut discarded: Vec<ObjectId> = Vec::new();
    for actor in conniving {
        let player = actor.player;
        events.push(Event::Connived {
            object: actor.object,
            player,
        });
        // Past an empty library one more draw is all that matters (CR 704.5b).
        let draws = count.min(state.zone(Zone::Library(player)).len() + 1);
        for _ in 0..draws {
            let nth = attempts.iter().filter(|who| **who == player).count() + 1;
            attempts.push(player);
            if let Some(event) = draw(state, player, nth).into_iter().nth(nth - 1) {
                let unchanged = matches!(
                    process(state, vec![event.clone()], &[]),
                    Ok(Processed::Events(found)) if found == [event.clone()]
                );
                if let (true, Event::CardDrawn { object, .. }) = (unchanged, &event) {
                    drawn.push((player, *object));
                }
                events.push(event);
            }
        }
        let hand: Vec<ObjectId> = state
            .zone(Zone::Hand(player))
            .iter()
            .copied()
            .chain(
                drawn
                    .iter()
                    .filter(|(who, _)| *who == player)
                    .map(|(_, card)| *card),
            )
            .filter(|card| !discarded.contains(card))
            .collect();
        let chosen = if count == 0 {
            Vec::new()
        } else if hand.len() <= count {
            hand
        } else {
            questions.cards(player, hand, (count, count), wording::discard(count))?
        };
        let mut nonland = 0;
        for card in chosen {
            discarded.push(card);
            events.push(Event::CardDiscarded {
                player,
                object: card,
            });
            if !state
                .object(card)
                .is_some_and(|object| object.base_types().contains(CardType::Land))
            {
                nonland += 1;
            }
        }
        if let (true, Some(holder)) = (nonland > 0, actor.permanent) {
            events.push(Event::CountersChanged(CountersChanged {
                on: Target::Object(holder),
                counter: CounterKind::PlusOnePlusOne,
                delta: nonland,
            }));
        }
    }
    questions.end()?;
    Ok(events)
}

#[cfg(test)]
mod tests {
    //! Goal: a connive N (one unless said) draws N cards and discards N, a +1/+1 counter for each nonland among them; a connive draws the top card of the controller's library and discards a card of their choice (asked only when the hand holds more than one, with the card just drawn among the choices), gives a creature still on the battlefield a +1/+1 counter when the card discarded is not a land, draws from an empty library as a draw does, takes the next card for the next creature of the same player, does nothing for a creature it cannot find, and refuses an answer it did not ask for.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use rstest::rstest;

    use super::*;
    use crate::choose::{CardsOptions, Choice};
    use crate::decision::{Decision, DecisionKind};
    use crate::fixtures::{creature, setup};
    use crate::resolve::{Next, Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    fn forest() -> Arc<CardDefinition> {
        Arc::new(CardDefinition::new("Forest").with_type(CardType::Land))
    }

    /// Seat 0 controls the Source and a Wolf (or holds the Source in hand when `on_board` is false), holds `hand` and has
    /// `library` (top first); seat 1 controls a Bear.
    fn game(
        hand: Vec<Arc<CardDefinition>>,
        library: Vec<Arc<CardDefinition>>,
        on_board: bool,
    ) -> GameState {
        let mut hand = hand;
        let mut board = vec![creature("Wolf", 2, 2)];
        if on_board {
            board.insert(0, creature("Source", 1, 1));
        } else {
            hand.push(creature("Source", 1, 1));
        }
        let mut game = setup(
            vec![hand, vec![]],
            vec![board, vec![creature("Bear", 2, 2)]],
        );
        game.players[0].library = library;
        GameState::from_setup(&game).unwrap()
    }

    fn named(game: &GameState, name: &str) -> ObjectId {
        game.objects()
            .find(|o| o.card().name == name)
            .map(crate::object::GameObject::id)
            .unwrap()
    }

    fn run_with(
        game: &GameState,
        journal: Vec<(PlayerId, Choice)>,
        json: &str,
    ) -> Result<Run, ResolveError> {
        let resolving = Resolving {
            journal,
            ..Resolving::begin(named(game, "Source"))
        };
        let source = game.object(named(game, "Source")).unwrap();
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(json).unwrap())
    }

    fn done(game: &GameState, run: Run) -> Vec<String> {
        let Run::Done {
            events,
            bound,
            snapshots,
            next,
        } = run
        else {
            panic!("the connive is done");
        };
        assert!(bound.is_empty() && snapshots.is_empty() && next == Next::Sibling);
        let name = |id: ObjectId| game.object(id).unwrap().card().name.clone();
        events
            .into_iter()
            .map(|event| match event {
                Event::Connived { object, .. } => format!("{} connives", name(object)),
                Event::CardDrawn { object, .. } => format!("draws {}", name(object)),
                Event::DrewFromEmptyLibrary { .. } => "draws from nothing".to_owned(),
                Event::CardDiscarded { object, .. } => format!("discards {}", name(object)),
                Event::CountersChanged(changed) => {
                    let Target::Object(holder) = changed.on else {
                        panic!("a creature gets it");
                    };
                    format!("{} {:+}", name(holder.id), changed.delta)
                }
                other => panic!("unexpected {other:?}"),
            })
            .collect()
    }

    fn asked(run: Run) -> CardsOptions {
        let Run::Ask(Decision {
            player,
            kind: DecisionKind::ChooseCards(options),
        }) = run
        else {
            panic!("the connive asks which card to discard");
        };
        assert_eq!(player, ME);
        options
    }

    const SOURCE: &str = r#"{"Connive":{"what":"This"}}"#;
    const BOTH: &str =
        r#"{"Connive":{"what":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}"#;

    #[rstest]
    #[case::a_nonland_drawn_and_discarded(vec![], vec![creature("Ogre", 3, 3)], &["Source connives", "draws Ogre", "discards Ogre", "Source +1"])]
    #[case::a_land_drawn_and_discarded(vec![], vec![forest()], &["Source connives", "draws Forest", "discards Forest"])]
    #[case::an_empty_library_and_one_card_in_hand(vec![creature("Bear", 2, 2)], vec![], &["Source connives", "draws from nothing", "discards Bear", "Source +1"])]
    #[case::an_empty_library_and_an_empty_hand(vec![], vec![], &["Source connives", "draws from nothing"])]
    fn a_hand_of_one_card_is_discarded_without_a_question(
        #[case] hand: Vec<Arc<CardDefinition>>,
        #[case] library: Vec<Arc<CardDefinition>>,
        #[case] expected: &[&str],
    ) {
        let game = game(hand, library, true);
        let events = done(&game, run_with(&game, vec![], SOURCE).unwrap());
        assert_eq!(events, expected);
    }

    #[rstest]
    #[case::the_card_drawn("Ogre", &["Source connives", "draws Ogre", "discards Ogre", "Source +1"])]
    #[case::a_card_of_the_hand("Bear", &["Source connives", "draws Ogre", "discards Bear", "Source +1"])]
    #[case::a_land_of_the_hand("Forest", &["Source connives", "draws Ogre", "discards Forest"])]
    fn the_player_chooses_among_the_hand_and_the_card_drawn(
        #[case] discard: &str,
        #[case] expected: &[&str],
    ) {
        let game = game(
            vec![creature("Bear", 2, 2), forest()],
            vec![creature("Ogre", 3, 3)],
            true,
        );
        let options = asked(run_with(&game, vec![], SOURCE).unwrap());
        let mut names: Vec<String> = options
            .from
            .iter()
            .map(|id| game.object(*id).unwrap().card().name.clone())
            .collect();
        names.sort();
        assert_eq!(names, ["Bear", "Forest", "Ogre"]);
        assert_eq!((options.min, options.max), (1, 1));
        let chosen = named(&game, discard);
        let events = done(
            &game,
            run_with(&game, vec![(ME, Choice::Cards(vec![chosen]))], SOURCE).unwrap(),
        );
        assert_eq!(events, expected);
    }

    #[test]
    fn the_next_creature_of_the_player_draws_the_next_card_and_cannot_discard_one_already_discarded()
     {
        let game = game(
            vec![],
            vec![creature("Ogre", 3, 3), forest(), creature("Imp", 1, 1)],
            true,
        );
        let events = done(&game, run_with(&game, vec![], BOTH).unwrap());
        assert_eq!(
            events,
            [
                "Source connives",
                "draws Ogre",
                "discards Ogre",
                "Source +1",
                "Wolf connives",
                "draws Forest",
                "discards Forest"
            ]
        );
    }

    #[test]
    fn a_creature_that_cannot_be_found_does_not_connive() {
        let game = game(vec![], vec![forest()], false);
        let events = done(&game, run_with(&game, vec![], SOURCE).unwrap());
        assert_eq!(events, Vec::<String>::new());
    }

    #[test]
    fn an_answer_that_is_not_in_the_hand_is_refused() {
        let game = game(
            vec![creature("Bear", 2, 2), forest()],
            vec![creature("Ogre", 3, 3)],
            true,
        );
        let wolf = named(&game, "Wolf");
        assert!(run_with(&game, vec![(ME, Choice::Cards(vec![wolf]))], SOURCE).is_err());
    }

    fn conniving(count: u32) -> String {
        format!(r#"{{"Connive":{{"what":"This","count":{count}}}}}"#)
    }

    #[rstest]
    #[case::two_nonlands(2, vec![creature("Ogre", 3, 3), creature("Imp", 1, 1)], &["Source connives", "draws Ogre", "draws Imp", "discards Ogre", "discards Imp", "Source +2"])]
    #[case::a_land_and_a_nonland(2, vec![creature("Ogre", 3, 3), forest()], &["Source connives", "draws Ogre", "draws Forest", "discards Ogre", "discards Forest", "Source +1"])]
    #[case::past_the_library_one_draw_from_nothing(3, vec![creature("Ogre", 3, 3)], &["Source connives", "draws Ogre", "draws from nothing", "discards Ogre", "Source +1"])]
    #[case::zero(0, vec![creature("Ogre", 3, 3)], &["Source connives"])]
    fn connive_n_draws_and_discards_n_and_counts_each_nonland(
        #[case] count: u32,
        #[case] library: Vec<Arc<CardDefinition>>,
        #[case] expected: &[&str],
    ) {
        let game = game(vec![], library, true);
        let events = done(&game, run_with(&game, vec![], &conniving(count)).unwrap());
        assert_eq!(events, expected);
    }

    #[test]
    fn connive_n_asks_for_n_cards_when_the_hand_holds_more() {
        let game = game(
            vec![creature("Bear", 2, 2)],
            vec![creature("Ogre", 3, 3), forest()],
            true,
        );
        let options = asked(run_with(&game, vec![], &conniving(2)).unwrap());
        assert_eq!((options.from.len(), options.min, options.max), (3, 2, 2));
        let chosen = vec![named(&game, "Bear"), named(&game, "Ogre")];
        let events = done(
            &game,
            run_with(&game, vec![(ME, Choice::Cards(chosen))], &conniving(2)).unwrap(),
        );
        assert_eq!(
            events,
            [
                "Source connives",
                "draws Ogre",
                "draws Forest",
                "discards Bear",
                "discards Ogre",
                "Source +2"
            ]
        );
    }
}
