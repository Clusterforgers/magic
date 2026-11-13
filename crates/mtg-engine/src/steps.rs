//! Turn-based actions and step transitions (CR 500-514).

use mtg_cards::spend::ManaLasts;
use std::collections::BTreeSet;

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;

use crate::answers::Outcome;
use crate::characteristics::controller_of;
use crate::choose::{CardsChoice, CardsOptions, check_cards};
use crate::combat::{attack_options, block_options, defending_players};
use crate::damage::{DamagePass, damage_events, first_strike_step_needed, pending_assignment};
use crate::decision::{Decision, DecisionKind, DiscardOptions};
use crate::effects::ends_with_turn;
use crate::error::EngineError;
use crate::events::{Event, NothingToDo};
use crate::flow::Progress;
use crate::object::{GameObject, PermanentState};
use crate::player::Player;
use crate::priority::{all_passed, priority_options};
use crate::rules::{cant_draw, max_hand_size, no_max_hand_size, skips_draw_step, untap_during};
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
        Step::Untap => untap_step(state),
        Step::Draw => {
            // Teams that share turns count as one player each for the first draw (CR 810.5a).
            let sides = state
                .teams()
                .filter(|_| state.shares_turns())
                .map_or(state.players().len(), |teams| {
                    usize::try_from(teams.count()).unwrap_or(usize::MAX)
                });
            let skipped = state.turn().number() == 1 && state.ruleset().skips_first_draw(sides);
            let mut events = Vec::new();
            if active.is_some() && !skipped {
                // Each player whose turn it is draws (CR 805.3).
                for player in state.active_players() {
                    events.extend(draw(state, player, state.ruleset().draw_step_cards()));
                }
            }
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
        // the rad counters of the active player mill (CR 725.2), then they roll to visit their Attractions (CR 505.5).
        Step::Main1 => {
            let mut events = crate::saga::precombat_counters(state);
            events.extend(crate::w7b_rad::w7b_precombat_rad(state));
            events.extend(crate::un33_attractions::precombat(state));
            events.push(grant(state));
            Progress::Events(events)
        }
        Step::Upkeep | Step::BeginCombat | Step::EndCombat | Step::Main2 => {
            Progress::Events(vec![grant(state)])
        }
    }
}

/// The untap step: phasing, then the active player determines which permanents untap and they untap together (CR 502).
/// A permanent that may stay tapped is asked about whatever the hand: one question lists every such tapped permanent that
/// would untap, and the answer is the ones that stay tapped (CR 502.3); no such permanent, no question. While an untap
/// limit would hold back a permanent, the question lists those permanents too and the answer is the ones that untap
/// (`rmt4_untap_limit`).
fn untap_step(state: &GameState) -> Progress {
    let (events, asked) = untap_events(state, &[]);
    match (state.active_player(), asked.from.first()) {
        (Some(player), Some(&source)) => Progress::Decide(Decision {
            player,
            kind: DecisionKind::ChooseCards(CardsOptions {
                one_per_name: false,
                source,
                max: u32::try_from(asked.from.len()).unwrap_or(u32::MAX),
                from: asked.from,
                min: 0,
                text: if asked.untap {
                    "Choose the permanents to untap; the untap limits keep the first ones they allow.".to_owned()
                } else {
                    "Choose the permanents to keep tapped.".to_owned()
                },
                can_cancel: false,
                groups: Vec::new(),
            }),
        }),
        _ => Progress::Events(events),
    }
}

/// The question of the untap step: the permanents it lists, and whether the answer names those that untap (an untap
/// limit holds some back) rather than those kept tapped.
pub(crate) struct UntapQuestion {
    /// The permanents listed.
    pub(crate) from: Vec<ObjectId>,
    /// Whether the answer is the permanents that untap.
    pub(crate) untap: bool,
}

