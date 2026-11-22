//! `refresh`: the daily card refresh of the `magic-ingest` `CronJob`. It fetches Scryfall's bulk files and MTGJSON's
//! Commander decks (conditional requests, so an unchanged upstream costs a listing and an index), then loads and
//! compiles only when something changed, defers that while a game runs (a restart after a rules change fails a stored
//! game, as with releases), takes a coverage snapshot and records the run: the upstream builds, the new cards and
//! precons, and what failed.

use std::collections::{BTreeMap, HashSet};
use std::time::Instant;

use mtg_db::{Database, NewPrecon, RefreshReport, RefreshRun, RefreshStatus};

use crate::args::{CompileArgs, RefreshArgs};
use crate::cache::{Cache, PRINTINGS_TYPE, SCRYFALL_TYPES};
use crate::compile::compile;
use crate::error::CliError;
use crate::fetch::Endpoints;
use crate::fetch::decks::{DeckFilter, fetch_decks};
use crate::fetch::scryfall::{BulkAction, fetch_scryfall_kinds};
use crate::http::Http;
use crate::ingest::{IngestOptions, ingest};
use crate::progress::Progress;
use crate::snapshot::take_snapshot;
use crate::{decks_summary, scryfall_summary};

/// What the fetch steps found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fetched {
    /// Whether any file was downloaded.
    pub changed: bool,
    /// The summary line of each fetch.
    pub steps: Vec<String>,
}

/// What the run does after fetching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    /// Load and compile.
    Load,
    /// Nothing changed since the last successful run of this release: load nothing.
    Unchanged,
    /// This many games are running: load at a later run.
    Defer(u64),
}

/// Whether to load: nothing changed since a successful run of the same release skips it, a running game defers it.
pub fn plan(
    fetched: &Fetched,
    last: Option<&RefreshRun>,
    release: Option<&str>,
    running_games: Option<u64>,
    args: &RefreshArgs,
) -> Plan {
    let release = release.filter(|r| !r.is_empty());
    let same_as_last = last.is_some_and(|run| {
        run.status == RefreshStatus::Succeeded && run.release.as_deref() == release
    });
    if !args.force && !fetched.changed && same_as_last {
        return Plan::Unchanged;
    }
    match running_games {
        Some(running) if running > 0 && !args.even_if_games => Plan::Defer(running),
        _ => Plan::Load,
    }
}

/// Downloads what changed upstream into the cache.
pub async fn fetch(
    http: &Http,
    cache: &Cache,
    endpoints: &Endpoints,
    progress: &Progress,
) -> Result<Fetched, CliError> {
    let started = Instant::now();
    let mut kinds = SCRYFALL_TYPES.to_vec();
    kinds.push(PRINTINGS_TYPE);
    let outcomes = fetch_scryfall_kinds(
        http,
        cache,
        &endpoints.scryfall_listing,
        &kinds,
        false,
        progress,
    )
    .await?;
    let scryfall_changed = outcomes
        .iter()
        .any(|outcome| matches!(outcome.action, BulkAction::Downloaded { .. }));
    let mut steps = vec![scryfall_summary(&outcomes, started.elapsed())];
    let started = Instant::now();
    let decks = fetch_decks(
        http,
        cache,
        endpoints,
        &DeckFilter::default(),
        false,
        progress,
    )
    .await?;
    steps.push(decks_summary(&decks, started.elapsed()));
    Ok(Fetched {
        changed: scryfall_changed || decks.downloaded > 0,
        steps,
    })
}

/// The whole refresh: fetch, then [`after_fetch`]; a failed fetch is recorded as a failed run.
pub async fn refresh(
    db: &Database,
    cache: &Cache,
    http: &Http,
    endpoints: &Endpoints,
    args: &RefreshArgs,
    progress: &Progress,
) -> Result<String, CliError> {
    match fetch(http, cache, endpoints, progress).await {
        Ok(fetched) => after_fetch(db, cache, &fetched, args, progress).await,
        Err(error) => {
            db.migrate().await?;
            let feed = db.card_feed();
            let id = feed.start_refresh(args.release.as_deref()).await?;
            let report = RefreshReport {
                failures: vec![format!("fetch: {error}")],
                ..upstream(cache)
            };
            feed.finish_refresh(id, RefreshStatus::Failed, &report)
                .await?;
            Err(error)
        }
    }
}

