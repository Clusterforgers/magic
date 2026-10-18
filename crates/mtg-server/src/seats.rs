//! Per-seat outbound queues: bounded, so a slow client can never stall the game.

use std::future::{Future, poll_fn};
use std::pin::Pin;
use std::task::Poll;

use mtg_core::ids::PlayerId;
use mtg_protocol::ServerMessage;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::{self, Receiver, Sender};

/// Smallest queue that still holds everything a join sends (welcome, snapshot, decision, game over).
pub const MIN_QUEUE: usize = 4;

/// The live connection of each seat, if it has one.
#[derive(Debug)]
pub(crate) struct Seats {
    sinks: Vec<Option<Sender<ServerMessage>>>,
    capacity: usize,
}

impl Seats {
    /// `seats` empty slots whose queues hold `capacity` messages (at least [`MIN_QUEUE`]).
    pub(crate) fn new(seats: usize, capacity: usize) -> Seats {
        Seats {
            sinks: (0..seats).map(|_| None).collect(),
            capacity: capacity.max(MIN_QUEUE),
        }
    }

    /// Number of seats at the table.
    pub(crate) fn len(&self) -> usize {
        self.sinks.len()
    }

    /// Opens a queue for `seat`, replacing (and so closing) any earlier one; `None` for a seat that does not exist.
    pub(crate) fn attach(&mut self, seat: PlayerId) -> Option<Receiver<ServerMessage>> {
        let slot = self.sinks.get_mut(seat.index())?;
        let (sink, inbox) = mpsc::channel(self.capacity);
        *slot = Some(sink);
        Some(inbox)
    }

    /// Closes the queue of `seat`; its client sees the stream end.
    pub(crate) fn detach(&mut self, seat: PlayerId) {
        if let Some(slot) = self.sinks.get_mut(seat.index()) {
            *slot = None;
        }
    }

    /// Seats with an open queue, in seat order; queues whose client went away are closed first.
    pub(crate) fn connected(&mut self) -> Vec<PlayerId> {
        for slot in &mut self.sinks {
            if slot.as_ref().is_some_and(Sender::is_closed) {
                *slot = None;
            }
        }
        self.sinks
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.is_some())
            .map(|(seat, _)| PlayerId::from_index(seat))
            .collect()
    }

    /// Resolves once some seat's client has gone away without leaving (its queue is closed), at once if one already
    /// has; never while every client listens. The closed queue stays in its slot until [`Seats::connected`] sweeps it.
    pub(crate) async fn client_gone(&self) {
        let mut closed: Vec<Pin<Box<dyn Future<Output = ()> + Send + '_>>> = self
            .sinks
            .iter()
            .flatten()
            .map(|sink| Box::pin(sink.closed()) as Pin<Box<dyn Future<Output = ()> + Send + '_>>)
            .collect();
        poll_fn(|cx| {
            if closed.iter_mut().any(|wait| wait.as_mut().poll(cx).is_ready()) {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
        .await;
    }

    /// Queues `message` for `seat` without waiting; a full or abandoned queue drops that client.
    pub(crate) fn send(&mut self, seat: PlayerId, message: ServerMessage) {
        let Some(slot) = self.sinks.get_mut(seat.index()) else {
            return;
        };
        let Some(sink) = slot else {
            return;
        };
        match sink.try_send(message) {
            Ok(()) => {}
            // The client must reconnect and resync; the game never waits for it.
            Err(TrySendError::Full(_) | TrySendError::Closed(_)) => *slot = None,
        }
    }

    /// Queues a copy of `message` for every connected seat.
    pub(crate) fn broadcast(&mut self, message: &ServerMessage) {
        for seat in 0..self.sinks.len() {
            self.send(PlayerId::from_index(seat), message.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a seat's queue is bounded and never blocks the sender; overflow or abandonment drops only that client, and a new join replaces the old queue.
    use super::*;
    use mtg_protocol::ServerMessage::Chat;

    fn line(n: u64) -> ServerMessage {
        Chat {
            from: PlayerId(0),
            text: n.to_string(),
        }
    }

    #[test]
    fn messages_arrive_in_order() {
        let mut seats = Seats::new(2, 8);
        let mut inbox = seats.attach(PlayerId(1)).unwrap();
        for n in 0..5 {
            seats.send(PlayerId(1), line(n));
        }
        for n in 0..5 {
            assert_eq!(inbox.try_recv().unwrap(), line(n));
        }
    }

    #[test]
    fn a_full_queue_drops_that_client_only() {
        let mut seats = Seats::new(2, MIN_QUEUE);
        let mut slow = seats.attach(PlayerId(0)).unwrap();
        let mut fast = seats.attach(PlayerId(1)).unwrap();
        for n in 0..=(MIN_QUEUE as u64) {
            seats.broadcast(&line(n));
            assert!(fast.try_recv().is_ok());
        }
        assert_eq!(seats.connected(), [PlayerId(1)]);
        // The dropped client still drains what was queued, then sees the end of the stream.
        for n in 0..MIN_QUEUE as u64 {
            assert_eq!(slow.try_recv().unwrap(), line(n));
        }
        assert_eq!(
            slow.try_recv(),
            Err(mpsc::error::TryRecvError::Disconnected)
        );
    }

    #[test]
    fn an_abandoned_queue_is_forgotten() {
        let mut seats = Seats::new(2, 8);
        drop(seats.attach(PlayerId(0)).unwrap());
        assert_eq!(seats.connected(), Vec::<PlayerId>::new());
        seats.send(PlayerId(0), line(1));
        assert_eq!(seats.connected(), Vec::<PlayerId>::new());
    }

    #[test]
    fn attaching_again_closes_the_old_queue() {
        let mut seats = Seats::new(2, 8);
        let mut old = seats.attach(PlayerId(0)).unwrap();
        let mut new = seats.attach(PlayerId(0)).unwrap();
        seats.send(PlayerId(0), line(1));
        assert_eq!(new.try_recv().unwrap(), line(1));
        assert_eq!(old.try_recv(), Err(mpsc::error::TryRecvError::Disconnected));
    }

    #[test]
    fn unknown_seats_are_ignored() {
        let mut seats = Seats::new(2, 8);
        assert!(seats.attach(PlayerId(2)).is_none());
        seats.send(PlayerId(2), line(1));
        seats.detach(PlayerId(2));
        assert_eq!(seats.len(), 2);
    }

    #[test]
    fn the_queue_is_never_smaller_than_a_join() {
        let mut seats = Seats::new(1, 0);
        let mut inbox = seats.attach(PlayerId(0)).unwrap();
        for n in 0..MIN_QUEUE as u64 {
            seats.send(PlayerId(0), line(n));
        }
        assert_eq!(inbox.try_recv().unwrap(), line(0));
        assert_eq!(seats.connected(), [PlayerId(0)]);
    }
}
