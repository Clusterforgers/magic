//! What each seat's latest turn held, kept when the turn ends: the spells every player cast during it and each count of
//! every player. The turn's own counts are cleared as the next turn begins; "last turn" reads this summary of the turn
//! that just ended.

use std::collections::BTreeMap;

use mtg_cards::value::TallyEvent;
use mtg_core::ids::PlayerId;

use crate::object::ObjectRef;

use crate::state::{GameState, Tally};

/// One finished turn: how many spells each player cast during it, by seat, and what each player counted in it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TurnSummary {
    /// Spells cast, indexed by seat.
    pub(crate) cast: Vec<u32>,
    /// The counts of each player during the turn ("if you lost life last turn").
    pub(crate) tallies: BTreeMap<(PlayerId, Tally), u32>,
    /// The creatures declared as attackers during the turn, as the objects they were.
    pub(crate) attackers: Vec<ObjectRef>,
    /// The turn's log of objects (what entered, was cast, died and the like), as they were then.
    pub(crate) logged: Vec<(TallyEvent, PlayerId, crate::snapshot::Snapshot)>,
    /// The players attacked during the turn (not their planeswalkers), each once.
    pub(crate) attacked: Vec<PlayerId>,
}

impl GameState {
    /// Keeps the turn of `active` that is ending as that seat's latest turn, before its counts are cleared.
    pub(crate) fn summarize_turn(&mut self, active: PlayerId) {
        let cast = (0..self.players.len())
            .map(|seat| self.tallied(PlayerId::from_index(seat), Tally::Cast))
            .collect();
        let tallies = self.tallies.players.clone();
        let attackers = self
            .tallies
            .records
            .iter()
            .filter(|record| record.event == TallyEvent::Attacked)
            .filter_map(|record| record.subject)
            .collect();
        let mut attacked: Vec<PlayerId> = Vec::new();
        for record in &self.tallies.records {
            if let (TallyEvent::Attacked, Some(crate::choose::Target::Player(player))) =
                (record.event, record.other)
                && !attacked.contains(&player)
            {
                attacked.push(player);
            }
        }
        self.seat_turns.insert(
            active,
            TurnSummary {
                cast,
                tallies,
                attackers,
                logged: self.tallies.logged.clone(),
                attacked,
            },
        );
        self.last_turn_of = Some(active);
    }

    /// The log of objects of the turn that ended last; empty before any turn has ended.
    pub(crate) fn logged_last_turn(&self) -> &[(TallyEvent, PlayerId, crate::snapshot::Snapshot)] {
        self.last_turn_of
            .and_then(|seat| self.seat_turns.get(&seat))
            .map_or(&[], |summary| summary.logged.as_slice())
    }

    /// Whether the object attacked during its controller's latest finished turn (`Pred::AttackedLastTurn`).
    pub(crate) fn attacked_last_turn(&self, object: ObjectRef) -> bool {
        self.object(object.id)
            .map(crate::object::GameObject::controller)
            .and_then(|controller| self.seat_turns.get(&controller))
            .is_some_and(|summary| summary.attackers.contains(&object))
    }

    /// Whether `attacker` attacked `defender` itself during `attacker`'s latest finished turn
    /// (`Condition::AttackedYouLastTurn`).
    pub(crate) fn r305b_attacked_last_turn(&self, attacker: PlayerId, defender: PlayerId) -> bool {
        self.seat_turns
            .get(&attacker)
            .is_some_and(|summary| summary.attacked.contains(&defender))
    }

    /// How many events of the kind the player had during the turn that ended last; 0 before any turn has ended.
    pub(crate) fn tallied_last_turn(&self, player: PlayerId, kind: Tally) -> u32 {
        self.last_turn_of
            .and_then(|seat| self.seat_turns.get(&seat))
            .and_then(|summary| summary.tallies.get(&(player, kind)).copied())
            .unwrap_or(0)
    }