/// The builds the cache holds: Scryfall's `updated_at` of Oracle Cards and MTGJSON's index build.
fn upstream(cache: &Cache) -> RefreshReport {
    let manifest = cache.manifest().unwrap_or_default();
    RefreshReport {
        scryfall_updated_at: manifest
            .scryfall
            .get("oracle_cards")
            .map(|record| record.updated_at.clone()),
        mtgjson_build: manifest.mtgjson.index.and_then(|index| index.build),
        ..RefreshReport::default()
    }
}

/// Loads, compiles and snapshots after a fetch as [`plan`] says, and records the run. A failed step is recorded and
/// returned as the error.
pub async fn after_fetch(
    db: &Database,
    cache: &Cache,
    fetched: &Fetched,
    args: &RefreshArgs,
    progress: &Progress,
) -> Result<String, CliError> {
    db.migrate().await?;
    let feed = db.card_feed();
    let last = feed.refreshes(1).await?.into_iter().next();
    let release = args.release.as_deref();
    let running = feed.running_games().await?;
    let plan = plan(fetched, last.as_ref(), release, running, args);
    let id = feed.start_refresh(release).await?;
    let mut report = RefreshReport {
        steps: fetched.steps.clone(),
        ..upstream(cache)
    };
    let outcome = steps(db, cache, plan, release, &mut report, progress).await;
    let status = match (&outcome, plan) {
        (Err(_), _) => RefreshStatus::Failed,
        (Ok(()), Plan::Defer(_)) => RefreshStatus::Deferred,
        (Ok(()), _) => RefreshStatus::Succeeded,
    };
    if let Err(error) = &outcome {
        report.failures.push(error.to_string());
    }
    feed.finish_refresh(id, status, &report).await?;
    outcome?;
    Ok(summary(&report, status))
}

async fn steps(
    db: &Database,
    cache: &Cache,
    plan: Plan,
    release: Option<&str>,
    report: &mut RefreshReport,
    progress: &Progress,
) -> Result<(), CliError> {
    match plan {
        Plan::Unchanged => report
            .steps
            .push("load: nothing changed since the last run".to_owned()),
        Plan::Defer(running) => report
            .steps
            .push(format!("load: deferred, {running} game(s) running")),
        Plan::Load => load(db, cache, report, progress).await?,
    }
    let snapshot = take_snapshot(db, release).await?;
    report.steps.push(snapshot.summary());
    Ok(())
}

/// Ingests and compiles, and notes the deck cards and precons that are new.
async fn load(
    db: &Database,
    cache: &Cache,
    report: &mut RefreshReport,
    progress: &Progress,
) -> Result<(), CliError> {
    let feed = db.card_feed();
    let cards_before = feed.deck_cards().await?;
    let precons_before = feed.precon_names().await?;
    let started = Instant::now();
    let options = IngestOptions {
        force: false,
        batch_size: mtg_db::import::DEFAULT_BATCH_SIZE,
    };
    let ingested = ingest(cache, db, &options, progress).await?;
    report.steps.push(ingested.summary(started.elapsed()));
    let compiled = compile(
        db,
        &CompileArgs {
            all: true,
            card: None,
        },
    )
    .await?;
    report.steps.push(
        compiled
            .render()
            .lines()
            .next()
            .unwrap_or_default()
            .to_owned(),
    );
    let before: HashSet<&str> = cards_before.iter().map(|(id, _)| id.as_str()).collect();
    let mut new_cards: Vec<String> = feed
        .deck_cards()
        .await?
        .into_iter()
        .filter(|(id, _)| !before.contains(id.as_str()))
        .map(|(_, name)| name)
        .collect();
    new_cards.sort();
    report.first_load = cards_before.is_empty();
    report.new_card_count = u32::try_from(new_cards.len()).unwrap_or(u32::MAX);
    report.new_cards = new_cards;
    let before: HashSet<(String, String)> = precons_before.into_iter().collect();
    let mut added: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (code, name) in feed.precon_names().await? {
        if !before.contains(&(code.clone(), name.clone())) {
            added.entry(code).or_default().push(name);
        }
    }
    for (code, mut names) in added {
        names.sort();
        let set_name = feed.set_name(&code).await?;
        report
            .new_precons
            .extend(names.into_iter().map(|name| NewPrecon {
                code: code.clone(),
                name,
                set_name: set_name.clone(),
            }));
    }
    Ok(())
}

