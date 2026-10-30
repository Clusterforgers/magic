//! The opening asks every seat at once: each seat's decision has its own id and clock, and the one whose clock runs out
//! first is the one the table answers for.

use mtg_protocol::DecisionId;
use tokio::time::Instant;

use super::{Issued, Pending, Running, millis};

impl Running {
    /// Every decision a seat may answer now: the engine's own, then the other seats' in the opening.
    pub(super) fn all(&self) -> impl Iterator<Item = &Pending> {
        self.pending.iter().chain(self.side.iter())
    }

    /// The decision whose clock runs out first (the first in order on a tie), else the engine's own.
    pub(super) fn expiring(&self) -> Option<&Pending> {
        self.all()
            .filter(|p| p.due.is_some())
            .min_by_key(|p| p.due)
            .or(self.pending.as_ref())
    }

    /// [`expiring`](Running::expiring), to change its clock.
    pub(super) fn expiring_mut(&mut self) -> Option<&mut Pending> {
        let due = self.all().filter_map(|p| p.due).min();
        match due {
            Some(due) => self
                .pending
                .iter_mut()
                .chain(self.side.iter_mut())
                .find(|p| p.due == Some(due)),
            None => self.pending.as_mut(),
        }
    }

    /// Aligns the opening's decisions with the engine's: a decision that is still asked keeps its id and clock, so an
    /// answer of one seat never restarts or invalidates another's; every new one gets a fresh id and the opening clock,
    /// and is returned to be sent.
    pub(super) fn settle_opening(&mut self) -> Vec<Issued> {
        self.clock_kept = false;
        let mut pool: Vec<Pending> = self.pending.take().into_iter().collect();
        pool.append(&mut self.side);
        let mut asked = Vec::new();
        let mut issued = Vec::new();
        for decision in self.engine.decisions() {
            if let Some(at) = pool
                .iter()
                .position(|p| p.decision == decision && !p.hidden)
            {
                asked.push(pool.swap_remove(at));
                continue;
            }
            self.last_id += 1;
            let id = DecisionId(self.last_id);
            let clock = self.pace.opening_clock();
            let now = Instant::now();
            let delay = clock
                .map(|span| self.pace.shown_after(span))
                .filter(|delay| !delay.is_zero());
            asked.push(Pending {
                id,
                decision: decision.clone(),
                due: clock.and_then(|span| now.checked_add(span)),
                shown_at: delay.and_then(|delay| now.checked_add(delay)),
                hidden: false,
            });
            issued.push(Issued {
                id,
                decision,
                deadline_ms: clock.map(millis),
                show_after_ms: delay.map(millis),
            });
        }
        let mut asked = asked.into_iter();
        self.pending = asked.next();
        self.side = asked.collect();
        issued
    }
}
