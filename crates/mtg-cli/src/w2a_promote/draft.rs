//! `promote --draft`: a proposal the checker marks `Unchecked` that is a whole card becomes a draft for players to read,
//! once the engine runs it as a `Read` card and it passes the smoke gate (`docs/cards-dsl.md`, Model-assisted reading,
//! Drafts).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use mtg_cards::ability::Ability;
use mtg_cards::card::{Card, ImplStatus};
use mtg_cards::load::{canonical, unwhole};
use mtg_cards::proposal::partial::{Mark, Refusal, prove};
use mtg_cards::proposal::{Report, Verdict};
use mtg_cards::support::Support;
use mtg_cards::{CardDefinition, PrintedCard};
use mtg_db::{SOURCE_FORGE, SOURCE_SWEEP};

use super::{W2aBasis, W2aDecision, W2aPromotion, W2aSkip};
use crate::drafts::{
    DraftItem, DraftMeta, FIXTURES, META_DIR, SmokeKind, read_definitions_proved, smoke,
};
use crate::error::CliError;

/// The stack of a thread that plays the smoke gate: as large as the main thread's.
const SMOKE_STACK: usize = 8 << 20;

/// The most threads that play the smoke gate at once.
const SMOKE_THREADS: usize = 4;

/// The draft of an `Unchecked` proposal, or why the card is not one: it must be a whole rules file that the engine
/// runs without a missing primitive. The smoke gate and the record come later.
pub fn decide(
    printed: &PrintedCard,
    slug: String,
    report: Report,
    source: String,
    oracle_matches: bool,
) -> Result<W2aPromotion, W2aSkip> {
    let (Some(record), Some(rules)) = (report.support, report.rules) else {
        return Err(W2aSkip::Rejected(Verdict::Unchecked));
    };
    if unwhole(&rules).is_some() {
        return Err(W2aSkip::NotWhole);
    }
    if record.support != Support::Verified {
        return Err(W2aSkip::State(record.support));
    }
    Ok(W2aPromotion {
        slug,
        oracle_id: printed.oracle_id.clone(),
        state: record.support,
        rules,
        draft: true,
        source,
        oracle_matches,
        proof: None,
        lines: Vec::new(),
    })
}

/// The lines of a card left by hand, as (face, start, end).
fn by_hand(card: &Card) -> BTreeSet<(usize, u32, u32)> {
    card.abilities()
        .filter_map(|(face, ability)| match ability {
            Ability::Manual { span, .. } => Some((face, span.start, span.end)),
            _ => None,
        })
        .collect()
}

/// The draft of a Forge translation with gaps, or why the proof refuses it: a matching text, a proof that every line
/// left by hand is safe (`mtg_cards::proposal::partial::prove`) and strictly fewer lines by hand than the compiled
/// `Partial` row, `compiled` (the card the compiler makes). Every refusal is named in `detail`; the smoke gate comes later.
pub fn decide_partial(
    printed: &PrintedCard,
    slug: String,
    report: &Report,
    meta: &DraftMeta,
    compiled: Option<&Card>,
    detail: &mut Option<String>,
) -> Result<W2aPromotion, W2aSkip> {
    let mut named: Vec<String> = Vec::new();
    let mut first = None;
    if !meta.oracle_matches {
        named.push("OracleDiffers".into());
        first = Some("OracleDiffers");
    }
    let proof = match prove(report, &meta.lines) {
        Ok(proof) => Some(proof),
        Err(refusals) => {
            named.extend(refusals.iter().map(ToString::to_string));
            first = first.or_else(|| refusals.first().map(Refusal::name));
            None
        }
    };
    let (Some(proof), Some(rules), None) = (proof, &report.rules, first) else {
        *detail = Some(named.join("; "));
        return Err(W2aSkip::Unproved(first.unwrap_or("InvalidRules")));
    };
    let hand = by_hand(rules);
    let marked = proof.marks.iter().any(|mark| mark.mark == Mark::ByHand);
    let kept = compiled
        .filter(|row| row.impl_status == ImplStatus::Partial)
        .map(by_hand);
    let gain = kept.is_none_or(|before| hand.is_subset(&before) && hand != before);
    if rules.impl_status != ImplStatus::Partial || hand.is_empty() || !marked || !gain {
        *detail =
            Some("NoGain: the draft keeps no more automatic lines than the compiled rules".into());
        return Err(W2aSkip::Unproved("NoGain"));
    }
    Ok(W2aPromotion {
        slug,
        oracle_id: printed.oracle_id.clone(),
        state: Support::Partial,
        rules: rules.clone(),
        draft: true,
        source: SOURCE_FORGE.into(),
        oracle_matches: true,
        proof: Some(proof),
        lines: meta.lines.clone(),
    })
}

