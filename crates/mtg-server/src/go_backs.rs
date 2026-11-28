//! Going back a step ([server.md](../../../docs/server.md), Going back): a seat takes back its passes to an earlier
//! step on its own, while the engine finds nothing irreversible since and no other person passed by hand meanwhile.

use mtg_core::ids::PlayerId;
use mtg_engine::Engine;
use mtg_engine::go_back::{GoBackOffer, GoBackRefusal};
use mtg_protocol::{FullControlMode, ServerMessage};

use crate::actor::{Fatal, GameActor, Phase, not_started};
use crate::store::EventStore;

/// How many passes sent by the seats are kept.
const KEPT_SENT: usize = 128;

/// The passes the seats sent themselves, the seats bots play and what each seat was last told. Never stored.
#[derive(Debug, Default)]
pub(crate) struct GoBacks {
    /// Each pass a seat sent (not one the server gave for it): the seat and where its events begin in the log.
    sent: Vec<(PlayerId, u64)>,
    bots: Vec<PlayerId>,
    told: Vec<Option<GoBackOffer>>,
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

    /// The step `seat` may go back to: its latest own pass in an earlier step the engine allows, with no pass of
    /// another person since. A person's pass counts whether they sent it or the table gave it for them: the table asks
    /// a seat only where it can act, so telling the two apart would show the asker what that seat holds. A bot's pass
    /// never counts.
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
        let offer = engine.go_back_offer(seat, own)?;
        let person = engine
            .passed_since(seat, offer.at)
            .iter()
            .any(|by| !self.bots.contains(by));
        if person {
            return Err(GoBackRefusal::OtherPlayerActed);
        }
        Ok(offer)
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

impl<S: EventStore> GameActor<S> {
    /// `seat` goes back to the step its offer names: refused if it has none now, otherwise the log is cut back to its
    /// pass, its yield ends and it is asked at that pass again.
    pub(crate) async fn go_back(&mut self, seat: PlayerId) -> Result<(), Fatal> {
        let Phase::Running(running) = &mut self.phase else {
            self.refuse(seat, not_started());
            return Ok(());
        };
        let offer = match self.go_backs.offer(&running.engine, seat) {
            Ok(offer) => offer,
            Err(reason) => {
                self.seats
                    .send(seat, ServerMessage::GoBackRefused { reason });
                return Ok(());
            }
        };
        let Ok(step) = running.go_back(seat, offer.at) else {
            let reason = GoBackRefusal::MovedOn;
            self.seats
                .send(seat, ServerMessage::GoBackRefused { reason });
            return Ok(());
        };
        tracing::info!(game = %self.id, seat = seat.index(), "went back a step");
        let mut yields = self.yields.clone();
        yields.cancel(seat);
        let mut stops = self.stops.clone();
        // The seat chose to act at its pass: it is asked there again, whatever the windows would pass for it.
        if stops.full_control(seat) == FullControlMode::Off {
            stops.set_full_control(seat, FullControlMode::NextWindow);
        }
        self.publish_with(step, yields, stops).await
    }

    /// After a step: tells each connected seat the step it may go back to now, when that changed.
    pub(crate) fn go_back_news(&mut self) {
        let Phase::Running(running) = &self.phase else {
            return;
        };
        let offers = self
            .seats
            .connected()
            .into_iter()
            .map(|seat| (seat, self.go_backs.offer(&running.engine, seat).ok()))
            .collect::<Vec<_>>();
        for (seat, offer) in offers {
            if self.go_backs.tell(seat, offer.clone()) {
                self.seats.send(seat, ServerMessage::GoBackOffer { offer });
            }
        }
    }

    /// What a seat that (re)joins is told: the step it may go back to, if any.
    pub(crate) fn go_back_sync(&mut self, seat: PlayerId) {
        let Phase::Running(running) = &self.phase else {
            return;
        };
        let offer = self.go_backs.offer(&running.engine, seat).ok();
        self.go_backs.tell(seat, offer.clone());
        if offer.is_some() {
            self.seats.send(seat, ServerMessage::GoBackOffer { offer });
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a seat goes back only to a pass it sent itself, and any pass of another person since refuses it, sent or given by the table, so the offer never shows where a seat was asked; a bot's pass never does.
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
    #[case::a_person_the_table_passed_for([true, false], false, Err(GoBackRefusal::OtherPlayerActed))]
    #[case::a_person_passed_by_hand([true, true], false, Err(GoBackRefusal::OtherPlayerActed))]
    #[case::a_bot_passed([true, true], true, Ok(Step::Main1))]
    #[case::the_table_passed_for_a_bot([true, false], true, Ok(Step::Main1))]
    #[case::the_table_passed_for_the_seat([false, false], true, Err(GoBackRefusal::Nothing))]
    fn only_the_seats_own_pass_with_no_person_passing_since_is_offered(
        #[case] sent: [bool; 2],
        #[case] bot: bool,
        #[case] expected: Result<Step, GoBackRefusal>,
    ) {
        let (engine, mut go_backs) = passed(sent);
        if bot {
            go_backs.mark_bot(SEATS[1]);
        }
        let step = |go_backs: &GoBacks| go_backs.offer(&engine, SEATS[0]).map(|offer| offer.step);
        assert_eq!(step(&go_backs), expected);
        go_backs.cut(0);
        assert_eq!(step(&go_backs), Err(GoBackRefusal::Nothing));
    }
}
