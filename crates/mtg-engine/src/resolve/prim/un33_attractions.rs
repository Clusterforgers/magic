//! `Effect::OpenAttraction` and `Effect::RollToVisit`: the first player named who is in the game opens Attractions
//! (CR 701.51) or rolls to visit theirs (CR 701.52).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::resolve::{Cx, ResolveError, Run};

/// Opens the Attractions or rolls to visit them; a player with no Attraction deck opens nothing (CR 701.51a).
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let (who, count) = match effect {
        Effect::OpenAttraction { who, count } => (who, Some(count)),
        Effect::RollToVisit { who } => (who, None),
        _ => return Err(wrong_effect("OpenAttraction")),
    };
    let state = cx.state();
    let Some(player) = cx
        .scope
        .players(who)?
        .into_iter()
        .find(|player| state.is_alive(*player))
    else {
        return Ok(Run::events(Vec::new()));
    };
    Ok(Run::events(match count {
        Some(count) => {
            let count = usize::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(0);
            crate::un33_attractions::open_events(state, player, count)
        }
        None => crate::un33_attractions::roll_events(state, player),
    }))
}

#[cfg(test)]
mod tests {
    //! Goal: opening puts the top cards of the player's Attraction deck onto the battlefield face up and out of the deck, as many as the count and no more than the deck holds, a player with no deck opens nothing; rolling to visit rolls a die and visits exactly the Attractions the player controls with the result lit up, counted for the turn; an Attraction card that would go to a graveyard, hand or library goes to the junkyard in the command zone instead, and the decks are shuffled before the game.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::attraction::Lights;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::events::{Event, ZoneCause};
    use crate::fixtures::{creature, setup};
    use crate::resolve::{Resolving, Scope};
    use crate::state::{GameState, Tally};
    use crate::un33_attractions::AttractionCard;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);

    fn attraction(name: &str) -> Arc<CardDefinition> {
        let mut card = (*creature(name, 0, 0)).clone();
        card.types = std::iter::once(mtg_core::card::CardType::Artifact).collect();
        card.subtypes = vec![mtg_core::card::Subtype("Attraction".to_owned())];
        card.power = None;
        card.toughness = None;
        Arc::new(card)
    }

    /// A game where the first seat brings an Attraction deck of `deck` cards, each lit up at `lit`.
    fn game(deck: usize, lit: &[u32], seed: u64) -> GameState {
        let mut setup = setup(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        setup.players[0].attractions = (0..deck)
            .map(|at| AttractionCard {
                card: attraction(&format!("Ride {at}")),
                lights: Lights::of(lit.iter().copied()),
            })
            .collect();
        let mut game = GameState::from_setup(&setup).unwrap();
        game.seed = seed;
        game
    }

    fn run_on(game: &GameState, json: &str) -> Vec<Event> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        match run(&cx, &serde_json::from_str(json).unwrap()).unwrap() {
            Run::Done { events, .. } => events,
            other => panic!("asks: {other:?}"),
        }
    }

    fn after(game: &GameState, events: &[Event]) -> GameState {
        let mut game = game.clone();
        for event in events {
            crate::fold::apply_event(&mut game, event).unwrap();
        }
        game
    }

    #[rstest]
    #[case::one_of_three(3, 1, 1)]
    #[case::two_of_three(3, 2, 2)]
    #[case::more_than_the_deck(2, 5, 2)]
    #[case::no_deck(0, 1, 0)]
    fn opening_moves_the_top_cards_onto_the_battlefield(
        #[case] deck: usize,
        #[case] count: u32,
        #[case] opened: usize,
    ) {
        let game = game(deck, &[2, 6], 1);
        let top: Vec<_> = game
            .attraction_deck(ME)
            .iter()
            .rev()
            .take(opened)
            .copied()
            .collect();
        let events = run_on(
            &game,
            &format!(r#"{{"OpenAttraction":{{"count":{count}}}}}"#),
        );
        assert_eq!(events.len(), opened);
        let game = after(&game, &events);
        for id in &top {
            let object = game.object(*id).unwrap();
            assert_eq!(object.zone(), Zone::Battlefield);
            assert_eq!(object.face_down(), None);
            assert!(!game.in_attraction_deck(*id));
        }
        assert_eq!(game.attraction_deck(ME).len(), deck - opened);
        for id in game.attraction_deck(ME) {
            assert!(game.object(*id).unwrap().face_down().is_some());
            assert!(!game.is_shown_to(PlayerId(1), *id) && !game.may_look_at(ME, *id));
        }
    }

    #[test]
    fn the_roll_visits_exactly_the_attractions_with_the_result_lit_up() {
        let mut seen = [false; 7];
        for seed in 0..48 {
            let mut game = game(2, &[2, 6], seed);
            let opened = run_on(&game, r#"{"OpenAttraction":{"count":2}}"#);
            game = after(&game, &opened);
            let events = run_on(&game, r#"{"RollToVisit":{}}"#);
            let [
                Event::DieRolled {
                    result, sides: 6, ..
                },
                Event::RolledToVisit {
                    result: visit,
                    visited,
                    ..
                },
            ] = events.as_slice()
            else {
                panic!("{events:?}");
            };
            assert_eq!(result, visit);
            seen[usize::try_from(*result).unwrap()] = true;
            let lit = matches!(result, 2 | 6);
            assert_eq!(visited.len(), if lit { 2 } else { 0 }, "rolled {result}");
            let game = after(&game, &events);
            assert_eq!(
                game.tallied(ME, Tally::AttractionsVisited),
                if lit { 2 } else { 0 }
            );
        }
        assert!(seen[2] && seen[6] && seen[1], "the seeds roll every face");
    }

    #[rstest]
    #[case::destroyed(Zone::Graveyard(ME), ZoneCause::Destroyed)]
    #[case::bounced(Zone::Hand(ME), ZoneCause::Moved)]
    #[case::tucked(Zone::Library(ME), ZoneCause::Moved)]
    fn an_attraction_card_goes_to_the_junkyard(#[case] to: Zone, #[case] cause: ZoneCause) {
        let mut game = game(1, &[3], 1);
        game = after(&game, &run_on(&game, r#"{"OpenAttraction":{}}"#));
        let ride = *game
            .zone(Zone::Battlefield)
            .iter()
            .find(|id| game.lights(**id).is_some())
            .unwrap();
        let _ = cause;
        assert_eq!(
            crate::un33_attractions::junkyard(&game, ride, to),
            Some(Zone::Command(ME))
        );
        assert_eq!(
            crate::un33_attractions::junkyard(&game, ride, Zone::Exile),
            None
        );
        let source = game.zone(Zone::Battlefield)[0];
        assert_eq!(crate::un33_attractions::junkyard(&game, source, to), None);
    }

    #[test]
    fn the_decks_are_shuffled_before_the_game() {
        let game = game(10, &[2], 3);
        let events = crate::un33_attractions::shuffles(&game, 3);
        let [Event::AttractionDeckShuffled { player, order }] = events.as_slice() else {
            panic!("{events:?}");
        };
        assert_eq!(*player, ME);
        let mut sorted = order.clone();
        sorted.sort_unstable();
        let mut deck = game.attraction_deck(ME).to_vec();
        deck.sort_unstable();
        assert_eq!(sorted, deck);
        let game = after(&game, &events);
        assert_eq!(game.attraction_deck(ME), order.as_slice());
    }
}
