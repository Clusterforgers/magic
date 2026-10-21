//! Goal: whole bot games with decks of planeswalkers (Jace, the Mind Sculptor, Jace Beleren, Ajani, Garruk and two more that burn, destroy and make tokens), the creatures that attack them and the burn that kills them finish and keep every invariant (zones, life, the journal, the replay of the log equal to the live state, conservation of cards, the loyalty counters of every planeswalker equal to those it entered with and the cost of its abilities put on less those taken off and the damage dealt to it, never fewer than none, no planeswalker with no loyalty at a decision, a loyalty ability only at sorcery timing and once for each planeswalker each turn) for many seeds at tables of two, three and four seats, and the games really use them: every planeswalker enters with its printed loyalty in the batch that brings it in, costs of each sign are paid, damage from combat and spells removes loyalty, planeswalkers die of damage and of their own costs, creatures attack planeswalkers and go on attacking when one leaves, an engine rebuilt from the log at each decision asks what the original asks, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::combat::AttackTarget;
use mtg_engine::events::ZoneCause;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event, GameState, replay};
use mtg_testkit::loyalty_cards::{LOYALTY_CORPUS_CARDS, LOYALTY_SYNTHETIC_CARDS};
use mtg_testkit::{RandomBot, loyalty_fuzz_setup, play_game};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40)
}

/// What a campaign saw the planeswalkers do.
#[derive(Default)]
struct Seen {
    entered: BTreeMap<String, u64>,
    with_printed_loyalty: u64,
    without_it: u64,
    plus: u64,
    minus: u64,
    zero: u64,
    damage_to_walkers: u64,
    died_of_damage_or_cost: u64,
    attacks_at_walkers: u64,
}

impl Seen {
    fn is_walker(game: &GameState, id: mtg_core::ids::ObjectId) -> bool {
        game.object(id)
            .is_some_and(|o| o.base_types().contains(CardType::Planeswalker))
    }