/// The events of the untap step with the permanents `kept` staying tapped, and the question the step asks.
pub(crate) fn untap_events(state: &GameState, kept: &[ObjectId]) -> (Vec<Event>, UntapQuestion) {
    let active = state.active_player();
    // Every player whose turn it is untaps (CR 805.3); the question of the step is the active player's.
    let actives = if active.is_some() {
        state.active_players()
    } else {
        Vec::new()
    };
    // Phasing comes first (CR 502.1): what phases in untaps with the rest.
    let leaving: Vec<Event> = actives
        .iter()
        .flat_map(|player| crate::phasing::w7e_keyword_out_events(state, *player))
        .collect();
    let going: BTreeSet<ObjectId> = leaving
        .iter()
        .filter_map(|event| match event {
            Event::PermanentPhasedOut { object, .. } => Some(*object),
            _ => None,
        })
        .collect();
    let mut events: Vec<Event> = actives
        .iter()
        .flat_map(|player| crate::phasing::in_events(state, *player))
        .collect();
    events.splice(0..0, leaving);
    let coming: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::PermanentPhasedIn { object } => Some(*object),
            _ => None,
        })
        .collect();
    let during: Vec<ObjectId> = actives
        .iter()
        .flat_map(|player| untap_during(state, *player))
        .collect();
    let untapping: Vec<ObjectId> = state
        .zone(Zone::Battlefield)
        .iter()
        .chain(&coming)
        .filter(|id| {
            active.is_some()
                && !going.contains(*id)
                && (controller_of(state, **id).is_some_and(|c| actives.contains(&c))
                    || during.contains(*id))
                && !crate::characteristics::doesnt_untap(state, **id)
                && state
                    .object(**id)
                    .and_then(GameObject::permanent)
                    .is_some_and(PermanentState::tapped)
        })
        .copied()
        .collect();
    let optional: Vec<ObjectId> = untapping
        .iter()
        .copied()
        .filter(|id| {
            controller_of(state, *id) == active && crate::combat_quirks::may_stay_tapped(state, *id)
        })
        .collect();
    events.extend(
        untapping
            .iter()
            .filter(|id| !kept.contains(id))
            .filter_map(|id| state.object(*id).map(crate::stun::untap_event)),
    );
    for player in &actives {
        events.extend(crate::c7_untap::c7_untap_spent(state, *player));
    }
    events.extend(leave_step(state));
    let held = active.map_or_else(Vec::new, |active| {
        crate::rmt4_untap_limit::held_back(state, active, &untapping)
    });
    let question = if held.is_empty() {
        UntapQuestion {
            from: optional,
            untap: false,
        }
    } else {
        let mut from = optional;
        for id in held {
            if !from.contains(&id) {
                from.push(id);
            }
        }
        UntapQuestion { from, untap: true }
    };
    (events, question)
}

/// The answer to the question of the untap step: the permanents chosen stay tapped and the others untap.
pub(crate) fn untap_answer(
    state: &GameState,
    options: &CardsOptions,
    choice: &CardsChoice,
) -> Result<Outcome, EngineError> {
    check_cards(options, &choice.0)?;
    let asked = untap_events(state, &[]).1;
    let kept = match state.active_player() {
        Some(active) if asked.untap => {
            let untapping = crate::rmt4_untap_limit::allowed(state, active, &choice.0);
            asked
                .from
                .into_iter()
                .filter(|id| !untapping.contains(id))
                .collect()
        }
        _ => choice.0.clone(),
    };
    Ok(Outcome::Events(untap_events(state, &kept).0))
}

/// Whether a choice of cards outside any resolution is the question of the untap step.
pub(crate) fn asks_untap(state: &GameState) -> bool {
    state.turn().step() == Step::Untap
}

/// Events for `player` drawing `count` cards, or failing to on an empty library. A player who can't draw cards neither
/// draws nor fails to.
pub(crate) fn draw(state: &GameState, player: PlayerId, count: usize) -> Vec<Event> {
    if cant_draw(state, player) {
        return Vec::new();
    }
    // "Can't draw more than one card each turn": the draws past the limit don't happen.
    let count = crate::rules::ep24_draws_left(state, player).map_or(count, |left| count.min(left));
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
    let mut next = following_step(state, step);
    while state
        .active_player()
        .is_some_and(|p| skipped_by(state, p, next))
    {
        next = following_step(state, next);
    }
    next
}