/// The summary: the status, the new cards and precons, then each step's line.
fn summary(report: &RefreshReport, status: RefreshStatus) -> String {
    let status = match status {
        RefreshStatus::Running | RefreshStatus::Succeeded => "done",
        RefreshStatus::Deferred => "deferred",
        RefreshStatus::Failed => "failed",
    };
    let mut text = format!(
        "refresh: {status}, {} new cards, {} new precons{}",
        report.new_card_count,
        report.new_precons.len(),
        if report.first_load {
            " (first load)"
        } else {
            ""
        }
    );
    for step in &report.steps {
        text.push_str("\n  ");
        text.push_str(step);
    }
    text
}

#[cfg(test)]
mod tests {
    //! Goal: a refresh loads only when something changed or the last run did not finish loading for this release, and defers loading while a game runs unless told otherwise.
    use mtg_db::RefreshId;
    use rstest::rstest;

    use super::*;

    fn run(status: RefreshStatus, release: Option<&str>) -> RefreshRun {
        RefreshRun {
            id: RefreshId(1),
            started_at: String::new(),
            finished_at: None,
            status,
            release: release.map(str::to_owned),
            report: RefreshReport::default(),
        }
    }

    fn args(force: bool, even_if_games: bool) -> RefreshArgs {
        RefreshArgs {
            release: None,
            even_if_games,
            force,
        }
    }

    const OK: RefreshStatus = RefreshStatus::Succeeded;

    #[rstest]
    #[case::nothing_new(false, Some(OK), Some("v1"), None, false, false, Plan::Unchanged)]
    #[case::unchanged_beats_games(
        false,
        Some(OK),
        Some("v1"),
        Some(2),
        false,
        false,
        Plan::Unchanged
    )]
    #[case::downloaded(true, Some(OK), Some("v1"), None, false, false, Plan::Load)]
    #[case::first_run(false, None, Some("v1"), None, false, false, Plan::Load)]
    #[case::after_a_deferral(
        false,
        Some(RefreshStatus::Deferred),
        Some("v1"),
        None,
        false,
        false,
        Plan::Load
    )]
    #[case::after_a_failure(
        false,
        Some(RefreshStatus::Failed),
        Some("v1"),
        None,
        false,
        false,
        Plan::Load
    )]
    #[case::new_release(false, Some(OK), Some("v0"), None, false, false, Plan::Load)]
    #[case::forced(false, Some(OK), Some("v1"), None, true, false, Plan::Load)]
    #[case::games_defer(true, Some(OK), Some("v1"), Some(2), false, false, Plan::Defer(2))]
    #[case::no_game_running(true, Some(OK), Some("v1"), Some(0), false, false, Plan::Load)]
    #[case::even_if_games(true, Some(OK), Some("v1"), Some(1), false, true, Plan::Load)]
    fn the_plan_follows_changes_releases_and_games(
        #[case] changed: bool,
        #[case] last: Option<RefreshStatus>,
        #[case] last_release: Option<&str>,
        #[case] running: Option<u64>,
        #[case] force: bool,
        #[case] even_if_games: bool,
        #[case] expected: Plan,
    ) {
        let fetched = Fetched {
            changed,
            steps: Vec::new(),
        };
        let last = last.map(|status| run(status, last_release));
        assert_eq!(
            plan(
                &fetched,
                last.as_ref(),
                Some("v1"),
                running,
                &args(force, even_if_games)
            ),
            expected
        );
    }

    #[test]
    fn the_summary_lists_the_steps() {
        let report = RefreshReport {
            new_card_count: 3,
            steps: vec!["fetch scryfall: up to date".to_owned()],
            ..RefreshReport::default()
        };
        assert_eq!(
            summary(&report, RefreshStatus::Deferred),
            "refresh: deferred, 3 new cards, 0 new precons\n  fetch scryfall: up to date"
        );
    }
}
