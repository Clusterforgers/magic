//! The engine's side of loops: noting each answer and priority point, ending a mandatory loop, asking the driver of an
//! optional one, and running the iterations chosen as one batch.

use mtg_core::ids::PlayerId;

use super::Engine;
use crate::decision::{Answer, Decision, DecisionKind};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::loops::{
    Found, How, LOOP_CAP, LOOP_WORK, LoopAnswer, LoopLife, LoopOptions, LoopStep, LoopSummary,
    LoopTokens, Offer, Taken,
};
use crate::state::{GameResult, Stage};

impl Engine {
    /// How the answer about to be given counts for loops, while the engine watches for them.
    pub(super) fn loop_how(&self, decision: &Decision, answer: &Answer) -> Option<How> {
        (!self.looping && self.state.stage() == Stage::Turns)
            .then(|| How::of(&self.state, decision, answer))
    }

    /// Notes the answer given and the priority point it led to; a loop the point closes is ended, run or offered, and
    /// its events join `events`.
    pub(super) fn watch_loops(
        &mut self,
        taken: Taken,
        events: &mut Vec<Event>,
    ) -> Result<(), EngineError> {
        self.watch.take(taken);
        let Some(decision) = self.decision.as_ref() else {
            return Ok(());
        };
        if !matches!(decision.kind, DecisionKind::Priority(_))
            || self.pending.is_some()
            || self.held.is_some()
            || self.state.stage() != Stage::Turns
        {
            return Ok(());
        }
        let holder = decision.player;
        let Some(found) = self.watch.mark(&self.state, holder, self.events.len()) else {
            return Ok(());
        };
        let bounded = found.bounded();
        if found.mandatory() {
            // No one can do anything else: a loop that never ends is a draw (CR 104.4b), one that ends runs to its end.
            let more = if bounded {
                self.repeat_loop(&found, None, LOOP_CAP)?
            } else {
                self.draw_by_loop(&found)?
            };
            events.extend(more);
            return Ok(());
        }
        let driven = !found.drivers.is_empty();
        // A loop that spends something other than life (mana, cards) ends by itself and needs no shortcut.
        if driven && bounded && !found.drains_life_only() {
            return Ok(());
        }
        let candidates = if driven {
            &found.drivers
        } else {
            &found.responders
        };
        let player = self.first_in_turn_order(candidates);
        let options = LoopOptions {
            cards: found.cards.clone(),
            per_iteration: LoopStep {
                tokens: tokens_of(&self.events[found.log.0..found.log.1]),
                life: found.life.clone(),
            },
            mandatory: !driven,
            bounded,
            max: LOOP_CAP.saturating_sub(self.watch.run),
            asked: self.watch.offered(found.key),
            can_draw: !driven && !bounded,
        };
        self.offer = Some(Offer {
            found,
            options: options.clone(),
        });
        self.set_decision(Some(Decision {
            player,
            kind: DecisionKind::Loop(options),
        }));
        Ok(())
    }

    /// The first of `players` in turn order from the active player (CR 732.4: the active player, or the first
    /// involved after them, makes the different choice).
    fn first_in_turn_order(&self, players: &[PlayerId]) -> PlayerId {
        self.state
            .turn_order_from(self.state.active_or_next())
            .into_iter()
            .find(|player| players.contains(player))
            .or_else(|| players.first().copied())
            .unwrap_or_else(|| self.state.active_or_next())
    }

    /// Answers the loop offer the engine waits on.
    pub(super) fn answer_loop(&mut self, answer: Answer) -> Result<Vec<Event>, EngineError> {
        let Some(offer) = self.offer.as_ref() else {
            return Err(EngineError::NoDecision);
        };
        let Answer::Loop(choice) = answer else {
            return Err(IllegalAnswer::WrongKind {
                expected: "loop",
                got: answer.name(),
            }
            .into());
        };
        let player = self.decision.as_ref().map(|decision| decision.player);
        match choice {
            LoopAnswer::Repeat(times) if times == 0 || times > offer.options.max => {
                Err(IllegalAnswer::NotOffered("number of iterations").into())
            }
            LoopAnswer::Draw if !offer.options.can_draw => {
                Err(IllegalAnswer::NotOffered("draw").into())
            }
            LoopAnswer::Repeat(times) => {
                let found = offer.found.clone();
                self.offer = None;
                self.advance()?;
                self.repeat_loop(&found, player, times)
            }
            LoopAnswer::Draw => {
                let found = offer.found.clone();
                self.offer = None;
                self.draw_by_loop(&found)
            }
            LoopAnswer::Stop => {
                self.offer = None;
                self.watch.clear();
                self.advance()?;
                Ok(Vec::new())
            }
        }
    }

