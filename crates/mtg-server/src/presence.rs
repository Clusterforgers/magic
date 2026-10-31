//! Presence: which seats have a live client, told to the table while the game runs. Never stored or hashed, so a
//! replay never sees it.

use mtg_core::ids::PlayerId;
use mtg_protocol::{ServerMessage, TableView};

use crate::actor::{GameActor, Phase};
use crate::store::EventStore;

/// The presence the table was last told, per seat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Presence {
    told: Vec<bool>,
}

impl Presence {
    /// A table of `seats` that has been told nobody is connected.
    pub(crate) fn new(seats: usize) -> Presence {
        Presence {
            told: vec![false; seats],
        }
    }

    /// Notes that the table now knows `connected` (a batch or snapshot carried it).
    pub(crate) fn told(&mut self, connected: &[PlayerId]) {
        for (index, flag) in self.told.iter_mut().enumerate() {
            *flag = connected.contains(&PlayerId::from_index(index));
        }
    }

    /// The seats whose presence differs from what the table was told, with their new flag, in seat order; afterwards
    /// the table counts as told.
    pub(crate) fn changes(&mut self, connected: &[PlayerId]) -> Vec<(PlayerId, bool)> {
        let mut out = Vec::new();
        for (index, flag) in self.told.iter_mut().enumerate() {
            let seat = PlayerId::from_index(index);
            let now = connected.contains(&seat);
            if *flag != now {
                *flag = now;
                out.push((seat, now));
            }
        }
        out
    }
}

/// Writes into `table` which seats have a live client.
pub(crate) fn stamp(table: &mut TableView, connected: &[PlayerId]) {
    for player in &mut table.players {
        player.connected = connected.contains(&player.seat);
    }
}

impl<S: EventStore> GameActor<S> {
    /// Tells the seats of a running game whose client came or went since they last heard; nothing in the lobby, whose
    /// roster carries the same news.
    ///
    /// A seat is never told of itself: while it can hear, it is connected. A client that cannot take the message is
    /// dropped by the send, and the next round tells the others of it.
    pub(crate) fn push_presence(&mut self) {
        let Phase::Running(running) = &self.phase else {
            return;
        };
        for _ in 0..=self.seats.len() {
            let connected = self.seats.connected();
            self.seats.take_gone();
            for (seat, now) in self.presence.changes(&connected) {
                let message = ServerMessage::Presence {
                    seat,
                    connected: now,
                    deadline_ms: if now { None } else { running.time_left(seat) },
                };
                for other in connected.iter().filter(|other| **other != seat) {
                    self.seats.send(*other, message.clone());
                }
            }
            if !self.seats.take_gone() {
                return;
            }
        }
    }

    /// Tells the other seats the time `seat` now has, when it is away: it was issued a decision or its clock grew.
    pub(crate) fn tell_away(&mut self, seat: PlayerId, deadline_ms: Option<u64>) {
        let connected = self.seats.connected();
        if connected.contains(&seat) {
            return;
        }
        let message = ServerMessage::Presence {
            seat,
            connected: false,
            deadline_ms,
        };
        for other in connected {
            self.seats.send(other, message.clone());
        }
    }

    /// Tells a client that just joined how long each seat that is away on a clock has; its snapshot says who is away.
    pub(crate) fn tell_joiner(&mut self, joiner: PlayerId) {
        let Phase::Running(running) = &self.phase else {
            return;
        };
        let connected = self.seats.connected();
        for index in 0..self.seats.len() {
            let seat = PlayerId::from_index(index);
            if connected.contains(&seat) {
                continue;
            }
            if let Some(deadline_ms) = running.time_left(seat) {
                let message = ServerMessage::Presence {
                    seat,
                    connected: false,
                    deadline_ms: Some(deadline_ms),
                };
                self.seats.send(joiner, message);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the table is told exactly the seats whose presence changed since it last heard, once each, and a stamp writes every seat's flag.
    use super::*;
    use rstest::rstest;

    fn seats(list: &[usize]) -> Vec<PlayerId> {
        list.iter().copied().map(PlayerId::from_index).collect()
    }

    #[rstest]
    #[case::nobody_moves(&[0, 1], &[0, 1], &[])]
    #[case::one_drops(&[0, 1, 2], &[0, 2], &[(1, false)])]
    #[case::one_returns(&[0, 2], &[0, 1, 2], &[(1, true)])]
    #[case::one_drops_another_returns(&[0, 1], &[0, 2], &[(1, false), (2, true)])]
    #[case::everybody_goes(&[0, 1, 2], &[], &[(0, false), (1, false), (2, false)])]
    fn changes_are_the_difference_from_what_was_told(
        #[case] before: &[usize],
        #[case] now: &[usize],
        #[case] expected: &[(usize, bool)],
    ) {
        let mut presence = Presence::new(3);
        presence.told(&seats(before));
        let expected: Vec<(PlayerId, bool)> = expected
            .iter()
            .map(|(seat, flag)| (PlayerId::from_index(*seat), *flag))
            .collect();
        assert_eq!(presence.changes(&seats(now)), expected);
        assert!(presence.changes(&seats(now)).is_empty(), "told only once");
    }

    #[rstest]
    #[case(&[])]
    #[case(&[1])]
    #[case(&[0, 2])]
    fn a_stamp_writes_every_seats_flag(#[case] connected: &[usize]) {
        let engine = mtg_engine::Engine::new(mtg_testkit::fuzz_setup(3, 1)).unwrap();
        let mut view = mtg_protocol::StateView::for_player(engine.state(), PlayerId(0));
        stamp(&mut view.table, &seats(connected));
        let flags: Vec<bool> = view.table.players.iter().map(|p| p.connected).collect();
        let expected: Vec<bool> = (0..3).map(|seat| connected.contains(&seat)).collect();
        assert_eq!(flags, expected);
    }
}
