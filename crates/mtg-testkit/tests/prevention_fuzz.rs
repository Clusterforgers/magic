//! Goal: whole bot games with decks of prevention and regeneration (Fog, Mending Hands, Samite Healer, Drudge Skeletons, Death Ward) against burn, destroy effects, Wrath of God and creatures that fight finish and keep every invariant for many seeds at tables of two, three and four seats, and the games use them: an independent ledger replays each log and finds that no combat damage is dealt while a Fog is in force unless it can't be prevented, that damage of a spell is dealt through a Fog, that a shield with damage left says how much it kept and that this is always less than before and more than nothing, that a creature regenerates only while a shield names it, and that Wrath of God resolves with no regeneration and leaves no creature.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::ObjectId;
use mtg_engine::events::{ResolutionOutcome, ZoneCause};
use mtg_engine::rules::Rule;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameSetup, GameState, apply_event, replay};
use mtg_testkit::{play_game, prevent_fuzz_setup};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40)
}

/// What a campaign saw the shields do.
#[derive(Default)]
struct Seen {
    fogs: u64,
    attacks_in_a_fog: u64,
    combat_damage_in_a_fog: u64,
    spell_damage_in_a_fog: u64,
    shields_used: u64,
    shields_ended: u64,
    regenerated: u64,
    wraths: u64,
    buried: u64,
}

fn name_of(state: &GameState, id: ObjectId) -> String {
    state
        .object(id)
        .map_or_else(|| "?".to_owned(), |o| o.card().name.clone())
}

/// Whether a Fog is in force: a shield that prevents all combat damage to anything from any source.
fn fog_in_force(state: &GameState) -> bool {
    state.rule_effects().iter().any(|effect| {
        matches!(
            &effect.rule,
            Rule::PreventDamage(shield)
                if shield.to.is_none()
                    && shield.left.is_none()
                    && shield.combat == Some(true)
                    && shield.source.is_empty()
        )
    })
}

/// Whether any rule makes some damage unpreventable.
fn unpreventable_in_force(state: &GameState) -> bool {
    state
        .rule_effects()
        .iter()
        .any(|effect| matches!(effect.rule, Rule::DamageCantBePrevented { .. }))
}

/// Whether a regeneration shield names this object.
fn regenerates(state: &GameState, object: ObjectId) -> bool {
    state
        .rule_effects()
        .iter()
        .any(|effect| matches!(&effect.rule, Rule::Regenerate(named) if named.id == object))
}

/// Replays the log event by event and checks each against the state it happened in.
fn ledger(setup: &GameSetup, events: &[Event], seen: &mut Seen) {
    let mut state = GameState::from_setup(setup).unwrap();
    let mut wrath: Option<ObjectId> = None;
    let mut wrath_ended = false;
    for (at, event) in events.iter().enumerate() {
        match event {
            Event::RuleCreated { effect }
                if matches!(&effect.rule, Rule::PreventDamage(shield)
                    if shield.combat == Some(true) && shield.to.is_none()) =>
            {
                seen.fogs += 1;
            }
            Event::AttackersDeclared { attackers, .. }
                if fog_in_force(&state) && !attackers.is_empty() =>
            {
                seen.attacks_in_a_fog += 1;
            }
            Event::DamageDealt { .. } if fog_in_force(&state) => {
                // Combat damage is dealt in a damage step with nothing resolving; the rest is a spell's or an ability's.
                let combat = state.resolving().is_none()
                    && matches!(
                        state.turn().step(),
                        Step::FirstStrikeDamage | Step::CombatDamage
                    );
                if combat {
                    assert!(
                        unpreventable_in_force(&state),
                        "combat damage at {at} with a Fog in force"
                    );
                    seen.combat_damage_in_a_fog += 1;
                } else {
                    seen.spell_damage_in_a_fog += 1;
                }
            }
            Event::ShieldUsed { id, left } => {
                let shield = state
                    .rule_effects()
                    .iter()
                    .find(|effect| effect.id == *id)
                    .and_then(|effect| match &effect.rule {
                        Rule::PreventDamage(shield) => Some(shield.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| panic!("ShieldUsed at {at} names no damage shield"));
                let before = shield.left.expect("only a shield with an amount is used");
                assert!(
                    *left > 0 && *left < before,
                    "ShieldUsed at {at}: {before} left became {left}"
                );
                seen.shields_used += 1;
            }
            Event::RuleExpired { id } => {
                if state
                    .rule_effects()
                    .iter()
                    .any(|effect| effect.id == *id && matches!(effect.rule, Rule::PreventDamage(_)))
                {
                    seen.shields_ended += 1;
                }
            }
            Event::Regenerated { object } => {
                assert!(
                    regenerates(&state, *object),
                    "{} regenerated at {at} with no shield",
                    name_of(&state, *object)
                );
                assert!(
                    wrath.is_none(),
                    "a regeneration inside Wrath of God at {at}"
                );
                seen.regenerated += 1;
            }
            Event::ResolutionBegan { object }
                if state
                    .object(*object)
                    .is_some_and(|spell| spell.card().name == "Wrath of God") =>
            {
                wrath = Some(*object);
            }
            Event::ResolutionEnded { object, outcome } if wrath == Some(*object) => {
                wrath = None;
                wrath_ended = *outcome == ResolutionOutcome::Resolved;
            }
            Event::ZoneChanged {
                cause: ZoneCause::Buried,
                from: Zone::Battlefield,
                ..
            } => seen.buried += 1,
            _ => {}
        }
        apply_event(&mut state, event).unwrap();
        if std::mem::take(&mut wrath_ended) {
            // Every creature is destroyed and none can regenerate: not one is left.
            let left: Vec<String> = state
                .zone(Zone::Battlefield)
                .iter()
                .filter_map(|id| state.object(*id))
                .filter(|o| o.card().power.is_some())
                .map(|o| o.card().name.clone())
                .collect();
            assert!(
                left.is_empty(),
                "creatures left after Wrath of God at {at}: {left:?}"
            );
            seen.wraths += 1;
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_prevention_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = prevent_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        assert_eq!(end.state_hash(), outcome.final_hash);
        ledger(&setup, &outcome.events, &mut seen);
    }
    let campaign = format!("{} games of {players} seats", games());
    assert!(seen.fogs > 0, "no Fog was cast in {campaign}");
    assert!(seen.shields_used > 0, "no shield kept damage in {campaign}");
    assert!(seen.shields_ended > 0, "no shield ended in {campaign}");
    assert!(
        seen.attacks_in_a_fog > 0,
        "no creature attacked in a Fog in {campaign}"
    );
    assert!(seen.regenerated > 0, "nothing regenerated in {campaign}");
    assert!(seen.wraths > 0, "no Wrath of God resolved in {campaign}");
    assert!(seen.buried > 0, "nothing was buried in {campaign}");
    assert!(
        seen.spell_damage_in_a_fog > 0,
        "no spell dealt damage in a Fog in {campaign}"
    );
    assert_eq!(
        seen.combat_damage_in_a_fog, 0,
        "combat damage was dealt in a Fog in {campaign}"
    );
}

#[test]
fn a_seeded_prevention_game_is_the_same_game_every_time() {
    let setup = prevent_fuzz_setup(3, 11).unwrap();
    let first = play_game(setup.clone(), 11).unwrap();
    let second = play_game(setup, 11).unwrap();
    assert_eq!(first.final_hash, second.final_hash);
    assert_eq!(first.events, second.events);
}
