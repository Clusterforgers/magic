//! Turn-based actions and step transitions (CR 500-514).

use mtg_core::ids::PlayerId;

use crate::characteristics::controller_of;
use crate::combat::{attack_options, block_options, defending_players};
use crate::damage::{DamagePass, damage_events, first_strike_step_needed, pending_assignment};
use crate::decision::{Decision, DecisionKind, DiscardOptions};
use crate::effects::Duration;
use crate::events::{Event, NothingToDo};
use crate::flow::Progress;
use crate::object::{GameObject, PermanentState};
use crate::priority::{all_passed, priority_options};
use crate::rules::cant_draw;
use crate::stack::resolve_top;
use crate::state::GameState;
use crate::turn::{MAX_HAND_SIZE, Step};
use crate::zones::Zone;

/// The next batch of events or decision for the current step.
pub(crate) fn next(state: &GameState) -> Progress {
    if !state.is_alive(state.turn().active()) {
        return Progress::Events(end_turn(state));
    }
    if state.priority().is_some() {
        priority_round(state)
    } else {
        begin_step(state, state.turn().step())
    }
}

fn grant(state: &GameState) -> Event {
    Event::PriorityGranted {
        player: state.turn().active(),
    }
}

fn begin_step(state: &GameState, step: Step) -> Progress {
    let active = state.turn().active();
    match step {
        Step::Untap => {
            let mut events: Vec<Event> = state
                .zone(Zone::Battlefield)
                .iter()
                .filter(|id| {
                    controller_of(state, **id) == Some(active)
                        && state
                            .object(**id)
                            .and_then(GameObject::permanent)
                            .is_some_and(PermanentState::tapped)
                })
                .map(|id| Event::PermanentUntapped { object: *id })
                .collect();
            events.extend(leave_step(state));
            Progress::Events(events)
        }
        Step::Draw => {
            let skipped = state.turn().number() == 1
                && state.format().skips_first_draw(state.players().len());
            let mut events = if skipped {
                Vec::new()
            } else {
                draw(state, active, 1)
            };
            events.push(grant(state));
            Progress::Events(events)
        }
        Step::DeclareAttackers => declare_attackers(state),
        Step::DeclareBlockers => declare_blockers(state),
        Step::FirstStrikeDamage => combat_damage(state, DamagePass::FirstStrike),
        Step::CombatDamage => combat_damage(state, DamagePass::Regular),
        Step::Cleanup => cleanup(state),
        Step::Upkeep
        | Step::Main1
        | Step::BeginCombat
        | Step::EndCombat
        | Step::Main2
        | Step::End => Progress::Events(vec![grant(state)]),
    }
}

/// Events for `player` drawing `count` cards, or failing to on an empty library. A player who can't draw cards neither
/// draws nor fails to.
pub(crate) fn draw(state: &GameState, player: PlayerId, count: usize) -> Vec<Event> {
    if cant_draw(state, player) {
        return Vec::new();
    }
    let library = state.zone(Zone::Library(player));
    (0..count)
        .map(|i| match library.iter().rev().nth(i) {
            Some(object) => Event::CardDrawn {
                player,
                object: *object,
            },
            None => Event::DrewFromEmptyLibrary { player },
        })
        .collect()
}

fn declare_attackers(state: &GameState) -> Progress {
    let active = state.turn().active();
    if !state.combat().attackers_declared() {
        let options = attack_options(state, active);
        if options.attackers.is_empty() {
            return Progress::Events(vec![
                Event::NothingToDo {
                    player: active,
                    kind: NothingToDo::Attack,
                },
                Event::AttackersDeclared {
                    player: active,
                    attackers: Vec::new(),
                },
            ]);
        }
        return Progress::Decide(Decision {
            player: active,
            kind: DecisionKind::DeclareAttackers(options),
        });
    }
    if state.combat().attackers().is_empty() {
        return Progress::Events(leave_step(state));
    }
    Progress::Events(vec![grant(state)])
}