    /// Adds the summaries to the hash; a game no turn has ended in hashes as it did before they existed.
    pub(crate) fn hash_turn_summaries<H: std::hash::Hasher>(&self, state: &mut H) {
        use std::hash::Hash;
        if let Some(last) = self.last_turn_of {
            last.hash(state);
            self.seat_turns.hash(state);
        }
    }

    /// The most spells one of `players` cast during the turn that ended last; 0 before any turn has ended.
    pub(crate) fn cast_last_turn(&self, players: &[PlayerId]) -> u32 {
        let Some(summary) = self
            .last_turn_of
            .and_then(|seat| self.seat_turns.get(&seat))
        else {
            return 0;
        };
        players
            .iter()
            .filter_map(|player| summary.cast.get(player.index()).copied())
            .max()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the summary of the turn that ended holds what each seat cast, "last turn" reads the most one chosen player cast in it, a seat's older turn is replaced by its newer one, and nothing is read before a turn has ended.
    use rstest::rstest;

    use super::*;
    use crate::fixtures::setup;

    fn game(seats: usize) -> GameState {
        GameState::from_setup(&setup(vec![vec![]; seats], vec![vec![]; seats])).unwrap()
    }

    fn all(game: &GameState) -> Vec<PlayerId> {
        (0..game.players.len()).map(PlayerId::from_index).collect()
    }

    #[rstest]
    #[case::no_spells_at_all(&[], 0)]
    #[case::one_spell(&[(0, 1)], 1)]
    #[case::two_by_one_player(&[(0, 2)], 2)]
    #[case::one_each_is_not_two_by_one(&[(0, 1), (1, 1)], 1)]
    #[case::the_other_players_count_too(&[(1, 2), (0, 1)], 2)]
    fn the_largest_count_of_one_player_is_read_after_the_turn_ends(
        #[case] casts: &[(usize, u32)],
        #[case] largest: u32,
    ) {
        let mut game = game(2);
        for (seat, times) in casts {
            for _ in 0..*times {
                game.tally(PlayerId::from_index(*seat), Tally::Cast);
            }
        }
        assert_eq!(game.cast_last_turn(&all(&game)), 0, "no turn has ended yet");
        game.summarize_turn(PlayerId(0));
        assert_eq!(game.cast_last_turn(&all(&game)), largest);
    }

    #[test]
    fn the_next_turn_beginning_keeps_the_ending_turn_before_it_clears_the_counts() {
        let mut game = game(2);
        game.tally(PlayerId(0), Tally::Cast);
        game.tally(PlayerId(0), Tally::Cast);
        crate::fold::apply_event(
            &mut game,
            &crate::events::Event::TurnBegan {
                number: 2,
                active: PlayerId(1),
                extra: false,
            },
        )
        .unwrap();
        assert_eq!(game.tallied(PlayerId(0), Tally::Cast), 0);
        assert_eq!(game.cast_last_turn(&[PlayerId(0)]), 2);
        assert_eq!(game.cast_last_turn(&[PlayerId(1)]), 0);
    }

    #[test]
    fn four_seats_keep_their_own_latest_turn_and_the_newest_is_read() {
        let mut game = game(4);
        for seat in 0..4 {
            for _ in 0..=seat {
                game.tally(PlayerId::from_index(seat), Tally::Cast);
            }
            game.summarize_turn(PlayerId::from_index(seat));
            game.clear_turn_uses();
        }
        // The fourth seat's turn is the last one and held its four spells; each turn started from cleared counts.
        assert_eq!(game.seat_turns.len(), 4);
        assert_eq!(game.cast_last_turn(&all(&game)), 4);
        assert_eq!(game.seat_turns[&PlayerId(1)].cast, [0, 2, 0, 0]);
        // The first seat's turn came again and held two spells by the third seat.
        game.tally(PlayerId(2), Tally::Cast);
        game.tally(PlayerId(2), Tally::Cast);
        game.summarize_turn(PlayerId(0));
        assert_eq!(game.cast_last_turn(&all(&game)), 2);
        assert_eq!(game.cast_last_turn(&[PlayerId(1)]), 0);
        assert_eq!(game.seat_turns.len(), 4, "a seat has one latest turn");
    }
}