    fn add(&mut self, game: &GameState, events: &[Event]) {
        for (n, event) in events.iter().enumerate() {
            match event {
                Event::SpellResolved { object }
                | Event::ZoneChanged {
                    object,
                    to: Zone::Battlefield,
                    ..
                } => {
                    if let Some(entered) = game.object(*object)
                        && entered.base_types().contains(CardType::Planeswalker)
                    {
                        *self.entered.entry(entered.card().name.clone()).or_default() += 1;
                        // The loyalty comes in the same batch: the next event, or the one after an `Entered`.
                        let printed = i64::from(entered.card().loyalty.unwrap());
                        let brought = events[n + 1..].iter().take(2).any(|next| {
                            matches!(next, Event::CountersChanged(changed)
                                if changed.on.object().is_some_and(|named| named.id == *object)
                                    && changed.counter == CounterKind::Loyalty
                                    && i64::from(changed.delta) == printed)
                        });
                        if brought {
                            self.with_printed_loyalty += 1;
                        } else {
                            self.without_it += 1;
                        }
                    }
                }
                Event::AbilityActivated { source, .. } if Self::is_walker(game, source.id) => {
                    let cost = n
                        .checked_sub(1)
                        .and_then(|before| events.get(before))
                        .and_then(|before| match before {
                            Event::CountersChanged(changed)
                                if changed.on.object().is_some_and(|named| named.id == source.id)
                                    && changed.counter == CounterKind::Loyalty =>
                            {
                                Some(changed.delta)
                            }
                            _ => None,
                        });
                    match cost {
                        Some(delta) if delta > 0 => self.plus += 1,
                        Some(_) => self.minus += 1,
                        None => self.zero += 1,
                    }
                }
                Event::DamageDealt {
                    target: mtg_engine::combat::DamageTarget::Object(object),
                    ..
                } if Self::is_walker(game, *object) => self.damage_to_walkers += 1,
                Event::ZoneChanged {
                    object,
                    cause: ZoneCause::StateBased,
                    ..
                } if Self::is_walker(game, *object) => self.died_of_damage_or_cost += 1,
                Event::AttackersDeclared { attackers, .. } => {
                    self.attacks_at_walkers += attackers
                        .iter()
                        .filter(|(_, target)| matches!(target, AttackTarget::Planeswalker(_)))
                        .count() as u64;
                }
                _ => {}
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_planeswalker_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = loyalty_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        seen.add(&end, &outcome.events);
    }
    let campaign = format!("{} games of {players} seats", games());
    for name in LOYALTY_CORPUS_CARDS
        .iter()
        .chain(&LOYALTY_SYNTHETIC_CARDS[..2])
    {
        assert!(
            seen.entered.get(*name).copied().unwrap_or(0) > 0,
            "{name} never entered play in {campaign}"
        );
    }
    assert_eq!(seen.without_it, 0, "a planeswalker came in without its loyalty in {campaign}");
    assert!(seen.with_printed_loyalty > 0, "no planeswalker entered in {campaign}");
    assert!(seen.plus > 0, "no + cost paid in {campaign}");
    assert!(seen.minus > 0, "no - cost paid in {campaign}");
    assert!(seen.zero > 0, "no 0 cost paid in {campaign}");
    assert!(seen.attacks_at_walkers > 0, "no attack at a planeswalker in {campaign}");
    assert!(seen.damage_to_walkers > 0, "no damage to a planeswalker in {campaign}");
    assert!(seen.died_of_damage_or_cost > 0, "no planeswalker died in {campaign}");
}

/// What a walk through the decisions of a game saw.
#[derive(Default)]
struct Walk {
    walkers_seen: u64,
    attackers_at_nothing: u64,
    attackers_checked: u64,
}

impl Walk {
    /// Judges the state at a decision: every planeswalker holds at least one loyalty counter, no creature is attacking
    /// a planeswalker that is not on the battlefield unless it is marked as attacking nothing, and an attacker aimed at
    /// nothing has no one to block it.
    fn look(&mut self, state: &GameState) {
        for &id in state.zone(Zone::Battlefield) {
            let object = state.object(id).unwrap();
            if object.base_types().contains(CardType::Planeswalker) && state.resolving().is_none() {
                self.walkers_seen += 1;
                assert!(
                    object.permanent().unwrap().counters_of(&CounterKind::Loyalty) >= 1,
                    "{} has no loyalty at a decision",
                    object.card().name
                );
            }
        }
        for attacker in state.combat().attackers() {
            self.attackers_checked += 1;
            if attacker.attacks_nothing() {
                self.attackers_at_nothing += 1;
                assert!(attacker.defender(state).is_none());
            } else if let AttackTarget::Planeswalker(id) = attacker.target() {
                assert!(
                    state
                        .object(id)
                        .is_some_and(|o| o.zone() == Zone::Battlefield),
                    "an attacker aimed at {id:?} which is not in play"
                );
            }
        }
    }
}

/// Walks the games of `seeds` decision by decision with one bot a seat, judging each decision; with `rebuild` it also
/// resumes an engine from the log at each decision and fails when that engine differs from the live one.
fn walk_games(
    players: usize,
    seeds: std::ops::Range<u64>,
    rebuild: bool,
) -> Result<Walk, EngineError> {
    let mut walk = Walk::default();
    for seed in seeds {
        let setup = loyalty_fuzz_setup(players, seed).unwrap();
        let mut engine = Engine::new(setup.clone())?;
        let mut bots: Vec<RandomBot> = (0..players as u64)
            .map(|seat| RandomBot::new(seed, seat))
            .collect();
        for step in 0..4000 {
            let Some(decision) = engine.decision().cloned() else {
                break;
            };
            if rebuild && step < 1500 {
                let resumed = Engine::resume(setup.clone(), engine.events().to_vec())?;
                assert_eq!(
                    resumed.state_hash(),
                    engine.state_hash(),
                    "seed {seed} step {step}"
                );
                let mid_cast = matches!(
                    &decision.kind,
                    mtg_engine::DecisionKind::PayMana(_)
                        | mtg_engine::DecisionKind::ChooseTargets(_)
                ) && !matches!(&decision.kind, mtg_engine::DecisionKind::ChooseTargets(o) if o.triggered);
                if !mid_cast {
                    assert_eq!(
                        resumed.decision(),
                        engine.decision(),
                        "seed {seed} step {step}"
                    );
                }
            }
            walk.look(engine.state());
            let answer = bots[decision.player.index()].choose(&decision);
            engine.apply(answer)?;
        }
    }
    Ok(walk)
}

#[rstest]
fn a_rebuilt_engine_asks_what_the_original_asks_under_planeswalkers(
    #[values(2, 3)] players: usize,
) -> Result<(), EngineError> {
    let walk = walk_games(players, 0..8, true)?;
    assert!(walk.walkers_seen > 0, "no planeswalker in play");
    Ok(())
}

#[rstest]
fn every_planeswalker_has_loyalty_and_every_attacker_has_a_target_or_none_at_every_decision(
    #[values(2, 3, 4)] players: usize,
) -> Result<(), EngineError> {
    let walk = walk_games(players, 0..games(), false)?;
    let seats = format!("{players} seats");
    assert!(walk.walkers_seen > 0, "no planeswalker at {seats}");
    assert!(walk.attackers_checked > 0, "no attacker at {seats}");
    Ok(())
}

#[test]
fn creatures_go_on_attacking_a_planeswalker_that_left() {
    let mut at_nothing = 0;
    for players in [2, 3, 4] {
        at_nothing += walk_games(players, 0..games() * 2, false)
            .unwrap()
            .attackers_at_nothing;
    }
    assert!(
        at_nothing > 0,
        "no attacker was left attacking a planeswalker that had gone in {} games of each size",
        games() * 2
    );
}

#[test]
fn a_seeded_planeswalker_game_is_the_same_game_every_time() {
    for seed in [3, 11] {
        let setup = loyalty_fuzz_setup(3, seed).unwrap();
        let first = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let second = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        assert_eq!(first.final_hash, second.final_hash, "seed {seed}");
        assert_eq!(first.events, second.events, "seed {seed}");
    }
}
