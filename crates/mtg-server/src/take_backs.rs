//! Take-backs ([server.md](../../../docs/server.md), Take-backs): a seat asks to undo its last cast, activation or land
//! play, every other seat in the game agrees or declines within [`TAKE_BACK_WAIT`], and the game is rebuilt without it.

use std::time::Duration;

use mtg_core::ids::PlayerId;
use mtg_engine::take_back::TakeBackOffer;
use mtg_protocol::{ErrorCode, ServerMessage, TakeBackOutcome};
use tokio::time::Instant;

use crate::actor::{Fatal, GameActor, Phase, not_started};
use crate::running::Rejection;
use crate::store::EventStore;

/// How long the other seats have to answer a take-back request; silence declines.
pub(crate) const TAKE_BACK_WAIT: Duration = Duration::from_secs(15);

/// The open request and what each seat was last told it may take back. Never stored.
#[derive(Debug, Default)]
pub(crate) struct TakeBacks {
    request: Option<Request>,
    told: Vec<Option<TakeBackOffer>>,
}

/// A take-back request waiting for the other seats.
#[derive(Debug, Clone)]
struct Request {
    offer: TakeBackOffer,
    waiting: Vec<PlayerId>,
    due: Instant,
}

impl Request {
    fn asked(&self) -> ServerMessage {
        let left = self.due.saturating_duration_since(Instant::now());
        ServerMessage::TakeBackAsked {
            offer: self.offer.clone(),
            waiting: self.waiting.clone(),
            deadline_ms: u64::try_from(left.as_millis()).unwrap_or(u64::MAX),
        }
    }
}

impl TakeBacks {
    /// When the open request counts as declined.
    pub(crate) fn due(&self) -> Option<Instant> {
        self.request.as_ref().map(|request| request.due)
    }

    /// Notes that `seat` was told `offer`; whether that changed what it knew.
    fn tell(&mut self, seat: PlayerId, offer: Option<TakeBackOffer>) -> bool {
        if self.told.len() <= seat.index() {
            self.told.resize(seat.index() + 1, None);
        }
        let changed = self.told[seat.index()] != offer;
        self.told[seat.index()] = offer;
        changed
    }
}

fn busy() -> Rejection {
    Rejection::new(ErrorCode::BadMessage, "a take-back is already being asked")
}

impl<S: EventStore> GameActor<S> {
    /// `seat` asks to take back its last action: refused at once if the engine cannot undo it, else every other seat in
    /// the game is asked.
    pub(crate) async fn ask_take_back(&mut self, seat: PlayerId) -> Result<(), Fatal> {
        let Phase::Running(running) = &self.phase else {
            self.refuse(seat, not_started());
            return Ok(());
        };
        if self.take_backs.request.is_some() {
            self.refuse(seat, busy());
            return Ok(());
        }
        let offer = match running.engine.take_back_offer(seat) {
            Ok(offer) => offer,
            Err(reason) => {
                self.seats
                    .send(seat, ServerMessage::TakeBackRefused { reason });
                return Ok(());
            }
        };
        let state = running.engine.state();
        let waiting = state
            .players()
            .iter()
            .map(|player| player.id())
            .filter(|player| *player != seat && state.is_alive(*player))
            .collect::<Vec<_>>();
        let request = Request {
            offer,
            waiting,
            due: Instant::now() + TAKE_BACK_WAIT,
        };
        tracing::info!(game = %self.id, seat = seat.index(), "take-back asked");
        if request.waiting.is_empty() {
            self.take_backs.request = Some(request);
            return self.carry_out().await;
        }
        self.seats.broadcast(&request.asked());
        self.take_backs.request = Some(request);
        Ok(())
    }

    /// `seat`'s answer to the open request: a decline ends it, the last agreement carries it out.
    pub(crate) async fn vote_take_back(
        &mut self,
        seat: PlayerId,
        approve: bool,
    ) -> Result<(), Fatal> {
        let Some(request) = self.take_backs.request.as_mut() else {
            let reason = "no take-back is being asked";
            self.refuse(seat, Rejection::new(ErrorCode::BadMessage, reason));
            return Ok(());
        };
        if !request.waiting.contains(&seat) {
            let reason = "no answer of yours is awaited";
            self.refuse(seat, Rejection::new(ErrorCode::BadMessage, reason));
            return Ok(());
        }
        if !approve {
            self.end_take_back(TakeBackOutcome::Declined { by: seat });
            return Ok(());
        }
        request.waiting.retain(|waiting| *waiting != seat);
        if request.waiting.is_empty() {
            return self.carry_out().await;
        }
        let asked = request.asked();
        self.seats.broadcast(&asked);
        Ok(())
    }

