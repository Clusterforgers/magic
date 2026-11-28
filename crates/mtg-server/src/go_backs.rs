//! Going back a step ([server.md](../../../docs/server.md), Going back): a seat takes back its passes to an earlier
//! step while the engine finds nothing irreversible since; every other person who passed meanwhile agrees first,
//! within [`GO_BACK_WAIT`].

use std::time::Duration;

use mtg_core::ids::PlayerId;
use mtg_engine::Engine;
use mtg_engine::go_back::{GoBackOffer, GoBackRefusal};
use mtg_protocol::{ErrorCode, FullControlMode, ServerMessage, TakeBackOutcome};
use tokio::time::Instant;

use crate::actor::{Fatal, GameActor, Phase, not_started};
use crate::running::Rejection;
use crate::store::EventStore;
use crate::take_backs::TAKE_BACK_WAIT;

/// How many passes sent by the seats are kept.
const KEPT_SENT: usize = 128;

/// How long the people asked have to answer a go-back request; silence declines.
pub(crate) const GO_BACK_WAIT: Duration = TAKE_BACK_WAIT;

/// The passes the seats sent themselves, the seats bots play, the open request and what each seat was last told.
/// Never stored.
#[derive(Debug, Default)]
pub(crate) struct GoBacks {
    /// Each pass a seat sent (not one the server gave for it): the seat and where its events begin in the log.
    sent: Vec<(PlayerId, u64)>,
    bots: Vec<PlayerId>,
    told: Vec<Option<GoBackOffer>>,
    request: Option<Request>,
}

/// A go-back request waiting for the people whose passes it undoes.
#[derive(Debug, Clone)]
struct Request {
    offer: GoBackOffer,
    /// Every seat asked so far, agreed or not.
    asked: Vec<PlayerId>,
    waiting: Vec<PlayerId>,
    due: Instant,
}

impl Request {
    fn asked(&self) -> ServerMessage {
        let left = self.due.saturating_duration_since(Instant::now());
        ServerMessage::GoBackAsked {
            offer: self.offer.clone(),
            waiting: self.waiting.clone(),
            deadline_ms: u64::try_from(left.as_millis()).unwrap_or(u64::MAX),
        }
    }
}

impl GoBacks {
    /// Notes that a server bot plays `seat`.
    pub(crate) fn mark_bot(&mut self, seat: PlayerId) {
        if !self.bots.contains(&seat) {
            self.bots.push(seat);
        }
    }

    /// Notes that `seat` sent a pass whose events begin at `at`.
    pub(crate) fn sent(&mut self, seat: PlayerId, at: u64) {
        self.sent.push((seat, at));
        if self.sent.len() > KEPT_SENT {
            self.sent.remove(0);
        }
    }

    /// Forgets the passes a cut of the log at `from` undid.
    pub(crate) fn cut(&mut self, from: u64) {
        self.sent.retain(|(_, at)| *at < from);
    }

    /// The step `seat` may go back to: its latest own pass in an earlier step the engine allows. Other people's passes
    /// since never refuse it (they are asked instead), so the offer shows nothing about where a seat was asked.
    pub(crate) fn offer(
        &self,
        engine: &Engine,
        seat: PlayerId,
    ) -> Result<GoBackOffer, GoBackRefusal> {
        if self.bots.contains(&seat) {
            return Err(GoBackRefusal::Nothing);
        }
        let len = u64::try_from(engine.events().len()).unwrap_or(u64::MAX);
        let own = |at: u64| at < len && self.sent.contains(&(seat, at));
        engine.go_back_offer(seat, own)
    }

    /// The people still in the game who passed since `offer`'s pass, whether they sent the pass or the table gave it
    /// for them: going back undoes those passes, so each is asked. A bot's pass needs no one's leave.
    fn consenting(&self, engine: &Engine, offer: &GoBackOffer) -> Vec<PlayerId> {
        let state = engine.state();
        engine
            .passed_since(offer.player, offer.at)
            .into_iter()
            .filter(|by| !self.bots.contains(by) && state.is_alive(*by))
            .collect()
    }

    /// When the open request counts as declined.
    pub(crate) fn due(&self) -> Option<Instant> {
        self.request.as_ref().map(|request| request.due)
    }

    /// Notes that `seat` was told `offer`; whether that changed what it knew.
    fn tell(&mut self, seat: PlayerId, offer: Option<GoBackOffer>) -> bool {
        if self.told.len() <= seat.index() {
            self.told.resize(seat.index() + 1, None);
        }
        let changed = self.told[seat.index()] != offer;
        self.told[seat.index()] = offer;
        changed
    }
}

fn bad(reason: &str) -> Rejection {
    Rejection::new(ErrorCode::BadMessage, reason)
}