/// Plays each draft in `jobs` (an index and its definition) in the smoke gate on a few threads, and returns the failures.
pub fn play(jobs: &[(usize, CardDefinition)]) -> Vec<(usize, SmokeKind, String)> {
    /// The outcome of each job a worker took: its place in `jobs` and the failure, if any.
    type Played = Vec<(usize, Option<(SmokeKind, String)>)>;
    fn work(jobs: &[(usize, CardDefinition)], next: &AtomicUsize) -> Played {
        let mut played = Vec::new();
        loop {
            let place = next.fetch_add(1, Ordering::Relaxed);
            let Some((_, definition)) = jobs.get(place) else {
                return played;
            };
            played.push((place, smoke(definition).err()));
        }
    }
    let next = AtomicUsize::new(0);
    let helpers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(SMOKE_THREADS)
        .min(jobs.len())
        .saturating_sub(1);
    let next = &next;
    let mut played: Played = Vec::new();
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..helpers)
            .filter_map(|_| {
                std::thread::Builder::new()
                    .stack_size(SMOKE_STACK)
                    .spawn_scoped(scope, move || work(jobs, next))
                    .ok()
            })
            .collect();
        played.extend(work(jobs, next));
        for worker in workers {
            if let Ok(done) = worker.join() {
                played.extend(done);
            }
        }
    });
    // A job no thread reported (its thread ended early) fails: a draft is never offered unplayed.
    let mut outcomes: Vec<Option<Option<(SmokeKind, String)>>> = vec![None; jobs.len()];
    for (place, outcome) in played {
        outcomes[place] = Some(outcome);
    }
    outcomes
        .into_iter()
        .zip(jobs)
        .filter_map(|(outcome, (at, _))| {
            let failure = outcome.unwrap_or_else(|| {
                Some((SmokeKind::Panicked, "the smoke thread ended early".into()))
            })?;
            Some((*at, failure.0, failure.1))
        })
        .collect()
}

/// Turns each draft the engine would not run as a `Read` card, and (unless `smoke_gate` is off) each that fails the
/// smoke gate, into a skip with the reason; of several drafts of one card the first that passes is kept, and none is
/// kept for a card that a corpus promotion covers.
pub fn gate(basis: &W2aBasis, decisions: &mut [W2aDecision], smoke_gate: bool) {
    play_candidates(basis, decisions, smoke_gate);
    let mut taken: BTreeSet<String> = decisions
        .iter()
        .filter_map(|decision| decision.outcome.as_ref().ok())
        .filter(|promotion| !promotion.draft)
        .map(|promotion| promotion.oracle_id.clone())
        .collect();
    for decision in decisions.iter_mut() {
        if let Ok(promotion) = &decision.outcome
            && promotion.draft
            && !taken.insert(promotion.oracle_id.clone())
        {
            decision.outcome = Err(W2aSkip::Repeated);
        }
    }
}

fn play_candidates(basis: &W2aBasis, decisions: &mut [W2aDecision], smoke_gate: bool) {
    let by_id: BTreeMap<&str, &PrintedCard> = basis
        .cards
        .iter()
        .map(|card| (card.oracle_id.as_str(), card))
        .collect();
    let candidates: Vec<usize> = decisions
        .iter()
        .enumerate()
        .filter_map(|(at, d)| matches!(&d.outcome, Ok(p) if p.draft).then_some(at))
        .collect();
    let items: Vec<DraftItem> = candidates
        .iter()
        .filter_map(|&at| {
            let promotion = decisions[at].outcome.as_ref().ok()?;
            Some((
                *by_id.get(promotion.oracle_id.as_str())?,
                &promotion.rules,
                promotion.proof.as_ref(),
            ))
        })
        .collect();
    let defined = if items.len() == candidates.len() {
        read_definitions_proved(basis.library, basis.caps, &items)
    } else {
        candidates
            .iter()
            .map(|_| Err("the card is not stored".to_owned()))
            .collect()
    };
    let mut jobs = Vec::new();
    for (&at, definition) in candidates.iter().zip(defined) {
        match definition {
            Ok(definition) => jobs.push((at, definition)),
            Err(message) => {
                decisions[at].outcome = Err(W2aSkip::NotRead);
                decisions[at].detail = Some(message);
            }
        }
    }
    if !smoke_gate {
        return;
    }
    for (at, kind, detail) in play(&jobs) {
        decisions[at].outcome = Err(W2aSkip::Smoke(kind));
        decisions[at].detail = Some(detail);
    }
}