fn declare_blockers(state: &GameState) -> Progress {
    for player in defending_players(state) {
        if state.combat().blockers_declared().contains(&player) {
            continue;
        }
        let options = block_options(state, player);
        if options.blockers.is_empty() {
            return Progress::Events(vec![
                Event::NothingToDo {
                    player,
                    kind: NothingToDo::Block,
                },
                Event::BlockersDeclared {
                    player,
                    blocks: Vec::new(),
                },
            ]);
        }
        return Progress::Decide(Decision {
            player,
            kind: DecisionKind::DeclareBlockers(options),
        });
    }
    Progress::Events(vec![grant(state)])
}

fn combat_damage(state: &GameState, pass: DamagePass) -> Progress {
    if let Some((player, options)) = pending_assignment(state, pass) {
        return Progress::Decide(Decision {
            player,
            kind: DecisionKind::AssignDamage(options),
        });
    }
    let mut events = damage_events(state, pass);
    events.push(grant(state));
    Progress::Events(events)
}

fn priority_round(state: &GameState) -> Progress {
    if all_passed(state) {
        return Progress::Events(match resolve_top(state) {
            Some(resolution) => vec![resolution],
            None => leave_step(state),
        });
    }
    match state.priority() {
        Some(priority) => Progress::Decide(Decision {
            player: priority.holder(),
            kind: DecisionKind::Priority(priority_options(state, priority.holder())),
        }),
        None => Progress::Events(vec![grant(state)]),
    }
}

fn next_step(state: &GameState, step: Step) -> Step {
    match step {
        Step::DeclareAttackers if state.combat().attackers().is_empty() => Step::EndCombat,
        Step::DeclareBlockers if !first_strike_step_needed(state) => Step::CombatDamage,
        other => other.next().unwrap_or(Step::Cleanup),
    }
}

/// Empties mana pools (CR 500.4), closes combat if leaving it, and starts the next step.
pub(crate) fn leave_step(state: &GameState) -> Vec<Event> {
    let step = state.turn().step();
    let mut events = pool_emptying(state);
    if step == Step::EndCombat {
        events.push(Event::CombatEnded);
    }
    events.push(Event::StepBegan {
        step: next_step(state, step),
    });
    events
}

fn pool_emptying(state: &GameState) -> Vec<Event> {
    state
        .players()
        .iter()
        .filter(|p| !p.mana().is_empty())
        .map(|p| Event::ManaPoolEmptied { player: p.id() })
        .collect()
}

fn cleanup(state: &GameState) -> Progress {
    let active = state.turn().active();
    let hand = state.zone(Zone::Hand(active));
    if hand.len() > MAX_HAND_SIZE {
        return Progress::Decide(Decision {
            player: active,
            kind: DecisionKind::Discard(DiscardOptions {
                hand: hand.to_vec(),
                count: hand.len() - MAX_HAND_SIZE,
            }),
        });
    }
    // CR 514.3a: an ability that triggered during cleanup gets a round of priority, then cleanup begins again.
    if !state.stack().is_empty() {
        return Progress::Events(vec![grant(state)]);
    }
    Progress::Events(end_turn(state))
}

/// Ends the turn: damage wears off, "until end of turn" effects end, and the next player's turn begins.
///
/// Also used when the active player leaves the game, which abandons the rest of their turn.
pub(crate) fn end_turn(state: &GameState) -> Vec<Event> {
    let mut events = Vec::new();
    let damaged = state
        .objects()
        .any(|o| o.permanent().is_some_and(|p| p.damage() > 0));
    if damaged {
        events.push(Event::DamageCleared);
    }
    events.extend(
        state
            .effects()
            .iter()
            .filter(|e| e.duration == Duration::UntilEndOfTurn)
            .map(|e| Event::EffectExpired { id: e.id }),
    );
    events.extend(
        state
            .rule_effects()
            .iter()
            .filter(|e| e.duration == Duration::UntilEndOfTurn)
            .map(|e| Event::RuleExpired { id: e.id }),
    );
    events.extend(pool_emptying(state));
    let active = state.turn().active();
    events.push(Event::TurnBegan {
        number: state.turn().number() + 1,
        active: state.next_alive_after(active).unwrap_or(active),
    });
    events.push(Event::StepBegan { step: Step::Untap });
    events
}
