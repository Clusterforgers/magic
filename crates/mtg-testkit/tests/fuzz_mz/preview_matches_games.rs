//! Goal: in bot-driven games at every table size, the preview of the answer a bot is about to give to a declare-attackers or declare-blockers decision equals the forecast read after really giving it (also for answers the bot did not give), a spoiled answer is refused as the real answer is, and a game played with previews at every combat decision is the same game, log and state, as one played without.

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::forecast::{CombatForecast, pending_combat_forecast};
use mtg_engine::{Decision, Engine, EngineError, Event, GameSetup, PreviewError};
use mtg_testkit::{RandomBot, fuzz_setup, keyword_fuzz_setup};
use proptest::prelude::*;
use rstest::rstest;

/// What a preview says: a forecast or why there is none.
type Preview = Result<CombatForecast, PreviewError>;

/// What one game exercised, so a passing property cannot hide an empty one.
#[derive(Debug, Default, Clone, Copy)]
struct Coverage {
    attacks: u32,
    empty_attacks: u32,
    blocks: u32,
    empty_blocks: u32,
    multi_blocks: u32,
    deaths: u32,
    lethal_players: u32,
    refusals: u32,
    alternatives: u32,
}

impl Coverage {
    fn add(&mut self, other: Coverage) {
        self.attacks += other.attacks;
        self.empty_attacks += other.empty_attacks;
        self.blocks += other.blocks;
        self.empty_blocks += other.empty_blocks;
        self.multi_blocks += other.multi_blocks;
        self.deaths += other.deaths;
        self.lethal_players += other.lethal_players;
        self.refusals += other.refusals;
        self.alternatives += other.alternatives;
    }
}

/// How a game is played: whether combat decisions are previewed first and how many other answers are tried per decision.
#[derive(Debug, Clone, Copy)]
struct Mode {
    previews: bool,
    alternatives: usize,
}

const PLAIN: Mode = Mode {
    previews: false,
    alternatives: 0,
};
const PREVIEWED: Mode = Mode {
    previews: true,
    alternatives: 0,
};
const WITH_ALTERNATIVES: Mode = Mode {
    previews: true,
    alternatives: 2,
};

/// Everything an engine shows from outside.
fn outside(engine: &Engine) -> (u64, usize, Option<Decision>) {
    (
        engine.state_hash(),
        engine.events().len(),
        engine.decision().cloned(),
    )
}

fn is_combat(decision: &Decision) -> bool {
    matches!(
        decision.kind,
        DecisionKind::DeclareAttackers(_) | DecisionKind::DeclareBlockers(_)
    )
}

/// The answer spoiled in a way the engine must refuse: a repeated entry, else a creature that does not exist.
fn spoiled(answer: &Answer, extra: ObjectId) -> Option<Answer> {
    match answer {
        Answer::DeclareBlockers(blocks) => {
            let mut bad = blocks.clone();
            match blocks.first() {
                Some(first) => bad.push(*first),
                None => bad.push((extra, extra)),
            }
            Some(Answer::DeclareBlockers(bad))
        }
        Answer::DeclareAttackers(attacks) => {
            let mut bad = attacks.clone();
            match attacks.first() {
                Some(first) => bad.push(*first),
                None => bad.push((extra, AttackTarget::Player(PlayerId(0)))),
            }
            Some(Answer::DeclareAttackers(bad))
        }
        _ => None,
    }
}

/// Previews `answer` (and a spoiled copy of it), checks that nothing moved, and returns the preview.
fn preview_checked(
    engine: &mut Engine,
    decision: &Decision,
    answer: &Answer,
    coverage: &mut Coverage,
) -> Result<Preview, String> {
    let before = outside(engine);
    let seat = decision.player;
    let preview = engine.preview_forecast(seat, answer);
    // Another seat is never shown this decision's forecast.
    let other = PlayerId::from_index((seat.index() + 1) % engine.state().players().len());
    if engine.preview_forecast(other, answer) != Err(PreviewError::NotYourDecision(other)) {
        return Err(format!("{other:?} could preview {seat:?}'s decision"));
    }
    if let Some(bad) = spoiled(answer, ObjectId(u64::MAX)) {
        let previewed = engine.preview_forecast(seat, &bad);
        let real = engine.apply(bad);
        match (previewed, real) {
            (Err(PreviewError::Illegal(a)), Err(EngineError::Illegal(b))) if a == b => {
                coverage.refusals += 1;
            }
            (previewed, real) => {
                return Err(format!(
                    "a spoiled answer: preview {previewed:?}, real {real:?}"
                ));
            }
        }
    }
    if outside(engine) != before {
        return Err("a preview or a refused answer moved the game".to_owned());
    }
    Ok(preview)
}

