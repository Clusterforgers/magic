//! Goal: whole bot games with decks of abilities that work from a zone other than the battlefield (a graveyard, a hand, the stack) and of state triggers finish and keep every invariant (zones, life, the journal, the replay of the log equal to the live state, conservation of cards) for many seeds at tables of two, three and four seats, and the games really use them: abilities are activated from a graveyard and a hand, abilities trigger from a graveyard, a hand and the stack, state triggers trigger, a static ability applies from a graveyard; no seat is ever offered an ability of a card it cannot identify or of a card whose ability does not work from the zone it is in; a state trigger never has two instances at once; an engine rebuilt from the log at each decision asks what the original asks; and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Mutex;

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::ZoneKind;
use mtg_cards::trigger::TriggerPattern;
use mtg_engine::ability_reader::ability;
use mtg_engine::decision::DecisionKind;
use mtg_engine::keywords::has_keyword;
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event, GameState, replay};
use mtg_testkit::zone_cards::{ZONE_CORPUS_CARDS, ZONE_SYNTHETIC_CARDS};
use mtg_testkit::{RandomBot, play_game, zones_fuzz_setup};
use rstest::rstest;

use crate::common::seeds::{for_each_seed, fuzz_games};

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(25)
}

/// The kind of a zone, as the data names it.
const fn kind_of(zone: Zone) -> ZoneKind {
    match zone {
        Zone::Library(_) => ZoneKind::Library,
        Zone::Hand(_) => ZoneKind::Hand,
        Zone::Battlefield => ZoneKind::Battlefield,
        Zone::Graveyard(_) => ZoneKind::Graveyard,
        Zone::Stack => ZoneKind::Stack,
        Zone::Exile => ZoneKind::Exile,
        Zone::Command(_) => ZoneKind::Command,
    }
}

/// What a campaign saw the abilities do.
#[derive(Default)]
struct Seen {
    entered: BTreeMap<String, u64>,
    activated_from_graveyard: u64,
    activated_from_hand: u64,
    triggered_from_graveyard: u64,
    triggered_from_hand: u64,
    triggered_from_the_stack: u64,
    state_triggers: u64,
}

