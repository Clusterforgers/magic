//! Facts of the game for pack r64-01, folded from events: the timestamp clock as each number was last rolled on a die,
//! which "until any player rolls a 1" reads (Drop Tower), and the lands each player tapped for mana this turn
//! (Desolation).

use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::state::{GameState, Tally};

/// Notes a die roll of `result` at the clock of the next timestamp, so every effect created before it sees it.
pub(crate) fn r6401_note_roll(state: &mut GameState, result: u32) {
    state.r6401_rolled_at.insert(result, state.next_timestamp);
}

/// Counts a land tapped for mana by `player` (`Tally::R6401LandTappedForMana`); another permanent counts for nothing.
pub(crate) fn r6401_note_tapped_for_mana(
    state: &mut GameState,
    player: PlayerId,
    source: ObjectId,
) {
    let land = crate::characteristics::characteristics(state, source)
        .is_some_and(|chars| chars.types.contains(CardType::Land));
    if land {
        state.tally(player, Tally::R6401LandTappedForMana);
    }
}

/// Whether `number` was rolled on a die after the effect with this timestamp began.
pub(crate) fn r6401_rolled_since(state: &GameState, number: u32, timestamp: u64) -> bool {
    state
        .r6401_rolled_at
        .get(&number)
        .is_some_and(|at| *at > timestamp)
}

#[cfg(test)]
mod tests {
    //! Goal: an effect "until end of turn, or until any player rolls a N" ends once any player rolls exactly that number
    //! after it began, whoever rolls and on any die, and never for another number or a roll made before it began.
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use mtg_cards::modify::Duration;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use crate::effects::{ContinuousEffect, Modification, Selector};
    use crate::events::Event;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::object::GameObject;
    use crate::state::GameState;
    use mtg_core::ids::EffectId;

    fn r6401_game(seats: usize) -> GameState {
        let mut battlefields = vec![vec![creature("Wolf", 2, 2)]];
        battlefields.extend((1..seats).map(|_| vec![]));
        state((0..seats).map(|_| vec![]).collect(), battlefields)
    }

    fn r6401_roll(game: &mut GameState, seat: u64, sides: u32, result: u32) {
        let event = Event::DieRolled {
            player: PlayerId(seat),
            sides,
            result,
            natural: None,
        };
        apply_event(game, &event).unwrap();
    }

    #[rstest]
    #[case::a_one_by_anyone_ends_it(1, 1, 6, true)]
    #[case::a_one_on_a_d20_ends_it(1, 1, 20, true)]
    #[case::another_number_does_not(1, 2, 6, false)]
    #[case::a_six_ends_a_six(6, 6, 6, true)]
    fn r6401_a_roll_of_the_number_ends_the_effect(
        #[values(2, 3, 4)] seats: usize,
        #[case] number: u32,
        #[case] rolled: u32,
        #[case] sides: u32,
        #[case] ends: bool,
    ) {
        let mut game = r6401_game(seats);
        // A roll before the effect began does not end it.
        r6401_roll(&mut game, 0, sides, rolled);
        let wolf = game
            .objects()
            .find(|o| o.card().name == "Wolf")
            .map(GameObject::reference)
            .unwrap();
        let effect = ContinuousEffect::new(
            EffectId(0),
            Selector::Objects(vec![wolf]),
            Modification::ModifyPowerToughness {
                power: 1,
                toughness: 1,
            },
            Duration::UntilEndOfTurnOrRolled(number),
            game.next_timestamp,
        )
        .begun(PlayerId(0), 1);
        apply_event(
            &mut game,
            &Event::EffectCreated {
                effect: effect.clone().into(),
            },
        )
        .unwrap();
        assert!(effect.is_active(&game));
        let roller = u64::try_from(seats - 1).unwrap();
        r6401_roll(&mut game, roller, sides, rolled);
        assert_eq!(effect.is_active(&game), !ends);
    }
}