/// The forecast read after really giving `answer` must be the preview.
fn same_as_real(preview: Preview, engine: &Engine, coverage: &mut Coverage) -> Result<(), String> {
    let real = pending_combat_forecast(engine.state());
    match (preview, real) {
        (Ok(previewed), Some(real)) if previewed == real => {
            coverage.multi_blocks += u32::from(real.attackers.iter().any(|a| a.blockers.len() > 1));
            coverage.deaths += u32::try_from(real.dying().count()).unwrap_or(0);
            coverage.lethal_players +=
                u32::try_from(real.players.iter().filter(|p| p.lethal).count()).unwrap_or(0);
            Ok(())
        }
        (Err(PreviewError::NoCombat), None) => Ok(()),
        (previewed, real) => Err(format!(
            "the preview and the forecast after the answer differ: preview {previewed:?}, real {real:?}"
        )),
    }
}

/// Plays a bot game in `mode`; returns its log, final state hash and what it exercised.
fn play(players: usize, seed: u64, mode: Mode) -> Result<(Vec<Event>, u64, Coverage), String> {
    play_decks(players, seed, mode, fuzz_setup)
}

/// A game between creatures with the keywords of combat, played in `mode`.
fn play_keywords(
    players: usize,
    seed: u64,
    mode: Mode,
) -> Result<(Vec<Event>, u64, Coverage), String> {
    play_decks(players, seed, mode, keyword_fuzz_setup)
}

