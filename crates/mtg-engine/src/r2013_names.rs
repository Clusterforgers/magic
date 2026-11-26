//! Every name of a nonlegendary creature card (`Modification::R2013AllCreatureNames`, CR 201.2, the Spy Kit rulings): an
//! object with it passes a name check for the name of any nonlegendary creature card, front or back face, and shares a
//! name with every object that has such a name; a token's name and a name of a card that only became a creature do not
//! count. The engine has no card catalogue, so the cards it knows are those of the game and those it may conjure.

use mtg_cards::CardDefinition;
use mtg_core::card::{CardType, Supertype};

use crate::object::ObjectKind;
use crate::state::GameState;

/// Whether the face is a nonlegendary creature card's face called `name`.
fn creature_face(face: &CardDefinition, name: &str) -> bool {
    face.name == name
        && face.types.contains(&CardType::Creature)
        && !face.supertypes.contains(&Supertype::Legendary)
}

/// Whether `card`, or one of its faces, is a nonlegendary creature card called `name`.
fn names_creature(card: &CardDefinition, name: &str) -> bool {
    creature_face(card, name) || card.faces.iter().any(|face| creature_face(face, name))
}

/// Whether `name` is the name of a nonlegendary creature card the game knows.
pub(crate) fn r2013_creature_name(state: &GameState, name: &str) -> bool {
    !name.is_empty()
        && (state
            .objects()
            .filter(|object| object.kind() == ObjectKind::Card)
            .any(|object| names_creature(object.card(), name))
            || state
                .conjurable
                .values()
                .any(|card| names_creature(card, name)))
}

/// Whether two objects share a name (CR 201.2a), given each one's name and whether it has every nonlegendary creature
/// card's name: two such objects always do.
pub(crate) fn r2013_share(
    state: &GameState,
    (mine, all_mine): (Option<&str>, bool),
    (theirs, all_theirs): (Option<&str>, bool),
) -> bool {
    match (mine, theirs) {
        _ if all_mine && all_theirs => true,
        (Some(mine), Some(theirs)) if mine == theirs => true,
        (_, Some(theirs)) if all_mine => r2013_creature_name(state, theirs),
        (Some(mine), _) if all_theirs => r2013_creature_name(state, mine),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a name counts as a nonlegendary creature card's name only for a card of the game that is one, on either
    //! face, and two objects share a name by their names or by having every such name.
    #![allow(clippy::unwrap_used)]
    use std::sync::Arc;

    use super::*;
    use crate::fixtures::{creature, state};
    use rstest::rstest;

    fn legend(name: &str) -> Arc<CardDefinition> {
        let mut card = (*creature(name, 2, 2)).clone();
        card.supertypes.insert(Supertype::Legendary);
        Arc::new(card)
    }

    fn game() -> GameState {
        let land = Arc::new(CardDefinition::basic_land(None));
        state(
            vec![vec![creature("Bear", 2, 2), legend("Hero")], vec![]],
            vec![vec![land], vec![]],
        )
    }

    #[rstest]
    #[case("Bear", true)]
    #[case("Hero", false)]
    #[case("Wastes", false)]
    #[case("Unknown", false)]
    #[case("", false)]
    fn a_creature_name_is_one_of_a_nonlegendary_creature_card(
        #[case] name: &str,
        #[case] expected: bool,
    ) {
        assert_eq!(r2013_creature_name(&game(), name), expected);
    }

    #[rstest]
    #[case((Some("Bear"), false), (Some("Bear"), false), true)]
    #[case((Some("Kit"), true), (Some("Bear"), false), true)]
    #[case((Some("Bear"), false), (Some("Kit"), true), true)]
    #[case((Some("Kit"), true), (Some("Hero"), false), false)]
    #[case((Some("Kit"), true), (None, true), true)]
    #[case((Some("Kit"), false), (Some("Bear"), false), false)]
    fn two_objects_share_a_name(
        #[case] mine: (Option<&str>, bool),
        #[case] theirs: (Option<&str>, bool),
        #[case] expected: bool,
    ) {
        assert_eq!(r2013_share(&game(), mine, theirs), expected);
    }
}