    /// Ends the game in a draw for every player still in it (CR 104.4b, 732.4).
    fn draw_by_loop(&mut self, found: &Found) -> Result<Vec<Event>, EngineError> {
        let mark = self.events.len();
        self.watch.clear();
        self.commit(vec![
            Event::LoopDrawn {
                cards: found.cards.clone(),
            },
            Event::GameEnded {
                result: GameResult::Draw,
            },
        ])?;
        self.advance()?;
        Ok(self.events[mark..].to_vec())
    }

    /// Repeats the cycle of `found` up to `times` times from the priority point it starts at, giving each recorded
    /// answer with the ids of the iteration; it stops early when the game ends, a question differs or an answer is no
    /// longer legal (CR 732.5). Logs `LoopRepeated` after the events it made.
    fn repeat_loop(
        &mut self,
        found: &Found,
        player: Option<PlayerId>,
        times: u32,
    ) -> Result<Vec<Event>, EngineError> {
        let mark = self.events.len();
        let lives: Vec<(PlayerId, i64)> = self.lives();
        self.looping = true;
        let mut done = 0;
        let mut work = 0usize;
        'run: while done < times && self.state.result().is_none() {
            let starts = self.decision.as_ref().is_some_and(|decision| {
                matches!(decision.kind, DecisionKind::Priority(_))
                    && self.pending.is_none()
                    && crate::loops::shape_at(&self.state, decision.player) == found.start
            });
            if !starts {
                break;
            }
            work += self
                .state
                .zone(crate::zones::Zone::Battlefield)
                .len()
                .max(1);
            if work > LOOP_WORK && done > 0 {
                break;
            }
            let base = self.state.next_object;
            for (taken, value) in &found.answers {
                let fits = self
                    .decision
                    .as_ref()
                    .is_some_and(|d| d.player == taken.player && d.kind.name() == taken.kind);
                let answer = crate::loops::absolute(value, base);
                let Some(answer) = answer.filter(|_| fits) else {
                    break 'run;
                };
                if self.apply(answer).is_err() {
                    break 'run;
                }
                if self.state.result().is_some() {
                    done += 1;
                    break 'run;
                }
            }
            done += 1;
        }
        self.looping = false;
        self.watch.run = self.watch.run.saturating_add(done);
        self.watch.clear();
        let made = self.events.len() - mark;
        let after = self.lives();
        let life = lives
            .iter()
            .zip(&after)
            .filter(|(before, now)| before.1 != now.1)
            .map(|(before, now)| LoopLife {
                player: before.0,
                change: now.1 - before.1,
            })
            .collect();
        let summary = LoopSummary {
            player,
            cards: found.cards.clone(),
            iterations: done,
            total: LoopStep {
                tokens: tokens_of(&self.events[mark..]),
                life,
            },
            events: u32::try_from(made).unwrap_or(u32::MAX),
        };
        let event = Event::LoopRepeated(Box::new(summary));
        crate::fold::apply_event(&mut self.state, &event)?;
        self.events.push(event);
        Ok(self.events[mark..].to_vec())
    }

    fn lives(&self) -> Vec<(PlayerId, i64)> {
        self.state
            .players()
            .iter()
            .map(|player| (player.id, i64::from(player.life)))
            .collect()
    }
}

/// The tokens `events` create, by controller and name.
fn tokens_of(events: &[Event]) -> Vec<LoopTokens> {
    let mut tokens: Vec<LoopTokens> = Vec::new();
    for event in events {
        if let Event::TokenCreated(token) = event {
            match tokens
                .iter_mut()
                .find(|seen| seen.controller == token.controller && seen.name == token.name)
            {
                Some(seen) => seen.count += 1,
                None => tokens.push(LoopTokens {
                    controller: token.controller,
                    name: token.name.clone(),
                    count: 1,
                }),
            }
        }
    }
    tokens
}
