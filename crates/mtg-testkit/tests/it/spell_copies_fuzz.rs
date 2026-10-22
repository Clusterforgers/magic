//! Goal: whole bot games with decks of Twincast, Reverberate, Fork, Reiterate, Dualcaster Mage, Strionic Resonator, Lithoform Engine, Cytoshape, Grapeshot, Brain Freeze and Tendrils of Agony finish and keep every invariant (zones, life, mana, the journal, the replay of the log equal to the live state, a copy that is no card never in a zone but the stack and the battlefield) for many seeds at tables of two, three and four seats, and the games really copy: spells and abilities are copied, a copy chooses new targets, and a copy is never cast; an engine rebuilt from the log at each decision asks the same question, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::decision::DecisionKind;
use mtg_engine::{Engine, EngineError, Event};
use mtg_testkit::{RandomBot, play_game, spell_copy_fuzz_setup};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30)
}

#[derive(Default)]
struct Seen {
    spells_copied: u64,
    abilities_copied: u64,
    temporary_copies: u64,
}

impl Seen {
    fn add(&mut self, events: &[Event]) {
        for event in events {
            match event {
                Event::SpellCopied(_) => self.spells_copied += 1,
                Event::AbilityCopied(_) => self.abilities_copied += 1,
                Event::BecameCopy(copy) if copy.until_end_of_turn => self.temporary_copies += 1,
                _ => {}
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_spell_copy_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = spell_copy_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        seen.add(&outcome.events);
    }
    let campaign = format!("{} games of {players} seats", games());
    assert!(seen.spells_copied > 0, "no spell was copied in {campaign}");
    assert!(
        seen.abilities_copied + seen.temporary_copies > 0,
        "no ability was copied and no creature became a copy in {campaign}"
    );
}

#[rstest]
fn a_rebuilt_engine_asks_what_the_original_asks_including_new_targets_of_a_copy(
    #[values(2, 3, 4)] players: usize,
) -> Result<(), EngineError> {
    let mut questions = 0;
    for seed in 0..8 {
        let setup = spell_copy_fuzz_setup(players, seed).unwrap();
        let mut engine = Engine::new(setup.clone())?;
        let mut bots: Vec<RandomBot> = (0..players as u64)
            .map(|seat| RandomBot::new(seed, seat))
            .collect();
        for step in 0..1500 {
            let Some(decision) = engine.decision().cloned() else {
                break;
            };
            let resumed = Engine::resume(setup.clone(), engine.events().to_vec())?;
            assert_eq!(
                resumed.state_hash(),
                engine.state_hash(),
                "seed {seed} step {step}"
            );
            // A cast not yet paid for, which the announcement holds, lives outside the state until it is answered; every other question is asked again by a rebuilt engine.
            let resolving = engine.state().resolving().is_some();
            let outside = engine.replacing()
                || !resolving
                    && matches!(
                        &decision.kind,
                        DecisionKind::PayMana(_)
                            | DecisionKind::ChooseTargets(_)
                            | DecisionKind::ChooseMode(_)
                            | DecisionKind::ChooseCosts(_)
                            | DecisionKind::ChooseNumber(_)
                            | DecisionKind::DivideAmount(_)
                            | DecisionKind::ChooseOption(_)
                    );
            if !outside {
                assert_eq!(
                    resumed.decision(),
                    engine.decision(),
                    "seed {seed} step {step}"
                );
            }
            if resolving
                && matches!(&decision.kind, DecisionKind::YesNo(options)
                    if options.text.starts_with("Choose new targets for the copy"))
            {
                questions += 1;
            }
            let answer = bots[decision.player.index()].choose(&decision);
            engine.apply(answer)?;
        }
    }
    assert!(
        questions > 0,
        "nobody was asked for new targets at {players} seats"
    );
    Ok(())
}

#[rstest]
fn a_seeded_spell_copy_game_is_the_same_game_every_time(#[values(2, 3, 4)] players: usize) {
    let play = || {
        let setup = spell_copy_fuzz_setup(players, 5).unwrap();
        play_game(setup, 5).unwrap_or_else(|failure| panic!("{failure}"))
    };
    let (first, second) = (play(), play());
    assert_eq!(first.events, second.events);
}