/// Writes the draft and its record below `dir`.
pub fn write(
    dir: &Path,
    promotion: &W2aPromotion,
    record: &str,
) -> Result<std::path::PathBuf, CliError> {
    write_meta(dir, promotion)?;
    let file = format!("{}.json", promotion.slug);
    let fixtures = dir.join(FIXTURES);
    std::fs::create_dir_all(&fixtures).map_err(|source| CliError::io(&fixtures, source))?;
    let rules = dir.join(&file);
    std::fs::write(&rules, canonical(&promotion.rules))
        .map_err(|source| CliError::io(&rules, source))?;
    let fixture = fixtures.join(&file);
    std::fs::write(&fixture, record).map_err(|source| CliError::io(&fixture, source))?;
    Ok(rules)
}

/// Writes `meta/<slug>.json` for a draft that is not a plain sweep draft, and removes a stale one for a sweep draft.
fn write_meta(dir: &Path, promotion: &W2aPromotion) -> Result<(), CliError> {
    let path = dir.join(META_DIR).join(format!("{}.json", promotion.slug));
    if promotion.source == SOURCE_SWEEP && !promotion.oracle_matches {
        return match std::fs::remove_file(&path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                Err(CliError::io(&path, error))
            }
            _ => Ok(()),
        };
    }
    let meta = DraftMeta {
        source: Some(promotion.source.clone()),
        oracle_matches: promotion.oracle_matches,
        gaps: Vec::new(),
        lines: promotion.lines.clone(),
        proof: promotion.proof.clone(),
    };
    let text = serde_json::to_string_pretty(&meta).unwrap_or_default();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| CliError::io(parent, source))?;
    }
    std::fs::write(&path, format!("{text}\n")).map_err(|source| CliError::io(&path, source))
}

