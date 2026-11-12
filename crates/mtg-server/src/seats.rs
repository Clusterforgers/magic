//! Per-seat outbound queues, and one per spectator: bounded, so a slow client can never stall the game.

use std::future::{Future, poll_fn};
use std::pin::Pin;
use std::task::Poll;

use mtg_core::ids::PlayerId;
use mtg_protocol::ServerMessage;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::{self, Receiver, Sender};

/// Smallest queue that still holds everything a join sends (welcome, snapshot, decision, game over).
pub const MIN_QUEUE: usize = 4;

/// The live connection of each seat, if it has one, and of each spectator.
#[derive(Debug)]
pub(crate) struct Seats {
    sinks: Vec<Option<Sender<ServerMessage>>>,
    /// The spectators' queues; a spectator that hangs up or falls behind is dropped without a word to the table.
    watchers: Vec<Sender<ServerMessage>>,
    /// How many spectators the seats were last told of.
    told_watchers: usize,
    capacity: usize,
    /// Set when a queue was dropped because its client hung up or fell behind; see [`Seats::take_gone`].
    gone: bool,
}

impl Seats {
    /// `seats` empty slots whose queues hold `capacity` messages (at least [`MIN_QUEUE`]).
    pub(crate) fn new(seats: usize, capacity: usize) -> Seats {
        Seats {
            sinks: (0..seats).map(|_| None).collect(),
            watchers: Vec::new(),
            told_watchers: 0,
            capacity: capacity.max(MIN_QUEUE),
            gone: false,
        }
    }

    /// Number of seats at the table.
    pub(crate) fn len(&self) -> usize {
        self.sinks.len()
    }

    /// The viewer every spectator is: the number of seats, which names no seat.
    pub(crate) fn spectator(&self) -> PlayerId {
        PlayerId::from_index(self.sinks.len())
    }

    /// Opens a queue for one more spectator holding `first` (its welcome and snapshot); a queue too small for them
    /// is closed at once, so the client reconnects.
    pub(crate) fn watch(&mut self, first: Vec<ServerMessage>) -> Receiver<ServerMessage> {
        let (sink, inbox) = mpsc::channel(self.capacity);
        if first
            .into_iter()
            .all(|message| sink.try_send(message).is_ok())
        {
            self.watchers.push(sink);
        }
        inbox
    }

    /// Whether some spectator listens; queues whose client went away are closed first.
    pub(crate) fn watched(&mut self) -> bool {
        self.watcher_count() > 0
    }

    /// How many spectators listen; queues whose client went away are closed first.
    pub(crate) fn watcher_count(&mut self) -> usize {
        self.watchers.retain(|sink| !sink.is_closed());
        self.watchers.len()
    }

