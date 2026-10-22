//! Turn-based actions and step transitions (CR 500-514).

use mtg_core::ids::PlayerId;

use crate::characteristics::controller_of;
use crate::combat::{attack_options, block_options, defending_players};
use crate::damage::{DamagePass, damage_events, first_strike_step_needed, pending_assignment};
use crate::decision::{Decision, DecisionKind, DiscardOptions};
use crate::effects::ends_with_turn;
use crate::events::{Event, NothingToDo};
use crate::flow::Progress;
use crate::object::{GameObject, PermanentState};
use crate::player::Player;
use crate::priority::{all_passed, priority_options};
use crate::rules::{cant_draw, no_max_hand_size};
use crate::stack::resolve_top;
use crate::state::{GameResult, GameState};
use crate::turn::Step;
use crate::zones::Zone;

/// The next batch of events or decision for the current step.
///
/// A turn whose active player has left the game goes on without one (CR 800.4): the turn-based actions that
/// belong to the active player (untap, draw, attack, discard down) are skipped and every step that grants
/// priority starts with the next player in turn order.
pub(crate) fn next(state: &GameState) -> Progress {
    if state.priority().is_some() {
        priority_round(state)
    } else {
        begin_step(state, state.turn().step())
    }
}

fn grant(state: &GameState) -> Event {
    Event::PriorityGranted {
        player: state.active_or_next(),
    }
}

fn begin_step(state: &GameState, step: Step) -> Progress {
    let active = state.active_player();
    match step {
        Step::Untap => {
            let mut events: Vec<Event> = state
                .zone(Zone::Battlefield)
                .iter()
                .filter(|id| {
                    active.is_some()
                        && controller_of(state, **id) == active
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
                && state.ruleset().skips_first_draw(state.players().len());
            let mut events = match active {
                Some(active) if !skipped => draw(state, active, state.ruleset().draw_step_cards()),
                _ => Vec::new(),
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
    let seat = state.turn().active();
    if !state.combat().attackers_declared() {
        // Nobody declares attackers in a turn without an active player.
        let Some(active) = state.active_player() else {
            return Progress::Events(vec![Event::AttackersDeclared {
                player: seat,
                attackers: Vec::new(),
            }]);
        };
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
        return Progress::Events(resolve_top(state).unwrap_or_else(|| leave_step(state)));
    }
    match state.priority() {
        Some(priority) => Progress::Decide(Decision {
            player: priority.holder(),
            kind: DecisionKind::Priority(Box::new(priority_options(state, priority.holder()))),
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

/// Empties every mana pool; under the house rule of mana burn, a player in the game loses 1 life for each mana that
/// was left, restricted mana included.
fn pool_emptying(state: &GameState) -> Vec<Event> {
    let burn = state.ruleset().mana_burn();
    state
        .players()
        .iter()
        .filter(|p| !p.mana().is_empty())
        .flat_map(|p| {
            let (player, amount) = (p.id(), p.mana().total());
            let mut events = vec![Event::ManaPoolEmptied { player }];
            if burn && p.is_in_game() {
                events.push(Event::ManaBurned { player, amount });
                events.push(Event::LifeLost { player, amount });
            }
            events
        })
        .collect()
}

fn cleanup(state: &GameState) -> Progress {
    if let Some(active) = state.active_player() {
        let hand = state.zone(Zone::Hand(active));
        let most = state
            .ruleset()
            .max_hand_size()
            .filter(|_| !no_max_hand_size(state, active));
        if let Some(most) = most.filter(|most| hand.len() > *most) {
            return Progress::Decide(Decision {
                player: active,
                kind: DecisionKind::Discard(DiscardOptions {
                    hand: hand.to_vec(),
                    count: hand.len() - most,
                }),
            });
        }
    }
    // CR 514.3a: an ability that triggered during cleanup gets a round of priority, then cleanup begins again.
    if !state.stack().is_empty() {
        return Progress::Events(vec![grant(state)]);
    }
    Progress::Events(end_turn(state))
}

/// Ends the turn: damage wears off, "until end of turn" effects end, and the next player's turn begins.
///
/// The next turn goes to the next player in turn order after the active player, who may have left the game.
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
            .filter(|e| ends_with_turn(state, e.duration, e.turn, e.controller))
            .map(|e| Event::EffectExpired { id: e.id }),
    );
    events.extend(
        state
            .rule_effects()
            .iter()
            .filter(|e| ends_with_turn(state, e.duration, e.turn, e.controller))
            .map(|e| Event::RuleExpired { id: e.id }),
    );
    // "Until end of turn" copy effects end with the other effects of the turn (CR 514.2).
    events.extend(
        state
            .objects()
            .filter(|o| {
                o.copy_effect()
                    .is_some_and(crate::copies::CopyEffect::is_temporary)
            })
            .map(|o| Event::CopyEnded { object: o.id() }),
    );
    events.extend(pool_emptying(state));
    if turn_limit_reached(state) {
        // The turn is wound down first, so the result reads the life totals after the mana burn.
        return if events.is_empty() {
            vec![Event::GameEnded {
                result: highest_life(state),
            }]
        } else {
            events
        };
    }
    let active = state.turn().active();
    events.push(Event::TurnBegan {
        number: state.turn().number() + 1,
        active: state.next_alive_after(active).unwrap_or(active),
    });
    // The spells the ending turn's active player cast are counted before the new turn clears the count (CR 726.3a).
    if let Some(to) = crate::day_night::at_turn_start(state) {
        events.push(Event::DayNightChanged { to });
    }
    events.push(Event::StepBegan { step: Step::Untap });
    events
}

/// Whether the turn that is ending is the last of the last round the house rule allows.
fn turn_limit_reached(state: &GameState) -> bool {
    state
        .ruleset()
        .turn_limit()
        .is_some_and(|limit| state.turn().round() >= limit && state.ends_round())
}

/// The result of a game stopped by the turn limit: the player with the highest life total wins, a tie for the
/// highest is a draw, and a player who has lost does not count.
fn highest_life(state: &GameState) -> GameResult {
    let alive = || state.players().iter().filter(|p| p.is_in_game());
    let highest = alive().map(Player::life).max();
    let mut leaders = alive().filter(|p| Some(p.life()) == highest);
    match (leaders.next(), leaders.next()) {
        (Some(only), None) => GameResult::Winner(only.id()),
        _ => GameResult::Draw,
    }
}