impl Seen {
    fn add(&mut self, game: &GameState, events: &[Event]) {
        for event in events {
            match event {
                Event::ZoneChanged {
                    object,
                    to: Zone::Battlefield,
                    ..
                }
                | Event::SpellResolved { object } => {
                    if let Some(object) = game.object(*object) {
                        *self.entered.entry(object.card().name.clone()).or_default() += 1;
                    }
                }
                Event::AbilityPutOnStack(put) if put.cause.is_none() => match put.snapshot.zone {
                    Zone::Graveyard(_) => self.activated_from_graveyard += 1,
                    Zone::Hand(_) => self.activated_from_hand += 1,
                    _ => {}
                },
                Event::AbilityTriggered(trigger) => {
                    match trigger.snapshot.zone {
                        Zone::Graveyard(_) => self.triggered_from_graveyard += 1,
                        Zone::Hand(_) => self.triggered_from_hand += 1,
                        Zone::Stack => self.triggered_from_the_stack += 1,
                        _ => {}
                    }
                    // A token that triggered has ceased to exist by the end of the game, and none has a state trigger.
                    let named = |name: &str| {
                        game.object(trigger.source.id)
                            .is_some_and(|source| source.card().name == name)
                    };
                    if trigger.cause.subjects.is_empty()
                        && ["Covetous Dragon", "Hollow Scholar", "Lone Wolf"]
                            .iter()
                            .any(|name| named(name))
                    {
                        self.state_triggers += 1;
                    }
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
fn seeded_zone_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(Seen::default());
    for_each_seed(0..games(), |seed| {
        let setup = zones_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        seen.lock().unwrap().add(&end, &outcome.events);
    });
    let seen = seen.into_inner().unwrap();
    let campaign = format!("{} games of {players} seats", games());
    // The Masterpiece is a sorcery for seven mana that the games seldom reach; its discard is counted below.
    for name in ZONE_CORPUS_CARDS
        .iter()
        .chain(ZONE_SYNTHETIC_CARDS)
        .filter(|name| **name != "Elemental Masterpiece")
    {
        assert!(
            seen.entered.get(*name).copied().unwrap_or(0) > 0,
            "{name} never entered play in {campaign}"
        );
    }
    assert!(
        seen.activated_from_graveyard > 0,
        "no graveyard ability in {campaign}"
    );
    assert!(
        seen.activated_from_hand > 0,
        "no hand ability in {campaign}"
    );
    assert!(
        seen.triggered_from_graveyard > 0,
        "no graveyard trigger in {campaign}"
    );
    assert!(
        seen.triggered_from_hand > 0,
        "no hand trigger in {campaign}"
    );
    assert!(
        seen.triggered_from_the_stack > 0,
        "no spell trigger in {campaign}"
    );
    assert!(seen.state_triggers > 0, "no state trigger in {campaign}");
}

/// What a walk through the decisions of a game saw.
#[derive(Default)]
struct Walk {
    offered_from_the_graveyard: u64,
    offered_from_the_hand: u64,
    offered_from_the_battlefield: u64,
    graveyard_hastes: u64,
    state_instances: u64,
}

impl Walk {
    /// Judges a decision of `state`: every ability a seat is offered is its own and works where its card is, no state
    /// trigger has two instances, and the creatures of a seat with Anger in its graveyard and a Mountain have haste.
    fn look(&mut self, state: &GameState, decision: &mtg_engine::Decision) {
        if let DecisionKind::Priority(options) = &decision.kind {
            for offered in &options.abilities {
                let object = state.object(offered.object).unwrap();
                match object.zone() {
                    Zone::Battlefield => self.offered_from_the_battlefield += 1,
                    Zone::Graveyard(owner) => {
                        assert_eq!(owner, decision.player, "another seat's graveyard card");
                        self.offered_from_the_graveyard += 1;
                    }
                    Zone::Hand(owner) => {
                        assert_eq!(owner, decision.player, "another seat's hand card");
                        self.offered_from_the_hand += 1;
                    }
                    other => panic!("an ability is offered from {other:?}"),
                }
                assert_eq!(object.owner(), decision.player);
                let rules = ability(state, *offered).unwrap();
                let Ability::Activated { from, .. } = &*rules else {
                    panic!("{offered:?} is not an activated ability");
                };
                assert_eq!(*from, kind_of(object.zone()), "{}", object.card().name);
            }
        }
        // A state trigger has one instance at a time: waiting in the queue or on the stack.
        let mut instances: BTreeMap<_, u32> = BTreeMap::new();
        let is_state = |rules: &Ability| {
            matches!(
                rules,
                Ability::Triggered {
                    when: TriggerPattern::State { .. },
                    ..
                }
            )
        };
        for waiting in state.pending_triggers() {
            if is_state(waiting.rules()) {
                let t = waiting.trigger();
                *instances
                    .entry((t.source, t.origin, t.ability))
                    .or_default() += 1;
            }
        }
        for id in state.stack() {
            let object = state.object(*id).unwrap();
            if object.kind() == ObjectKind::Ability
                && let Some(ability) = object.ability()
                && is_state(ability.rules())
            {
                *instances
                    .entry((ability.source(), ability.origin(), ability.index()))
                    .or_default() += 1;
            }
        }
        for (key, count) in &instances {
            assert_eq!(*count, 1, "{key:?} has {count} instances at once");
        }
        self.state_instances += instances.len() as u64;
        for seat in state.alive_players() {
            let anger = state
                .zone(Zone::Graveyard(seat))
                .iter()
                .any(|id| state.object(*id).is_some_and(|o| o.card().name == "Anger"));
            let mountain = state.zone(Zone::Battlefield).iter().any(|id| {
                state.object(*id).is_some_and(|o| {
                    o.controller() == seat && o.card().subtypes.iter().any(|s| s.0 == "Mountain")
                })
            });
            if anger && mountain {
                for id in state.zone(Zone::Battlefield) {
                    let object = state.object(*id).unwrap();
                    if object.controller() == seat
                        && object
                            .base_types()
                            .contains(mtg_core::card::CardType::Creature)
                    {
                        assert!(
                            has_keyword(state, *id, &Keyword::Haste),
                            "{}",
                            object.card().name
                        );
                        self.graveyard_hastes += 1;
                    }
                }
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
        let setup = zones_fuzz_setup(players, seed).unwrap();
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
                    DecisionKind::PayMana(_) | DecisionKind::ChooseTargets(_)
                ) && !matches!(&decision.kind, DecisionKind::ChooseTargets(o) if o.triggered);
                if !mid_cast {
                    assert_eq!(
                        resumed.decision(),
                        engine.decision(),
                        "seed {seed} step {step}"
                    );
                }
            }
            walk.look(engine.state(), &decision);
            let answer = bots[decision.player.index()].choose(&decision);
            engine.apply(answer)?;
        }
    }
    Ok(walk)
}

#[rstest]
fn a_rebuilt_engine_asks_what_the_original_asks_under_abilities_from_other_zones(
    #[values(2, 3)] players: usize,
) -> Result<(), EngineError> {
    let walk = walk_games(players, 0..8, true)?;
    assert!(
        walk.offered_from_the_graveyard > 0,
        "nothing was offered from a graveyard"
    );
    Ok(())
}

#[rstest]
fn every_offered_ability_is_the_seats_own_works_where_its_card_is_and_no_state_trigger_doubles(
    #[values(2, 3, 4)] players: usize,
) -> Result<(), EngineError> {
    let walk = walk_games(players, 0..games(), false)?;
    let seats = format!("{players} seats");
    assert!(
        walk.offered_from_the_graveyard > 0,
        "no graveyard ability at {seats}"
    );
    assert!(walk.offered_from_the_hand > 0, "no hand ability at {seats}");
    assert!(
        walk.offered_from_the_battlefield > 0,
        "no battlefield ability at {seats}"
    );
    assert!(
        walk.graveyard_hastes > 0,
        "no Anger gave haste from a graveyard at {seats}"
    );
    assert!(
        walk.state_instances > 0,
        "no state trigger was alive at {seats}"
    );
    Ok(())
}

#[test]
fn a_seeded_zone_game_is_the_same_game_every_time() {
    for seed in [3, 11] {
        let setup = zones_fuzz_setup(3, seed).unwrap();
        let first = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let second = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        assert_eq!(first.final_hash, second.final_hash, "seed {seed}");
        assert_eq!(first.events, second.events, "seed {seed}");
    }
}