impl<S: EventStore> GameActor<S> {
    /// `seat` goes back to the step its offer names: refused if it has none now; otherwise each person who passed since
    /// is asked, and with no one to ask the log is cut back to its pass at once.
    pub(crate) async fn go_back(&mut self, seat: PlayerId) -> Result<(), Fatal> {
        let Phase::Running(running) = &self.phase else {
            self.refuse(seat, not_started());
            return Ok(());
        };
        if self.go_backs.request.is_some() {
            self.refuse(seat, bad("a go-back is already being asked"));
            return Ok(());
        }
        let offer = match self.go_backs.offer(&running.engine, seat) {
            Ok(offer) => offer,
            Err(reason) => {
                self.seats
                    .send(seat, ServerMessage::GoBackRefused { reason });
                return Ok(());
            }
        };
        let waiting = self.go_backs.consenting(&running.engine, &offer);
        if waiting.is_empty() {
            return self.went_back(&offer).await.map(|_| ());
        }
        let request = Request {
            offer,
            asked: waiting.clone(),
            waiting,
            due: Instant::now() + GO_BACK_WAIT,
        };
        tracing::info!(game = %self.id, seat = seat.index(), "go-back asked");
        self.seats.broadcast(&request.asked());
        self.go_backs.request = Some(request);
        Ok(())
    }

    /// `seat`'s answer to the open request: a decline ends it, the last agreement carries it out.
    pub(crate) async fn vote_go_back(
        &mut self,
        seat: PlayerId,
        approve: bool,
    ) -> Result<(), Fatal> {
        let Some(request) = self.go_backs.request.as_mut() else {
            self.refuse(seat, bad("no go-back is being asked"));
            return Ok(());
        };
        if !request.waiting.contains(&seat) {
            self.refuse(seat, bad("no answer of yours is awaited"));
            return Ok(());
        }
        if !approve {
            self.end_go_back(TakeBackOutcome::Declined { by: seat });
            return Ok(());
        }
        request.waiting.retain(|waiting| *waiting != seat);
        if !request.waiting.is_empty() {
            let asked = request.asked();
            self.seats.broadcast(&asked);
            return Ok(());
        }
        // Taken out first, so the step's news finds no request to withdraw.
        let Some(request) = self.go_backs.request.take() else {
            return Ok(());
        };
        let outcome = if self.went_back(&request.offer).await? {
            TakeBackOutcome::Approved
        } else {
            TakeBackOutcome::Withdrawn
        };
        self.go_backs.request = Some(request);
        self.end_go_back(outcome);
        Ok(())
    }

    /// The asker withdraws its open request.
    pub(crate) fn cancel_go_back(&mut self, seat: PlayerId) {
        let mine = self
            .go_backs
            .request
            .as_ref()
            .is_some_and(|request| request.offer.player == seat);
        if mine {
            self.end_go_back(TakeBackOutcome::Withdrawn);
        } else {
            self.refuse(seat, bad("no go-back of yours is being asked"));
        }
    }

