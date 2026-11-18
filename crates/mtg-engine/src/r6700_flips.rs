//! Facts of pack r67-00. The first set of coin flips each player makes each turn (Edgar, King of Figaro: "The first time you flip one or more
//! coins each turn, those coins come up heads and you win those flips"), folded from `CoinFlipped`. Per Edgar's ruling,
//! every coin one instruction flips is one set ("Flip five coins" of Ral Zarek): the set is the flips the player makes
//! for one spell or ability as it resolves; a "flip until you lose a flip" puts only its first flip in the set.

use mtg_core::ids::{ObjectId, PlayerId};

use crate::state::GameState;

/// Notes a coin `player` flipped: the first one of the turn opens the player's set for the resolving spell or ability.
pub(crate) fn r6700_note_flip(state: &mut GameState, player: PlayerId) {
    let turn = state.turn().number;
    let resolving = state.resolving().map(|resolving| resolving.object);
    let entry = state
        .r6700_flip_sets
        .entry(player)
        .or_insert((turn, resolving));
    if entry.0 != turn {
        *entry = (turn, resolving);
    }
}

/// Whether a coin `player` is about to flip for the resolving object `resolving` is in their first set of the turn:
/// they flipped no coin yet this turn, or the set is this resolution's and the flip is not a repeat of a flip until a
/// loss (`until`), which flips one coin at a time.
pub(crate) fn r6700_in_first_set(
    state: &GameState,
    player: PlayerId,
    resolving: ObjectId,
    until: bool,
) -> bool {
    match state.r6700_flip_sets.get(&player) {
        Some((turn, object)) if *turn == state.turn().number => {
            !until && *object == Some(resolving)
        }
        _ => true,
    }
}

/// Whether the coin `player` is about to flip comes up heads and is won by a rule (`Rule::R6700FirstFlipsHeads`).
pub(crate) fn r6700_heads_won(
    state: &GameState,
    player: PlayerId,
    resolving: ObjectId,
    until: bool,
) -> bool {
    crate::rules::r6700_first_heads(state, player)
        && r6700_in_first_set(state, player, resolving, until)
}

/// Whether a static ability keeps the last loyalty counter on the planeswalker `id` against damage that would remove all
/// of them (`Modification::R6700KeepsLastLoyalty`, Deification).
pub(crate) fn r6700_keeps_last_loyalty(state: &GameState, id: ObjectId) -> bool {
    state.layers().get(id).is_some_and(|held| {
        held.applied.iter().any(|applied| {
            matches!(
                applied.modification,
                crate::effects::Modification::R6700KeepsLastLoyalty
            )
        })
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a player's first set of coin flips of a turn is the flips made for one resolution (every coin of "flip five
    //! coins", the ruling of Edgar, King of Figaro), only the first flip of a flip until a loss, and none of a later
    //! resolution of the same turn; a new turn opens a new set, for each player apart, at two to four seats.
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::r6700_in_first_set;
    use crate::fixtures::state;
    use crate::state::GameState;

    fn game(seats: usize) -> GameState {
        state(vec![vec![]; seats], vec![vec![]; seats])
    }

    #[rstest]
    #[case::no_flip_yet(None, 7, false, true)]
    #[case::the_same_resolution(Some((5, 7)), 7, false, true)]
    #[case::a_repeat_of_a_flip_until_a_loss(Some((5, 7)), 7, true, false)]
    #[case::another_resolution(Some((5, 7)), 8, false, false)]
    #[case::a_set_of_an_earlier_turn(Some((4, 7)), 8, true, true)]
    fn r6700_the_first_set_is_one_resolution_of_the_turn(
        #[values(2, 3, 4)] seats: usize,
        #[case] set: Option<(u32, u64)>,
        #[case] resolving: u64,
        #[case] until: bool,
        #[case] first: bool,
    ) {
        let mut game = game(seats);
        game.turn.number = 5;
        let flipper = PlayerId(u64::try_from(seats - 1).unwrap());
        if let Some((turn, object)) = set {
            game.r6700_flip_sets
                .insert(flipper, (turn, Some(ObjectId(object))));
        }
        assert_eq!(
            r6700_in_first_set(&game, flipper, ObjectId(resolving), until),
            first
        );
        // Another player's set is their own: they flipped nothing this turn.
        assert!(r6700_in_first_set(&game, PlayerId(0), ObjectId(9), false));
    }
}