/// Whether the turn of `player` skips `step` (CR 614.10): the draw step by `SkipDrawStep`, any step by a prohibition.
fn skipped_by(state: &GameState, player: PlayerId, step: Step) -> bool {
    (step == Step::Draw && skips_draw_step(state, player))
        || crate::prohibit::skips_step(state, player, step)
}

/// The steps the active player's turn skips right after `step`, in order.
fn skipped_after(state: &GameState, step: Step) -> Vec<Step> {
    let mut skipped = Vec::new();
    let Some(player) = state.active_player() else {
        return skipped;
    };
    let mut next = following_step(state, step);
    while next != Step::Cleanup && skipped_by(state, player, next) {
        skipped.push(next);
        next = following_step(state, next);
    }
    skipped
}

/// The step after `step` before any step is skipped.
fn following_step(state: &GameState, step: Step) -> Step {
    match step {
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
    let (next, through) = match (
        state.turn().steps_ahead().first(),
        state.turn().phases_ahead().first(),
    ) {
        // A step added after this one comes first; its phase goes on (an additional upkeep step).
        (Some(step_added), _) => (*step_added, ManaLasts::Step),
        (None, Some(added)) if ends_phase => (*added, ManaLasts::Combat),
        _ if ends_phase => (natural, ManaLasts::Combat),
        _ => (natural, ManaLasts::Step),
    };
    let mut events = pool_emptying(state, through);
    if step == Step::EndCombat {
        events.push(Event::CombatEnded);
    }
    if next == natural
        && let Some(player) = state.active_player()
    {
        events.extend(crate::rmt3_skips::rmt3_used(
            state,
            player,
            &skipped_after(state, step),
            true,
        ));
    }
    events.push(Event::StepBegan { step: next });
    events
}

/// Empties every mana pool of the mana that lasts no longer than `through`; under the house rule or a rule of mana
/// burn, a player in the game loses 1 life for each mana that was left, restricted mana included.
fn pool_emptying(state: &GameState, through: ManaLasts) -> Vec<Event> {
    let burn = state.ruleset().mana_burn() || state.rules().mana_burn;
    state
        .players()
        .iter()
        .filter(|p| p.mana().emptying(through) > 0)
        .flat_map(|p| {
            let player = p.id();
            let (keeps, becomes) = crate::rules::ep24_unspent_mana(state, player);
            if keeps.contains(&None) {
                return Vec::new();
            }
            let kept: Vec<_> = ManaType::ALL
                .into_iter()
                .filter(|mana| keeps.contains(&Some(*mana)))
                .collect();
            let through = p.mana().least_through(through);
            let mut after = p.mana().clone();
            after.empty_keeping(through, &kept, becomes);
            if after == *p.mana() {
                return Vec::new();
            }
            let amount = p.mana().total().saturating_sub(after.total());
            let mut events = vec![Event::ManaPoolEmptied {
                player,
                through,
                kept,
                becomes,
            }];
            if amount == 0 {
                return events;
            }
            if burn && p.is_in_game() && !crate::rules::life_total_cant_change(state, player) {
                events.push(Event::ManaBurned { player, amount });
                events.push(Event::LifeLost { player, amount });
            }
            events
        })
        .collect()
}

fn cleanup(state: &GameState) -> Progress {
    // Each player whose turn it is discards to their maximum hand size (CR 514.1, CR 805.3).
    let actives = if state.active_player().is_some() {
        state.active_players()
    } else {
        Vec::new()
    };
    for active in actives {
        let hand = state.zone(Zone::Hand(active));
        let most = state
            .ruleset()
            .max_hand_size()
            .filter(|_| !no_max_hand_size(state, active))
            .map(|base| max_hand_size(state, active, base));
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
            .filter(|e| ends_with_turn(state, &e.duration, e.turn, e.controller))
            .map(|e| Event::EffectExpired { id: e.id }),
    );
    events.extend(
        state
            .rule_effects()
            .iter()
            .filter(|e| ends_with_turn(state, &e.duration, e.turn, e.controller))
            .map(|e| Event::RuleExpired { id: e.id }),
    );
    events.extend(crate::r377_skips::r377_their_turn_over(state));
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
                attached_by: None,
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
    let (passed, spent, active, extra) = next_turn(state);
    events.extend(passed);
    events.push(Event::TurnBegan {
        number: state.turn().number() + 1,
        active,
        extra: extra.is_some(),
    });
    events.extend(crate::schemes::at_turn_start(state, active));
    events.extend(spent);
    events.extend(crate::c10_xp_copy_ends::c10_xp_copies_ending(state, active));
    // The spells the ending turn's active player cast are counted before the new turn clears the count (CR 726.3a).
    if let Some(to) = crate::day_night::at_turn_start(state) {
        events.push(Event::DayNightChanged { to });
    }
    // A skipped untap step does not begin at all (CR 614.10).
    let mut first = if extra.is_some_and(|turn| turn.skip_untap) {
        Step::Upkeep
    } else {
        Step::Untap
    };
    let mut skipped = Vec::new();
    while skipped_by(state, active, first) {
        skipped.push(first);
        first = first.next().unwrap_or(Step::Cleanup);
    }
    events.extend(crate::rmt3_skips::rmt3_used(state, active, &skipped, false));
    events.push(Event::StepBegan { step: first });
    events
}

/// The turn that follows: the events that pass over extra turns, the events that end the rules used up by skipped turns, the
/// player and the extra turn taken.
type NextTurn = (Vec<Event>, Vec<Event>, PlayerId, Option<ExtraTurn>);

/// Who takes the next turn: the latest extra turn added whose player is still in the game and does not skip it (an
/// earlier one waits), else the next player in turn order after the seat of the regular turn (CR 500.7). The events
/// pass over the extra turns of players who have left or skip them.
fn next_turn(state: &GameState) -> NextTurn {
    let mut passed = Vec::new();
    let mut spent = Vec::new();
    for turn in state.extra_turns().iter().rev() {
        if state.is_alive(turn.player)
            && !crate::rules::skips_extra_turns(state, turn.player)
            && !crate::prohibit::skips_turn(state, turn.player)
        {
            return (passed, spent, turn.player, Some(*turn));
        }
        passed.push(Event::ExtraTurnSkipped {
            player: turn.player,
        });
        if crate::prohibit::skips_turn(state, turn.player) {
            use_skips(&mut spent, state, turn.player);
        }
    }
    let regular = state.turn().regular();
    let first = state.next_turn_seat(regular).unwrap_or(regular);
    // A skipped turn is not taken; when every player skips, the first takes theirs.
    let mut next = first;
    for _ in 0..state.players().len() {
        if !crate::prohibit::skips_turn(state, next) {
            return (passed, spent, next, None);
        }
        use_skips(&mut spent, state, next);
        next = state.next_turn_seat(next).unwrap_or(next);
    }
    (passed, spent, first, None)
}

/// Adds the events that end the rule a skipped turn of `player` uses up ("skip your next turn"), after the turn that follows
/// has begun. A player passed over twice in one search (the seats of players who left are counted) ends it once.
fn use_skips(spent: &mut Vec<Event>, state: &GameState, player: PlayerId) {
    for id in crate::prohibit::skipped_turn_rules(state, player) {
        let event = Event::RuleExpired { id };
        if !spent.contains(&event) {
            spent.push(event);
        }
    }
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
        (Some(only), None) if state.teams().is_some() => GameResult::Team(state.team_of(only.id())),
        (Some(only), None) => GameResult::Winner(only.id()),
        _ => GameResult::Draw,
    }
}