    /// Ends the open request as declined if its time is up; whether it was.
    pub(crate) fn go_back_expired(&mut self) -> bool {
        let Some(request) = &self.go_backs.request else {
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
        self.end_go_back(TakeBackOutcome::TimedOut { by });
        true
    }

    /// When the open request counts as declined.
    pub(crate) fn go_back_due(&self) -> Option<Instant> {
        self.go_backs.due()
    }

    /// Closes the open request and tells every seat how it ended.
    fn end_go_back(&mut self, outcome: TakeBackOutcome) {
        if let Some(request) = self.go_backs.request.take() {
            tracing::info!(game = %self.id, seat = request.offer.player.index(), ?outcome, "go-back ended");
            self.seats.broadcast(&ServerMessage::GoBackEnded {
                offer: request.offer,
                outcome,
            });
        }
    }

    /// Cuts the log back to `offer`'s pass if it is still on offer, ends the asker's yield and asks it at that pass
    /// again; whether the go-back was made and saved.
    async fn went_back(&mut self, offer: &GoBackOffer) -> Result<bool, Fatal> {
        let Phase::Running(running) = &mut self.phase else {
            return Ok(false);
        };
        let seat = offer.player;
        let still = self.go_backs.offer(&running.engine, seat).as_ref() == Ok(offer);
        let step = if still {
            running.go_back(seat, offer.at).ok()
        } else {
            None
        };
        let Some(step) = step else {
            let reason = GoBackRefusal::MovedOn;
            self.seats
                .send(seat, ServerMessage::GoBackRefused { reason });
            return Ok(false);
        };
        tracing::info!(game = %self.id, seat = seat.index(), "went back a step");
        let mut yields = self.yields.clone();
        yields.cancel(seat);
        let mut stops = self.stops.clone();
        // The seat chose to act at its pass: it is asked there again, whatever the windows would pass for it.
        if stops.full_control(seat) == FullControlMode::Off {
            stops.set_full_control(seat, FullControlMode::NextWindow);
        }
        self.publish_with(step, yields, stops).await?;
        let Phase::Running(running) = &self.phase else {
            return Ok(false);
        };
        // A step that could not be saved is undone, and the pass is on offer again.
        Ok(self.go_backs.offer(&running.engine, seat).as_ref() != Ok(offer))
    }

    /// After a step: withdraws a request the step made moot, asks each person who passed since it was opened, and tells
    /// each connected seat the step it may go back to now, when that changed.
    pub(crate) fn go_back_news(&mut self) {
        let Phase::Running(running) = &self.phase else {
            return;
        };
        let mut moot = false;
        let mut fresh = Vec::new();
        if let Some(request) = &self.go_backs.request {
            let offer = self.go_backs.offer(&running.engine, request.offer.player);
            moot = offer.as_ref() != Ok(&request.offer);
            fresh = self
                .go_backs
                .consenting(&running.engine, &request.offer)
                .into_iter()
                .filter(|by| !request.asked.contains(by))
                .collect();
        }
        let offers = self
            .seats
            .connected()
            .into_iter()
            .map(|seat| (seat, self.go_backs.offer(&running.engine, seat).ok()))
            .collect::<Vec<_>>();
        if moot {
            self.end_go_back(TakeBackOutcome::Withdrawn);
        } else if let Some(request) = self.go_backs.request.as_mut()
            && !fresh.is_empty()
        {
            request.asked.extend(fresh.iter().copied());
            request.waiting.extend(fresh);
            let asked = request.asked();
            self.seats.broadcast(&asked);
        }
        for (seat, offer) in offers {
            if self.go_backs.tell(seat, offer.clone()) {
                self.seats.send(seat, ServerMessage::GoBackOffer { offer });
            }
        }
    }

    /// What a seat that (re)joins is told: the step it may go back to, if any, and the open request.
    pub(crate) fn go_back_sync(&mut self, seat: PlayerId) {
        let Phase::Running(running) = &self.phase else {
            return;
        };
        let offer = self.go_backs.offer(&running.engine, seat).ok();
        self.go_backs.tell(seat, offer.clone());
        if offer.is_some() {
            self.seats.send(seat, ServerMessage::GoBackOffer { offer });
        }
        if let Some(request) = &self.go_backs.request {
            let asked = request.asked();
            self.seats.send(seat, asked);
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a seat goes back only to a pass it sent itself, whoever passed since, so the offer never shows where a seat was asked; every person who passed since, by hand or by the table, is asked, and a bot never is.
    use super::*;
    use mtg_engine::Answer;
    use mtg_engine::decision::PriorityAction;
    use mtg_engine::turn::Step;
    use mtg_testkit::Scenario;
    use rstest::rstest;

    const SEATS: [PlayerId; 2] = [PlayerId(0), PlayerId(1)];

    /// Both seats pass the precombat main phase; each sent pass is noted as `sent` says.
    fn passed(sent: [bool; 2]) -> (Engine, GoBacks) {
        let mut engine = Scenario::new(2).build().unwrap();
        let mut go_backs = GoBacks::default();
        for seat in SEATS {
            let at = u64::try_from(engine.events().len()).unwrap();
            engine
                .apply(Answer::Priority(PriorityAction::Pass))
                .unwrap();
            if sent[seat.index()] {
                go_backs.sent(seat, at);
            }
        }
        assert_eq!(engine.state().turn().step(), Step::BeginCombat);
        (engine, go_backs)
    }

    #[rstest]
    #[case::a_person_the_table_passed_for([true, false], false, Ok(Step::Main1), &[PlayerId(1)])]
    #[case::a_person_passed_by_hand([true, true], false, Ok(Step::Main1), &[PlayerId(1)])]
    #[case::a_bot_passed([true, true], true, Ok(Step::Main1), &[])]
    #[case::the_table_passed_for_a_bot([true, false], true, Ok(Step::Main1), &[])]
    #[case::the_table_passed_for_the_seat([false, false], true, Err(GoBackRefusal::Nothing), &[])]
    fn only_the_seats_own_pass_is_offered_and_every_person_since_is_asked(
        #[case] sent: [bool; 2],
        #[case] bot: bool,
        #[case] expected: Result<Step, GoBackRefusal>,
        #[case] asked: &[PlayerId],
    ) {
        let (engine, mut go_backs) = passed(sent);
        if bot {
            go_backs.mark_bot(SEATS[1]);
        }
        let offer = go_backs.offer(&engine, SEATS[0]);
        assert_eq!(
            offer.as_ref().map(|offer| offer.step).map_err(Clone::clone),
            expected
        );
        if let Ok(offer) = &offer {
            assert_eq!(go_backs.consenting(&engine, offer), asked);
        }
        go_backs.cut(0);
        assert_eq!(
            go_backs.offer(&engine, SEATS[0]).map(|offer| offer.step),
            Err(GoBackRefusal::Nothing)
        );
    }
}
