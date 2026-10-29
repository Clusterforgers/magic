//! Turn-based actions and step transitions (CR 500-514).

use mtg_cards::spend::ManaLasts;
use std::collections::BTreeSet;

use mtg_core::ids::{ObjectId, PlayerId};

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
use crate::rules::{cant_draw, no_max_hand_size, skips_draw_step};
use crate::stack::resolve_top;
use crate::state::{GameResult, GameState};
use crate::turn::{ExtraTurn, Step};
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
            // Phasing comes first (CR 502.1): what phases in untaps with the rest.
            let leaving = active.map_or_else(Vec::new, |active| {
                crate::phasing::w7e_keyword_out_events(state, active)
            });
            let going: BTreeSet<ObjectId> = leaving
                .iter()
                .filter_map(|event| match event {
                    Event::PermanentPhasedOut { object, .. } => Some(*object),
                    _ => None,
                })
                .collect();
            let mut events =
                active.map_or_else(Vec::new, |active| crate::phasing::in_events(state, active));
            events.splice(0..0, leaving);
            let coming: Vec<_> = events
                .iter()
                .filter_map(|event| match event {
                    Event::PermanentPhasedIn { object } => Some(*object),
                    _ => None,
                })
                .collect();
            events.extend(
                state
                    .zone(Zone::Battlefield)
                    .iter()
                    .chain(&coming)
                    .filter(|id| {
                        active.is_some()
                            && !going.contains(*id)
                            && controller_of(state, **id) == active
                            && !crate::characteristics::doesnt_untap(state, **id)
                            && state
                                .object(**id)
                                .and_then(GameObject::permanent)
                                .is_some_and(PermanentState::tapped)
                    })
                    .map(|id| Event::PermanentUntapped { object: *id }),
            );
            if let Some(active) = active {
                events.extend(crate::c7_untap::c7_untap_spent(state, active));
            }
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
        Step::End => {
            let mut events = crate::monarch::end_step_draw(state);
            events.push(grant(state));
            Progress::Events(events)
        }
        // The Sagas of the active player get a lore counter as the precombat main phase begins (CR 714.3b), and
        // the rad counters of the active player mill (CR 725.2).
        Step::Main1 => {
            let mut events = crate::saga::precombat_counters(state);
            events.extend(crate::w7b_rad::w7b_precombat_rad(state));
            events.push(grant(state));
            Progress::Events(events)
        }
        Step::Upkeep | Step::BeginCombat | Step::EndCombat | Step::Main2 => {
            Progress::Events(vec![grant(state)])
        }
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
    events.extend(crate::monarch::taken_by_combat(state, &events));
    events.extend(crate::w15a_initiative::w15a_taken_by_combat(state, &events));
    events.push(grant(state));
    Progress::Events(events)
}

fn priority_round(state: &GameState) -> Progress {
    if all_passed(state) {
        // A mutating creature spell asks its controller whether it goes over or under the creature (CR 702.140b).
        if let Some(decision) = crate::merged::question(state) {
            return Progress::Decide(decision);
        }
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

/// The step after `step`: a skipped step does not begin at all (CR 614.10), so nothing triggers at its start.
fn next_step(state: &GameState, step: Step) -> Step {
    match step {
        Step::Upkeep
            if state
                .active_player()
                .is_some_and(|p| skips_draw_step(state, p)) =>
        {
            Step::Main1
        }
        Step::DeclareAttackers if state.combat().attackers().is_empty() => Step::EndCombat,
        Step::DeclareBlockers if !first_strike_step_needed(state) => Step::CombatDamage,
        other => other.next().unwrap_or(Step::Cleanup),
    }
}

/// Empties mana pools (CR 500.4), closes combat if leaving it, and starts the next step.
pub(crate) fn leave_step(state: &GameState) -> Vec<Event> {
    let step = state.turn().step();
    let natural = next_step(state, step);
    // Mana kept through the steps of a phase goes as the phase ends; a phase added after it (CR 500.8) comes next.
    let ends_phase = natural.phase() != step.phase();
    let (next, through) = match state.turn().phases_ahead().first() {
        Some(added) if ends_phase => (*added, ManaLasts::Combat),
        _ if ends_phase => (natural, ManaLasts::Combat),
        _ => (natural, ManaLasts::Step),
    };
    let mut events = pool_emptying(state, through);
    if step == Step::EndCombat {
        events.push(Event::CombatEnded);
    }
    events.push(Event::StepBegan { step: next });
    events
}

/// Empties every mana pool of the mana that lasts no longer than `through`; under the house rule of mana burn, a player
/// in the game loses 1 life for each mana that was left, restricted mana included.
fn pool_emptying(state: &GameState, through: ManaLasts) -> Vec<Event> {
    let burn = state.ruleset().mana_burn();
    state
        .players()
        .iter()
        .filter(|p| p.mana().emptying(through) > 0)
        .flat_map(|p| {
            let (player, amount) = (p.id(), p.mana().emptying(through));
            let through = p.mana().least_through(through);
            let mut events = vec![Event::ManaPoolEmptied { player, through }];
            if burn && p.is_in_game() && !crate::rules::life_total_cant_change(state, player) {
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
                    .is_some_and(|copy| copy.c10_xp_ends_at(crate::copies::C10XpCopyEnds::Cleanup))
            })
            .map(|o| Event::CopyEnded {
                object: o.id(),
                next_turn_of: None,
            }),
    );
    events.extend(pool_emptying(state, ManaLasts::Turn));
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
    let (passed, active, extra) = next_turn(state);
    events.extend(passed);
    events.push(Event::TurnBegan {
        number: state.turn().number() + 1,
        active,
        extra: extra.is_some(),
    });
    events.extend(crate::c10_xp_copy_ends::c10_xp_copies_ending(state, active));
    // The spells the ending turn's active player cast are counted before the new turn clears the count (CR 726.3a).
    if let Some(to) = crate::day_night::at_turn_start(state) {
        events.push(Event::DayNightChanged { to });
    }
    // A skipped untap step does not begin at all (CR 614.10).
    let first = if extra.is_some_and(|turn| turn.skip_untap) {
        Step::Upkeep
    } else {
        Step::Untap
    };
    events.push(Event::StepBegan { step: first });
    events
}

/// Who takes the next turn: the latest extra turn added whose player is still in the game and does not skip it (an
/// earlier one waits), else the next player in turn order after the seat of the regular turn (CR 500.7). The events
/// pass over the extra turns of players who have left or skip them.
fn next_turn(state: &GameState) -> (Vec<Event>, PlayerId, Option<ExtraTurn>) {
    let mut passed = Vec::new();
    for turn in state.extra_turns().iter().rev() {
        if state.is_alive(turn.player) && !crate::rules::skips_extra_turns(state, turn.player) {
            return (passed, turn.player, Some(*turn));
        }
        passed.push(Event::ExtraTurnSkipped {
            player: turn.player,
        });
    }
    let regular = state.turn().regular();
    (
        passed,
        state.next_alive_after(regular).unwrap_or(regular),
        None,
    )
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
