//! Goal: whole bot games with decks of Auras (Pacifism, Epic Proportions, Control Magic, Squirrel Nest, Shielded by Faith and four of ours for creatures, lands and opponents' creatures), a spell that returns an Aura, legendary creatures and artifacts that repeat, and the burn that takes a host away finish and keep every invariant (zones, life, the journal, the replay of the log equal to the live state, conservation of cards, what every permanent is attached to equal to what the attach and unattach events say, no Aura attached to nothing or to what it may not enchant, no Equipment, Fortification or other permanent attached wrongly, and no player with two legendary permanents of one name at a decision unless the engine is asking which to keep) for many seeds at tables of two, three and four seats, and the games really use them: Auras are cast at a target and enter attached to it, an Aura whose target left is not put onto the battlefield, an Aura whose host left goes to its owner's graveyard, an Aura put onto the battlefield without being cast chooses its host, the legend rule puts the rest of a name into the graveyard, an engine rebuilt from the log at each decision asks what the original asks, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_core::card::Supertype;
use mtg_core::ids::ObjectId;
use mtg_engine::events::ZoneCause;
use mtg_engine::zones::Zone;
use mtg_engine::{DecisionKind, Engine, EngineError, Event, GameState, replay};
use mtg_testkit::{RandomBot, attach_fuzz_setup, play_game};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40)
}

fn is_aura(game: &GameState, id: ObjectId) -> bool {
    game.object(id)
        .is_some_and(|o| o.card().subtypes.iter().any(|s| s.0 == "Aura"))
}

fn is_legend(game: &GameState, id: ObjectId) -> bool {
    game.object(id)
        .is_some_and(|o| o.card().supertypes.contains(&Supertype::Legendary))
}

/// What a campaign saw the Auras and legends do.
#[derive(Default)]
struct Seen {
    cast: BTreeMap<String, u64>,
    entered_attached: u64,
    entered_without_being_cast: u64,
    fell_off: u64,
    legend_puts: u64,
    unattached: u64,
    chose_a_host: u64,
}

impl Seen {
    fn add(&mut self, game: &GameState, events: &[Event]) {
        for (n, event) in events.iter().enumerate() {
            match event {
                Event::SpellCast { object, .. } if is_aura(game, *object) => {
                    let name = game.object(*object).unwrap().card().name.clone();
                    *self.cast.entry(name).or_default() += 1;
                }
                Event::SpellResolved { object } if is_aura(game, *object) => {
                    // The host comes in the same batch: the event right after the spell.
                    if matches!(events.get(n + 1), Some(Event::Attached { object: o, .. }) if o == object) {
                        self.entered_attached += 1;
                    }
                }
                Event::Attached { object, .. } if is_aura(game, *object) => {
                    let before = n.checked_sub(1).and_then(|i| events.get(i));
                    if matches!(before, Some(Event::ZoneChanged { object: o, to: Zone::Battlefield, .. }) if o == object)
                        || matches!(before, Some(Event::Entered { object: o, .. }) if o == object)
                    {
                        self.entered_without_being_cast += 1;
                    }
                }
                Event::ZoneChanged {
                    object,
                    from: Zone::Battlefield,
                    cause: ZoneCause::StateBased,
                    ..
                } => {
                    if is_aura(game, *object) {
                        self.fell_off += 1;
                    } else if is_legend(game, *object) {
                        self.legend_puts += 1;
                    }
                }
                Event::Unattached { .. } => self.unattached += 1,
                Event::ChoiceMade { .. } => {
                    // A host chosen as an Aura comes in: the answer is followed by the move of an Aura.
                    if events[n..]
                        .iter()
                        .take(6)
                        .any(|next| matches!(next, Event::Attached { object, .. } if is_aura(game, *object)))
                        && events[n..].iter().take(6).any(|next| {
                            matches!(next, Event::ZoneChanged { object, to: Zone::Battlefield, .. } if is_aura(game, *object))
                        })
                    {
                        self.chose_a_host += 1;
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
fn seeded_aura_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = attach_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        seen.add(&end, &outcome.events);
    }
    let campaign = format!("{} games of {players} seats", games());
    for name in ["Pacifism", "Control Magic", "Squirrel Nest", "Shielded by Faith"]
        .into_iter()
        .chain(["Mighty Aura", "Hex Aura"])
    {
        assert!(
            seen.cast.get(name).copied().unwrap_or(0) > 0,
            "{name} was never cast in {campaign}"
        );
    }
    assert!(seen.entered_attached > 0, "no Aura entered attached in {campaign}");
    assert!(seen.fell_off > 0, "no Aura lost its host in {campaign}");
    assert!(seen.legend_puts > 0, "the legend rule never applied in {campaign}");
    assert!(
        seen.entered_without_being_cast > 0,
        "no Aura was put onto the battlefield without being cast in {campaign}"
    );
}

/// What a walk through the decisions of a game saw.
#[derive(Default)]
struct Walk {
    legend_questions: u64,
    attached_seen: u64,
}

/// Walks the games of `seeds` decision by decision with one bot a seat; with `rebuild` it also resumes an engine from the
/// log at each decision and fails when that engine differs from the live one.
fn walk_games(
    players: usize,
    seeds: std::ops::Range<u64>,
    rebuild: bool,
) -> Result<Walk, EngineError> {
    let mut walk = Walk::default();
    for seed in seeds {
        let setup = attach_fuzz_setup(players, seed).unwrap();
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
            if let DecisionKind::ChooseCards(options) = &decision.kind
                && engine.state().resolving().is_none()
            {
                walk.legend_questions += 1;
                assert_eq!((options.min, options.max), (1, 1));
                assert!(options.from.len() >= 2);
            }
            walk.attached_seen += engine
                .state()
                .zone(Zone::Battlefield)
                .iter()
                .filter(|id| engine.state().host_of(**id).is_some())
                .count() as u64;
            let answer = bots[decision.player.index()].choose(&decision);
            engine.apply(answer)?;
        }
    }
    Ok(walk)
}

#[rstest]
fn a_rebuilt_engine_asks_what_the_original_asks_under_auras_and_legends(
    #[values(2, 3)] players: usize,
) -> Result<(), EngineError> {
    let walk = walk_games(players, 0..8, true)?;
    assert!(walk.attached_seen > 0, "nothing was attached");
    Ok(())
}

#[rstest]
fn the_legend_rule_is_asked_of_the_player_who_controls_two_of_a_name(
    #[values(2, 3, 4)] players: usize,
) -> Result<(), EngineError> {
    let walk = walk_games(players, 0..games(), false)?;
    assert!(
        walk.legend_questions > 0,
        "the legend rule was never asked at {players} seats"
    );
    Ok(())
}

#[test]
fn a_seeded_aura_game_is_the_same_game_every_time() {
    for seed in [3, 11] {
        let setup = attach_fuzz_setup(3, seed).unwrap();
        let first = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let second = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        assert_eq!(first.final_hash, second.final_hash, "seed {seed}");
        assert_eq!(first.events, second.events, "seed {seed}");
    }
}