/// Plays a bot game from `setup(players, seed)` in `mode`; returns its log, final state hash and what it exercised.
fn play_decks(
    players: usize,
    seed: u64,
    mode: Mode,
    setup: fn(usize, u64) -> GameSetup,
) -> Result<(Vec<Event>, u64, Coverage), String> {
    let mut engine = Engine::new(setup(players, seed)).map_err(|e| e.to_string())?;
    let mut bots: Vec<RandomBot> = (0..players as u64)
        .map(|seat| RandomBot::new(seed, seat))
        .collect();
    let mut others = RandomBot::new(seed ^ 0x5eed_a17e, 99);
    let mut coverage = Coverage::default();
    for _ in 0..200_000 {
        let Some(decision) = engine.decision().cloned() else {
            let hash = engine.state_hash();
            return Ok((engine.events().to_vec(), hash, coverage));
        };
        let answer = bots[decision.player.index()].choose(&decision);
        if !(mode.previews && is_combat(&decision)) {
            engine
                .apply(answer)
                .map_err(|e| format!("seed {seed}: engine refused an answer: {e}"))?;
            continue;
        }
        match &answer {
            Answer::DeclareAttackers(attacks) => {
                coverage.attacks += 1;
                coverage.empty_attacks += u32::from(attacks.is_empty());
            }
            Answer::DeclareBlockers(blocks) => {
                coverage.blocks += 1;
                coverage.empty_blocks += u32::from(blocks.is_empty());
            }
            _ => {}
        }
        // Answers nobody gave: each is previewed, given for real, compared and then taken back.
        for _ in 0..mode.alternatives {
            let alternative = others.choose(&decision);
            let preview = preview_checked(&mut engine, &decision, &alternative, &mut coverage)
                .map_err(|e| format!("seed {seed}, {players} players: {e}"))?;
            let checkpoint = engine.checkpoint();
            engine
                .apply(alternative)
                .map_err(|e| format!("seed {seed}: engine refused a legal alternative: {e}"))?;
            same_as_real(preview, &engine, &mut coverage)
                .map_err(|e| format!("seed {seed}, {players} players, alternative: {e}"))?;
            engine
                .restore(checkpoint)
                .map_err(|e| format!("seed {seed}: restore failed: {e}"))?;
            coverage.alternatives += 1;
        }
        let preview = preview_checked(&mut engine, &decision, &answer, &mut coverage)
            .map_err(|e| format!("seed {seed}, {players} players: {e}"))?;
        engine
            .apply(answer)
            .map_err(|e| format!("seed {seed}: engine refused an answer: {e}"))?;
        same_as_real(preview, &engine, &mut coverage)
            .map_err(|e| format!("seed {seed}, {players} players: {e}"))?;
    }
    Err(format!("seed {seed}: game did not finish"))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn the_preview_agrees_with_the_forecast_after_the_answer(players in 2usize..=4, seed in any::<u64>()) {
        if let Err(difference) = play(players, seed, PREVIEWED) {
            prop_assert!(false, "{}", difference);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(6))]

    #[test]
    fn answers_nobody_gave_are_previewed_as_they_would_play_out(players in 2usize..=4, seed in any::<u64>()) {
        if let Err(difference) = play(players, seed, WITH_ALTERNATIVES) {
            prop_assert!(false, "{}", difference);
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn a_game_with_previews_is_the_game_without_them(#[case] players: usize) {
    for seed in 0..12 {
        let (plain_events, plain_hash, _) =
            play(players, seed, PLAIN).unwrap_or_else(|difference| panic!("{difference}"));
        let (events, hash, coverage) =
            play(players, seed, PREVIEWED).unwrap_or_else(|difference| panic!("{difference}"));
        assert_eq!(events, plain_events, "seed {seed}: the logs differ");
        assert_eq!(hash, plain_hash, "seed {seed}: the states differ");
        assert!(coverage.attacks + coverage.blocks > 0, "seed {seed}");
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn the_games_exercise_every_kind_of_preview(#[case] players: usize) {
    let mut total = Coverage::default();
    for seed in 0..40 {
        let (_, _, coverage) =
            play(players, seed, PREVIEWED).unwrap_or_else(|difference| panic!("{difference}"));
        total.add(coverage);
    }
    assert!(total.attacks > 0, "{total:?}");
    assert!(total.empty_attacks > 0, "{total:?}");
    assert!(total.blocks > 0, "{total:?}");
    assert!(total.empty_blocks > 0, "{total:?}");
    assert!(total.multi_blocks > 0, "{total:?}");
    assert!(total.deaths > 0, "{total:?}");
    assert!(total.lethal_players > 0, "{total:?}");
    assert!(total.refusals > 0, "{total:?}");
}

#[rstest]
#[case(2)]
#[case(4)]
fn alternatives_are_tried_too(#[case] players: usize) {
    let mut total = Coverage::default();
    for seed in 0..6 {
        let (_, _, coverage) = play(players, seed, WITH_ALTERNATIVES)
            .unwrap_or_else(|difference| panic!("{difference}"));
        total.add(coverage);
    }
    assert!(total.alternatives > 20, "{total:?}");
    assert!(total.deaths > 0, "{total:?}");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    #[test]
    fn the_preview_agrees_with_the_forecast_between_creatures_with_keywords(players in 2usize..=4, seed in any::<u64>()) {
        if let Err(difference) = play_keywords(players, seed, PREVIEWED) {
            prop_assert!(false, "{}", difference);
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn a_keyword_game_with_previews_alternatives_and_refusals_is_the_game_without_them(
    #[case] players: usize,
) {
    let mut total = Coverage::default();
    for seed in 0..10 {
        let (plain_events, plain_hash, _) =
            play_keywords(players, seed, PLAIN).unwrap_or_else(|difference| panic!("{difference}"));
        let (events, hash, coverage) = play_keywords(players, seed, WITH_ALTERNATIVES)
            .unwrap_or_else(|difference| panic!("{difference}"));
        assert_eq!(events, plain_events, "seed {seed}: the logs differ");
        assert_eq!(hash, plain_hash, "seed {seed}: the states differ");
        total.add(coverage);
    }
    assert!(total.alternatives > 10, "{total:?}");
    assert!(total.deaths > 0, "{total:?}");
    assert!(total.refusals > 0, "{total:?}");
}
