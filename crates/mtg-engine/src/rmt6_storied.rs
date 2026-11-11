//! Storied: an enduring story, a designation a player keeps for the rest of the game once storied gives it.
//!
//! A permanent with storied gives its controller an enduring story as soon as they control three or more permanents
//! that are artifacts, legendary or Sagas ("artifacts, legendaries, and/or Sagas"), checked with the state-based actions
//! as ascend gives the city's blessing. A permanent that is several of those counts once. Nothing takes the story away.

use mtg_cards::keyword::Keyword;
use mtg_core::card::{CardType, Subtype, Supertype};
use mtg_core::ids::{ObjectId, PlayerId};

use crate::characteristics::{characteristics, controller_of_object, has_supertype};
use crate::events::Event;
use crate::keywords::has_keyword;
use crate::state::GameState;
use crate::zones::Zone;

/// How many artifacts, legendaries and Sagas a player controls to get an enduring story.
const STORY: usize = 3;

impl GameState {
    /// Whether the player has an enduring story.
    pub fn has_enduring_story(&self, player: PlayerId) -> bool {
        self.enduring_story.contains(&player)
    }
}

/// Whether the permanent is an artifact, legendary or a Saga.
fn tells(state: &GameState, id: ObjectId) -> bool {
    let Some(object) = state.object(id) else {
        return false;
    };
    let printed = &object.card().subtypes;
    characteristics(state, id).is_some_and(|c| {
        c.types.contains(CardType::Artifact)
            || has_supertype(state, id, Supertype::Legendary)
            || c.has_subtype(printed, &Subtype("Saga".to_owned()))
    })
}

/// The artifacts, legendaries and Sagas `player` controls now.
fn told_by(state: &GameState, player: PlayerId) -> usize {
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            state
                .object(**id)
                .is_some_and(|object| controller_of_object(state, object) == player)
                && tells(state, **id)
        })
        .count()
}

/// What one pass of the state-based actions gives: an enduring story to each player who controls a permanent with
/// storied and three or more artifacts, legendaries and Sagas, and has none.
pub(crate) fn sync(state: &GameState) -> Vec<Event> {
    if !state.possible.storied {
        return Vec::new();
    }
    let storied: Vec<ObjectId> = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| has_keyword(state, *id, &Keyword::Storied))
        .collect();
    state
        .alive_players()
        .filter(|player| {
            !state.has_enduring_story(*player)
                && storied.iter().any(|id| {
                    state
                        .object(*id)
                        .is_some_and(|object| controller_of_object(state, object) == *player)
                })
                && told_by(state, *player) >= STORY
        })
        .map(|player| Event::EnduringStoryGained { player })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Goal: a player who controls a permanent with storied and three artifacts, legendaries or Sagas gets an enduring story once and keeps it, at two to four seats; two of them, three without storied, plain creatures, or another seat's permanents give nothing.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;

    const ME: PlayerId = PlayerId(0);

    fn storied() -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Teller")
                .with_type(CardType::Creature)
                .with_power_toughness(1, 1)
                .with_rules(Ability::Keyword(Keyword::Storied)),
        )
    }

    fn piece(kind: &str) -> Arc<CardDefinition> {
        let card = CardDefinition::new(kind);
        Arc::new(match kind {
            "artifact" => card.with_type(CardType::Artifact),
            "legend" => card
                .with_type(CardType::Creature)
                .with_supertype(Supertype::Legendary)
                .with_power_toughness(2, 2),
            "saga" => card.with_type(CardType::Enchantment).with_subtype("Saga"),
            "legendary artifact" => card
                .with_type(CardType::Artifact)
                .with_supertype(Supertype::Legendary),
            _ => card
                .with_type(CardType::Creature)
                .with_power_toughness(2, 2),
        })
    }

    /// Seat 0 of `seats` controls the pieces, after a storied creature when `teller`; the other seats control the
    /// pieces `theirs` names.
    fn game(seats: usize, teller: bool, mine: &[&str], theirs: &[&str]) -> GameState {
        let mut boards: Vec<Vec<Arc<CardDefinition>>> = vec![Vec::new(); seats];
        if teller {
            boards[0].push(storied());
        }
        boards[0].extend(mine.iter().map(|kind| piece(kind)));
        for board in boards.iter_mut().skip(1) {
            board.extend(theirs.iter().map(|kind| piece(kind)));
        }
        state(vec![Vec::new(); seats], boards)
    }

    #[rstest]
    fn three_artifacts_legendaries_or_sagas_tell_a_story(
        #[values(2, 3, 4)] seats: usize,
        #[values(
            (true, &["artifact", "legend", "saga"][..], true),
            (true, &["artifact", "artifact", "artifact", "saga"][..], true),
            (true, &["artifact", "legend"][..], false),
            (true, &["legendary artifact", "legendary artifact"][..], false),
            (true, &["bear", "bear", "bear"][..], false),
            (false, &["artifact", "legend", "saga"][..], false)
        )]
        case: (bool, &[&str], bool),
    ) {
        let (teller, mine, gets) = case;
        let game = game(seats, teller, mine, &["artifact", "artifact", "artifact"]);
        let expected: Vec<Event> = gets
            .then_some(Event::EnduringStoryGained { player: ME })
            .into_iter()
            .collect();
        assert_eq!(sync(&game), expected);
    }

    #[rstest]
    fn the_story_is_kept_and_given_once(#[values(2, 4)] seats: usize) {
        let mut game = game(seats, true, &["artifact", "legend", "saga"], &[]);
        let gained = sync(&game);
        apply_event(&mut game, &gained[0]).unwrap();
        assert!(game.has_enduring_story(ME));
        assert!((1..seats).all(|seat| !game.has_enduring_story(PlayerId::from_index(seat))));
        assert_eq!(sync(&game), []);
    }

    #[test]
    fn the_permanents_of_another_seat_do_not_count() {
        let board: Vec<_> = (0..3)
            .map(|n| creature(&format!("Bear {n}"), 2, 2))
            .collect();
        let mut theirs = board;
        theirs.push(piece("artifact"));
        theirs.push(piece("artifact"));
        theirs.push(piece("artifact"));
        let game = state(vec![vec![], vec![]], vec![vec![storied()], theirs]);
        assert_eq!(sync(&game), []);
    }
}