    /// Ends the open request as declined if its time is up; whether it was.
    pub(crate) fn take_back_expired(&mut self) -> bool {
        let Some(request) = &self.take_backs.request else {
            return false;
        };
        if request.due > Instant::now() {
            return false;
        }
        let by = request
            .waiting
            .first()
            .copied()
            .unwrap_or(request.offer.player);
        self.end_take_back(TakeBackOutcome::TimedOut { by });
        true
    }

    /// When the open request counts as declined.
    pub(crate) fn take_back_due(&self) -> Option<Instant> {
        self.take_backs.due()
    }

    /// Closes the open request and tells every seat how it ended.
    fn end_take_back(&mut self, outcome: TakeBackOutcome) {
        if let Some(request) = self.take_backs.request.take() {
            tracing::info!(game = %self.id, seat = request.offer.player.index(), ?outcome, "take-back ended");
            self.seats.broadcast(&ServerMessage::TakeBackEnded {
                offer: request.offer,
                outcome,
            });
        }
    }

    /// Takes the action back once every seat agreed, if it is still the one asked about.
    async fn carry_out(&mut self) -> Result<(), Fatal> {
        let Some(request) = self.take_backs.request.clone() else {
            return Ok(());
        };
        let Phase::Running(running) = &mut self.phase else {
            return Ok(());
        };
        let seat = request.offer.player;
        let still = running.engine.take_back_offer(seat).as_ref() == Ok(&request.offer);
        let step = if still {
            running.take_back(seat).ok()
        } else {
            None
        };
        let Some(step) = step else {
            self.end_take_back(TakeBackOutcome::Withdrawn);
            return Ok(());
        };
        self.take_backs.request = None;
        self.publish(step).await?;
        let Phase::Running(running) = &self.phase else {
            return Ok(());
        };
        // A step that could not be saved is undone, and the action is on offer again.
        let saved = running.engine.take_back_offer(seat).as_ref() != Ok(&request.offer);
        let outcome = if saved {
            TakeBackOutcome::Approved
        } else {
            TakeBackOutcome::Withdrawn
        };
        self.seats.broadcast(&ServerMessage::TakeBackEnded {
            offer: request.offer,
            outcome,
        });
        Ok(())
    }

    /// After a step: withdraws a request the step made moot and tells each connected seat what it may take back now,
    /// when that changed.
    pub(crate) fn take_back_news(&mut self) {
        let Phase::Running(running) = &self.phase else {
            return;
        };
        let moot = self.take_backs.request.as_ref().is_some_and(|request| {
            running
                .engine
                .take_back_offer(request.offer.player)
                .as_ref()
                != Ok(&request.offer)
        });
        let offers = self
            .seats
            .connected()
            .into_iter()
            .map(|seat| (seat, running.engine.take_back_offer(seat).ok()))
            .collect::<Vec<_>>();
        if moot {
            self.end_take_back(TakeBackOutcome::Withdrawn);
        }
        for (seat, offer) in offers {
            if self.take_backs.tell(seat, offer.clone()) {
                self.seats
                    .send(seat, ServerMessage::TakeBackOffer { offer });
            }
        }
    }

    /// What a seat that (re)joins is told: what it may take back and the open request.
    pub(crate) fn take_back_sync(&mut self, seat: PlayerId) {
        let Phase::Running(running) = &self.phase else {
            return;
        };
        let offer = running.engine.take_back_offer(seat).ok();
        self.take_backs.tell(seat, offer.clone());
        self.seats
            .send(seat, ServerMessage::TakeBackOffer { offer });
        if let Some(request) = &self.take_backs.request {
            let asked = request.asked();
            self.seats.send(seat, asked);
        }
    }
}