/// Removes the draft of a card that has just become a corpus card, and its record; returns whether there was one.
pub fn remove_superseded(dir: &Path, slug: &str) -> Result<bool, CliError> {
    let file = format!("{slug}.json");
    let rules = dir.join(&file);
    if !rules.is_file() {
        return Ok(false);
    }
    for path in [
        rules,
        dir.join(FIXTURES).join(&file),
        dir.join(META_DIR).join(&file),
    ] {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => return Err(CliError::io(&path, source)),
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    //! Goal: only a whole `Unchecked` card the engine runs becomes a draft candidate (a `Manual` ability, a status other
    //! than `full` or a state other than `Verified` is refused with its reason), the smoke gate names the draft that
    //! fails among those that pass, and a draft the corpus supersedes is removed with its record.
    #![allow(clippy::unwrap_used)]

    use mtg_cards::proposal::Report;
    use mtg_cards::support::SupportRecord;
    use mtg_testkit::cards::creature;
    use rstest::rstest;

    use super::*;

    const MANUAL: &str = r#"{"Manual":{"reason":"left to the table","span":{"start":0,"end":3}}}"#;

    fn card(status: &str, abilities: &[&str]) -> Card {
        let abilities = abilities.join(",");
        let text = format!(
            r#"{{"oracle_id":"o","name":"X","impl_status":"{status}","faces":[{{"name":"X","oracle_fingerprint":"{}","abilities":[{abilities}]}}]}}"#,
            "0".repeat(32)
        );
        serde_json::from_str(&text).unwrap()
    }

    fn report(rules: Option<Card>, state: Option<Support>) -> Report {
        Report {
            card: "X".into(),
            problems: Vec::new(),
            clauses: Vec::new(),
            strays: Vec::new(),
            support: state.map(|support| SupportRecord {
                oracle_id: "o".into(),
                support,
                clauses: Vec::new(),
                fingerprint: String::new(),
            }),
            repairs: Vec::new(),
            rules,
        }
    }

    fn printed() -> PrintedCard {
        PrintedCard {
            oracle_id: "o".into(),
            name: "X".into(),
            ..PrintedCard::default()
        }
    }

    #[rstest]
    #[case::whole("Full", &[], Some(Support::Verified), None)]
    #[case::a_manual_ability("Full", &[MANUAL], Some(Support::Verified), Some(W2aSkip::NotWhole))]
    #[case::a_partial_status("Partial", &[], Some(Support::Partial), Some(W2aSkip::NotWhole))]
    #[case::a_primitive_the_engine_lacks("Full", &[], Some(Support::Partial), Some(W2aSkip::State(Support::Partial)))]
    #[case::no_state("Full", &[], None, Some(W2aSkip::Rejected(Verdict::Unchecked)))]
    fn a_draft_is_a_whole_card_the_engine_runs(
        #[case] status: &str,
        #[case] abilities: &[&str],
        #[case] state: Option<Support>,
        #[case] skip: Option<W2aSkip>,
    ) {
        let rules = card(status, abilities);

        let decided = decide(
            &printed(),
            "x".into(),
            report(Some(rules), state),
            "sweep".into(),
            false,
        );

        match (decided, skip) {
            (Ok(promotion), None) => {
                assert!(promotion.draft);
                assert_eq!(promotion.slug, "x");
                assert_eq!(promotion.oracle_id, "o");
            }
            (Err(found), Some(expected)) => assert_eq!(found, expected),
            (found, expected) => panic!("{found:?}, expected {expected:?}"),
        }
    }

    #[test]
    fn the_gate_names_the_drafts_that_fail_among_those_that_pass() {
        let plain = (*creature("Bear", 2, &[], 2, 2)).clone();
        // Costs more than any library's lands can pay: no game casts it.
        let unplayable = (*creature("Colossus", 20, &[], 9, 9)).clone();
        let jobs = vec![
            (3, plain.clone()),
            (5, unplayable),
            (8, plain.clone()),
            (9, plain),
        ];

        let failed = play(&jobs);

        assert_eq!(failed.len(), 1, "{failed:?}");
        assert_eq!((failed[0].0, failed[0].1), (5, SmokeKind::NeverSeen));
    }

    #[test]
    fn a_corpus_card_removes_the_draft_and_its_record() {
        let dir = tempfile::tempdir().unwrap();
        let fixtures = dir.path().join(FIXTURES);
        std::fs::create_dir_all(&fixtures).unwrap();
        for folder in [dir.path(), fixtures.as_path()] {
            std::fs::write(folder.join("bolt.json"), "{}").unwrap();
            std::fs::write(folder.join("other.json"), "{}").unwrap();
        }

        assert!(remove_superseded(dir.path(), "bolt").unwrap());
        assert!(!remove_superseded(dir.path(), "bolt").unwrap());
        assert!(!remove_superseded(dir.path(), "never_drafted").unwrap());

        assert!(!dir.path().join("bolt.json").exists());
        assert!(!fixtures.join("bolt.json").exists());
        assert!(dir.path().join("other.json").exists() && fixtures.join("other.json").exists());
    }
}

#[cfg(test)]
mod partial_tests {
    //! Goal: a Forge translation with a gap is drafted only through the proof, and each refusal is named: a differing
    //! text, a gap no ledger line explains, and a draft that keeps no more automatic lines than the compiled rules.
    #![allow(clippy::unwrap_used)]

    use mtg_cards::Caps;
    use mtg_cards::compile::compile;
    use mtg_cards::load::Library;
    use mtg_cards::proposal::check;
    use mtg_cards::proposal::partial::LedgerLine;
    use mtg_cards::support::PrintedFace;
    use rstest::rstest;

    use super::*;

    const ROLL: &str = "{2}, {T}: Roll a six-sided die.";

    fn angel(text: &str) -> PrintedCard {
        PrintedCard {
            oracle_id: "00000000-0000-0000-0000-000000000001".into(),
            name: "Angel".into(),
            layout: "normal".into(),
            faces: vec![PrintedFace {
                name: "Angel".into(),
                mana_cost: "{2}{W}".into(),
                type_line: "Creature \u{2014} Angel".into(),
                oracle_text: text.into(),
                ..PrintedFace::default()
            }],
            ..PrintedCard::default()
        }
    }

    fn row(tag: &str, text: &str, abilities: &[usize], gap: Option<&str>) -> LedgerLine {
        LedgerLine {
            face: 0,
            tag: tag.into(),
            text: text.into(),
            abilities: abilities.to_vec(),
            gap: gap.map(str::to_owned),
        }
    }

    fn rows(keyword: &str) -> Vec<LedgerLine> {
        vec![
            row("K", keyword, &[0], None),
            row(
                "A",
                "{2}, {T}: Roll a six-sided die.",
                &[],
                Some("effect:RollDice"),
            ),
        ]
    }

    /// The decision for a card of `keyword` and the roll line, which the translator leaves as a gap.
    fn decided(
        keyword: &str,
        (matches, compiled): (bool, bool),
        lines: Vec<LedgerLine>,
    ) -> (Result<W2aPromotion, W2aSkip>, Option<String>) {
        let printed = angel(&format!("{keyword}\n{ROLL}"));
        let compiled = compiled.then(|| compile(&printed).card).flatten();
        let proposal = format!(r#"{{"faces":[{{"abilities":[{{"Keyword":"{keyword}"}}]}}]}}"#);
        let names = BTreeSet::from(["angel".to_owned()]);
        let caps = Caps::everything();
        let report = check(&proposal, &printed, &Library::default(), &names, &caps);
        let meta = DraftMeta {
            source: Some("forge".into()),
            oracle_matches: matches,
            gaps: vec!["effect:RollDice".into()],
            lines,
            proof: None,
        };
        let mut detail = None;
        let out = decide_partial(
            &printed,
            "angel".into(),
            &report,
            &meta,
            compiled.as_ref(),
            &mut detail,
        );
        (out, detail)
    }

    #[test]
    fn a_proved_partial_translation_is_a_draft_with_its_proof_and_ledger() {
        let (out, detail) = decided("Flying", (true, false), rows("Flying"));

        let promotion = out.unwrap_or_else(|skip| panic!("{skip:?} {detail:?}"));
        let proof = promotion.proof.unwrap();
        let marks: Vec<Mark> = proof.marks.iter().map(|m| m.mark).collect();
        assert_eq!(marks, [Mark::Auto, Mark::ByHand]);
        assert_eq!(promotion.lines, rows("Flying"));
        assert_eq!(promotion.state, Support::Partial);
        assert!(promotion.draft && promotion.oracle_matches);
    }

    #[rstest]
    #[case::text_differs((false, false), rows("Flying"), "OracleDiffers")]
    #[case::no_ledger((true, false), Vec::new(), "OpenWithoutGap")]
    #[case::the_compiled_row_has_the_same_line_by_hand((true, true), rows("Flying"), "NoGain")]
    fn a_refusal_is_named(
        #[case] (matches, compiled): (bool, bool),
        #[case] lines: Vec<LedgerLine>,
        #[case] name: &'static str,
    ) {
        let (out, detail) = decided("Flying", (matches, compiled), lines);

        assert_eq!(out.unwrap_err(), W2aSkip::Unproved(name));
        assert!(detail.unwrap().contains(name));
    }

    #[test]
    fn the_meta_of_a_partial_draft_keeps_its_proof_and_a_build_loads_it_only_with_that_proof() {
        let (out, _) = decided("Flying", (true, false), rows("Flying"));
        let promotion = out.unwrap();
        let dir = tempfile::tempdir().unwrap();

        write_meta(dir.path(), &promotion).unwrap();

        let text = std::fs::read_to_string(dir.path().join(META_DIR).join("angel.json")).unwrap();
        let meta = DraftMeta::parse(&text);
        assert_eq!(meta.proof, promotion.proof);
        assert_eq!(meta.lines, rows("Flying"));
        let printed = angel(&format!("Flying\n{ROLL}"));
        let caps = Caps::everything();
        let fits = |proof| {
            crate::drafts::misfits_proved(
                &Library::default(),
                &caps,
                &[(&printed, &promotion.rules, proof)],
            )
        };
        assert_eq!(fits(meta.proof.as_ref()), [None]);
        assert!(matches!(
            fits(None)[..],
            [Some(crate::drafts::Misfit::Unloadable(_))]
        ));
    }

    #[test]
    fn the_smoke_gate_plays_a_partial_draft_with_its_proof() {
        let (out, _) = decided("Flying", (true, false), rows("Flying"));
        let printed = angel(&format!("Flying\n{ROLL}"));
        let mut decisions = vec![W2aDecision {
            file: "angel.json".into(),
            card: "Angel".into(),
            outcome: out,
            detail: None,
        }];
        let (names, library, caps) = (BTreeSet::new(), Library::default(), mtg_engine::caps());
        let cards = [printed];
        let basis = W2aBasis {
            cards: &cards,
            library: &library,
            names: &names,
            corpus: &names,
            caps: &caps,
            out: Path::new("."),
            partial: false,
            draft: true,
            source: None,
        };

        gate(&basis, &mut decisions, true);

        let outcome = &decisions[0].outcome;
        assert!(outcome.is_ok(), "{outcome:?} {:?}", decisions[0].detail);
    }
}
