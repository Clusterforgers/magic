//! The lesson's bot: plays its script by turn and step, and otherwise passes, never attacks or blocks, keeps its hand
//! and lets the engine pay. Each answer waits the beat, so the learner sees every play.

use std::time::Duration;

use mtg_core::ids::ObjectId;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{MulliganChoice, PayChoice, PriorityAction};
use mtg_engine::{Answer, Decision, DecisionKind};
use mtg_protocol::{ClientMessage, ServerMessage, StateView};
use tokio::task::JoinHandle;

use super::lessons::{BotAct, BotLine};
use super::{BOT_SEAT, PLAYER_SEAT};
use crate::handle::GameHandle;

/// The bot's memory: what it sees, which lines of its script it has played and the target its last cast aims at.
struct Bot {
    script: &'static [BotLine],
    done: Vec<bool>,
    aim: Option<&'static str>,
    view: Option<StateView>,
}

impl Bot {
    fn new(script: &'static [BotLine]) -> Self {
        Bot {
            script,
            done: vec![false; script.len()],
            aim: None,
            view: None,
        }
    }

    /// The name of object `id`, as the bot sees it.
    fn name(&self, id: ObjectId) -> &str {
        self.view
            .as_ref()
            .and_then(|view| view.objects.iter().find(|object| object.id == id))
            .map_or("", |object| object.name.as_str())
    }

    /// The first of `ids` called `name`.
    fn named(&self, ids: impl IntoIterator<Item = ObjectId>, name: &str) -> Option<ObjectId> {
        ids.into_iter().find(|&id| self.name(id) == name)
    }

    /// The lines of the script for the current turn that are still to play, with their index.
    fn due(&self) -> Vec<(usize, BotAct)> {
        let Some(turn) = self.view.as_ref().map(|view| &view.table.turn) else {
            return Vec::new();
        };
        self.script
            .iter()
            .enumerate()
            .filter(|&(index, &(at, step, _))| {
                !self.done[index] && at == turn.number && step == turn.step
            })
            .map(|(index, &(_, _, act))| (index, act))
            .collect()
    }

    /// The bot's answer to `decision`.
    fn answer(&mut self, decision: &Decision) -> Option<Answer> {
        match &decision.kind {
            DecisionKind::Mulligan(_) => Some(Answer::Mulligan(MulliganChoice::Keep)),
            DecisionKind::Priority(options) => {
                for (index, act) in self.due() {
                    let action = match act {
                        BotAct::Land(name) => self
                            .named(options.playable_lands.iter().copied(), name)
                            .map(PriorityAction::PlayLand),
                        BotAct::Cast(name, aim) => {
                            self.aim = aim;
                            self.named(options.castable.iter().copied(), name)
                                .map(PriorityAction::CastSpell)
                        }
                        _ => continue,
                    };
                    self.done[index] = true;
                    if let Some(action) = action {
                        return Some(Answer::Priority(action));
                    }
                }
                Some(Answer::Priority(PriorityAction::Pass))
            }
            DecisionKind::DeclareAttackers(options) => {
                let mut attack = Vec::new();
                for (index, act) in self.due() {
                    let BotAct::Attack(names) = act else { continue };
                    self.done[index] = true;
                    let at = AttackTarget::Player(PLAYER_SEAT);
                    for name in names {
                        let free = options.attackers.iter().filter(|option| {
                            option.defenders.contains(&at)
                                && option.tax.is_empty()
                                && !attack.iter().any(|&(id, _)| id == option.object)
                        });
                        if let Some(id) = self.named(free.map(|option| option.object), name) {
                            attack.push((id, at));
                        }
                    }
                }
                Some(Answer::DeclareAttackers(attack))
            }
            DecisionKind::DeclareBlockers(options) => {
                let mut blocks = Vec::new();
                for (index, act) in self.due() {
                    let BotAct::Block(blocker, attacker) = act else {
                        continue;
                    };
                    self.done[index] = true;
                    let found = options.blockers.iter().find_map(|option| {
                        (self.name(option.object) == blocker)
                            .then(|| self.named(option.can_block.iter().copied(), attacker))
                            .flatten()
                            .map(|attacker| (option.object, attacker))
                    });
                    blocks.extend(found);
                }
                Some(Answer::DeclareBlockers(blocks))
            }
            DecisionKind::ChooseTargets(options) => {
                let aim = self.aim.take();
                let target = options.legal.iter().copied().find(|target| {
                    matches!(target, Target::Object(object)
                        if aim.is_some_and(|aim| self.name(object.id) == aim))
                });
                match target {
                    Some(target) => {
                        Some(Answer::ChooseTargets(TargetChoice::Targets(vec![target])))
                    }
                    None => crate::pace::fallback(decision),
                }
            }
            DecisionKind::PayMana(options) if options.auto_available => {
                Some(Answer::PayMana(PayChoice::Auto))
            }
            _ => crate::pace::fallback(decision),
        }
    }
}

/// Plays the bot's seat of `handle` by `script` until the game ends or closes: readies both seats, then answers each
/// decision sent to the bot after `beat`.
pub fn spawn(handle: GameHandle, script: &'static [BotLine], beat: Duration) -> JoinHandle<()> {
    tokio::spawn(async move {
        let Ok(mut inbox) = handle.join(BOT_SEAT).await else {
            return;
        };
        for seat in [BOT_SEAT, PLAYER_SEAT] {
            if handle.send(seat, ClientMessage::Ready).await.is_err() {
                return;
            }
        }
        // The game starts with the second Ready; a snapshot asked for now gives the bot its view of the table.
        if handle.send(BOT_SEAT, ClientMessage::Resync).await.is_err() {
            return;
        }
        let mut bot = Bot::new(script);
        while let Some(message) = inbox.recv().await {
            match message {
                ServerMessage::Snapshot { state, .. } => bot.view = Some(*state),
                ServerMessage::Events(batch) => {
                    if let Some(view) = bot.view.as_mut() {
                        view.apply(&batch);
                    }
                }
                ServerMessage::Decision { id, decision, .. } => {
                    let Some(answer) = bot.answer(&decision) else {
                        continue;
                    };
                    if !beat.is_zero() {
                        tokio::time::sleep(beat).await;
                    }
                    let reply = ClientMessage::Answer {
                        decision: id,
                        answer,
                    };
                    if handle.send(BOT_SEAT, reply).await.is_err() {
                        return;
                    }
                }
                ServerMessage::GameOver { .. } | ServerMessage::EndedByAdmin { .. } => return,
                _ => {}
            }
        }
    })
}