    /// The number of spectators when it changed since the seats were last told, and notes it as told.
    pub(crate) fn spectator_news(&mut self) -> Option<usize> {
        let now = self.watcher_count();
        (now != self.told_watchers).then(|| {
            self.told_watchers = now;
            now
        })
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
                self.gone = true;
            }
        }
        self.sinks
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.is_some())
            .map(|(seat, _)| PlayerId::from_index(seat))
            .collect()
    }

    /// Whether a client has been dropped since the last call (it hung up or could not keep up; not a `detach`), and
    /// forgets it. The table that was told of the seats at that moment is told again.
    pub(crate) fn take_gone(&mut self) -> bool {
        std::mem::take(&mut self.gone)
    }

    /// Resolves once some seat's or spectator's client has gone away without leaving (its queue is closed), at once
    /// if one already has; never while every client listens. The closed queue stays until [`Seats::connected`] (a
    /// seat's) or [`Seats::spectator_news`] (a spectator's) sweeps it.
    pub(crate) async fn client_gone(&self) {
        let mut closed: Vec<Pin<Box<dyn Future<Output = ()> + Send + '_>>> = self
            .sinks
            .iter()
            .flatten()
            .chain(&self.watchers)
            .map(|sink| Box::pin(sink.closed()) as Pin<Box<dyn Future<Output = ()> + Send + '_>>)
            .collect();
        poll_fn(|cx| {
            if closed
                .iter_mut()
                .any(|wait| wait.as_mut().poll(cx).is_ready())
            {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
        .await;
    }

    /// Queues `message` for `seat`, or for every spectator when `seat` is [`Seats::spectator`], without waiting; a
    /// full or abandoned queue drops that client.
    pub(crate) fn send(&mut self, seat: PlayerId, message: ServerMessage) {
        if seat == self.spectator() {
            self.watchers
                .retain(|sink| sink.try_send(message.clone()).is_ok());
            return;
        }
        let Some(slot) = self.sinks.get_mut(seat.index()) else {
            return;
        };
        let Some(sink) = slot else {
            return;
        };
        match sink.try_send(message) {
            Ok(()) => {}
            // The client must reconnect and resync; the game never waits for it.
            Err(TrySendError::Full(_) | TrySendError::Closed(_)) => {
                *slot = None;
                self.gone = true;
            }
        }
    }

    /// Queues a copy of `message` for every connected seat and every spectator.
    pub(crate) fn broadcast(&mut self, message: &ServerMessage) {
        for seat in 0..=self.sinks.len() {
            self.send(PlayerId::from_index(seat), message.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a seat's queue is bounded and never blocks the sender; overflow or abandonment drops only that client, and a new join replaces the old queue; spectators share every message sent to the spectator viewer, each starts with its own first messages, and none counts as a seat or a client gone.
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

    #[test]
    fn a_dropped_client_is_remembered_once_however_it_was_found() {
        // Found by a sweep, by a send to a closed queue, and by a send to a full one.
        let mut seats = Seats::new(3, MIN_QUEUE);
        let (hung_up, too_slow, listening) = (
            seats.attach(PlayerId(0)).unwrap(),
            seats.attach(PlayerId(1)).unwrap(),
            seats.attach(PlayerId(2)).unwrap(),
        );
        assert!(!seats.take_gone(), "nobody has gone yet");
        drop(hung_up);
        assert_eq!(seats.connected(), [PlayerId(1), PlayerId(2)]);
        assert!(seats.take_gone());
        assert!(!seats.take_gone(), "and it is told once");

        let mut listening = listening;
        for n in 0..=MIN_QUEUE as u64 {
            seats.broadcast(&line(n));
            while listening.try_recv().is_ok() {}
        }
        assert!(
            seats.take_gone(),
            "a queue that overflowed is a client gone"
        );
        assert_eq!(seats.connected(), [PlayerId(2)]);
        drop(too_slow);
    }

    #[test]
    fn leaving_and_taking_a_seat_over_are_not_a_client_gone() {
        let mut seats = Seats::new(2, 8);
        let _kept = seats.attach(PlayerId(0)).unwrap();
        let _old = seats.attach(PlayerId(1)).unwrap();
        let _new = seats.attach(PlayerId(1)).unwrap();
        seats.detach(PlayerId(0));
        assert_eq!(seats.connected(), [PlayerId(1)]);
        assert!(!seats.take_gone());
    }

    #[tokio::test]
    async fn the_wait_for_a_client_gone_ends_when_a_queue_closes_and_not_before() {
        use std::time::Duration;
        use tokio::time::timeout;

        let mut seats = Seats::new(3, 8);
        // No seat connected, and seats whose clients listen: nothing to wait for.
        assert!(
            timeout(Duration::from_millis(30), seats.client_gone())
                .await
                .is_err()
        );
        let first = seats.attach(PlayerId(0)).unwrap();
        let second = seats.attach(PlayerId(2)).unwrap();
        assert!(
            timeout(Duration::from_millis(30), seats.client_gone())
                .await
                .is_err()
        );

        drop(second);
        timeout(Duration::from_secs(5), seats.client_gone())
            .await
            .expect("a closed queue ends the wait");
        // It stays ended until the closed queue is swept, so the caller must sweep.
        timeout(Duration::from_secs(5), seats.client_gone())
            .await
            .expect("still closed");
        assert_eq!(seats.connected(), [PlayerId(0)]);
        assert!(
            timeout(Duration::from_millis(30), seats.client_gone())
                .await
                .is_err()
        );
        drop(first);
    }

    #[test]
    fn spectators_share_what_the_spectator_viewer_is_sent_and_start_with_their_own() {
        let mut seats = Seats::new(2, 8);
        let mut seat = seats.attach(PlayerId(0)).unwrap();
        let mut first = seats.watch(vec![line(10)]);
        let mut second = seats.watch(vec![line(20)]);
        assert_eq!(seats.spectator(), PlayerId(2));
        assert!(seats.watched());
        seats.send(PlayerId(2), line(1));
        seats.broadcast(&line(2));
        for (inbox, own) in [(&mut first, 10), (&mut second, 20)] {
            assert_eq!(inbox.try_recv().unwrap(), line(own));
            assert_eq!(inbox.try_recv().unwrap(), line(1));
            assert_eq!(inbox.try_recv().unwrap(), line(2));
        }
        assert_eq!(
            seat.try_recv().unwrap(),
            line(2),
            "a seat gets the broadcast only"
        );
        assert!(seat.try_recv().is_err());
        assert_eq!(seats.connected(), [PlayerId(0)], "a spectator is no seat");
    }

    #[test]
    fn a_spectator_that_goes_or_falls_behind_is_dropped_without_a_client_gone() {
        let mut seats = Seats::new(1, MIN_QUEUE);
        let gone = seats.watch(Vec::new());
        let mut slow = seats.watch(Vec::new());
        drop(gone);
        for n in 0..=(MIN_QUEUE as u64) {
            seats.send(PlayerId(1), line(n));
        }
        assert!(!seats.watched());
        assert!(!seats.take_gone());
        for n in 0..MIN_QUEUE as u64 {
            assert_eq!(slow.try_recv().unwrap(), line(n));
        }
        assert_eq!(
            slow.try_recv(),
            Err(mpsc::error::TryRecvError::Disconnected)
        );
    }

    #[test]
    fn a_spectator_whose_first_messages_overflow_its_queue_is_closed() {
        let mut seats = Seats::new(1, MIN_QUEUE);
        let too_many = (0..=MIN_QUEUE as u64).map(line).collect();
        let mut refused = seats.watch(too_many);
        assert!(!seats.watched());
        while refused.try_recv().is_ok() {}
        assert_eq!(
            refused.try_recv(),
            Err(mpsc::error::TryRecvError::Disconnected)
        );
    }
}
