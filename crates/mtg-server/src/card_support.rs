//! The card support endpoints for signed-in accounts: `GET /api/card-support` answers the newest
//! coverage snapshot, the trend over earlier ones, the newest daily refresh and the feed built from
//! both; `GET /api/card-support/missing` answers the cards that do not play automatically. The
//! rows come from `mtg-cli refresh` and `mtg-cli coverage --all --snapshot`; a server without a
//! database answers 503 `cards_unavailable`.

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use mtg_db::{CardFeedRepo, MissingCard, RefreshRun, RefreshStatus, Snapshot};
use mtg_protocol::rest::{
    CardSupport, DeckErrorCode, FeedEvent, MAX_FEED_EVENTS, MAX_FEED_NAMES, MAX_SUPPORT_HISTORY,
    MissingCardView, MissingCards, MissingState, RefreshCheck, RefreshState, SupportBucket,
    SupportPoint, SupportSnapshot, SupportStates,
};

use crate::api::ApiError;
use crate::card_api::failed;
use crate::server::AppState;

/// How many refresh runs the feed reads.
const FEED_RUNS: u32 = 60;

/// The 503 of a server without a database, then the 401 of a request without a session.
async fn repo(app: &AppState, headers: &HeaderMap) -> Result<CardFeedRepo, ApiError> {
    let cards = app.cards().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            DeckErrorCode::CardsUnavailable.as_str(),
            "this server runs without a database, so it counts no cards",
        )
    })?;
    app.auth().require_user(headers).await?;
    Ok(cards.database().card_feed())
}

/// `GET /api/card-support`.
pub(crate) async fn overview(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<CardSupport>, ApiError> {
    let feed = repo(&app, &headers).await?;
    let snapshots = feed
        .snapshots(MAX_SUPPORT_HISTORY)
        .await
        .map_err(|error| failed(&error))?;
    let runs = feed
        .refreshes(FEED_RUNS)
        .await
        .map_err(|error| failed(&error))?;
    Ok(Json(support_of(&snapshots, &runs)))
}

/// `GET /api/card-support/missing`.
pub(crate) async fn missing(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<MissingCards>, ApiError> {
    let feed = repo(&app, &headers).await?;
    let latest = feed
        .latest_missing()
        .await
        .map_err(|error| failed(&error))?;
    Ok(Json(match latest {
        Some((id, cards)) => MissingCards {
            snapshot_id: Some(id_of(id)),
            cards: cards.into_iter().map(missing_view).collect(),
        },
        None => MissingCards {
            snapshot_id: None,
            cards: Vec::new(),
        },
    }))
}

fn id_of(id: i64) -> u32 {
    u32::try_from(id).unwrap_or(u32::MAX)
}

fn missing_view(card: MissingCard) -> MissingCardView {
    let state = match card.state.as_str() {
        "waiting" => MissingState::Waiting,
        "partial" => MissingState::Partial,
        "manual" => MissingState::Manual,
        _ => MissingState::Other,
    };
    MissingCardView {
        oracle_id: card.oracle_id,
        name: card.name,
        state,
        blocker: card.blocker,
        detail: card.detail,
        blockers: card.blockers,
        rank: card.rank,
    }
}

/// The answer from `snapshots` and `runs`, both the newest first.
pub(crate) fn support_of(snapshots: &[Snapshot], runs: &[RefreshRun]) -> CardSupport {
    CardSupport {
        snapshot: snapshots.first().map(snapshot_view),
        history: snapshots
            .iter()
            .rev()
            .map(|snapshot| SupportPoint {
                taken_at: snapshot.taken_at.clone(),
                release: snapshot.release.clone(),
                cards: snapshot.cards,
                automatic: snapshot.automatic,
            })
            .collect(),
        last_check: runs.first().map(check_view),
        feed: feed_of(snapshots, runs),
    }
}

fn snapshot_view(snapshot: &Snapshot) -> SupportSnapshot {
    let states = snapshot.states;
    SupportSnapshot {
        id: id_of(snapshot.id),
        taken_at: snapshot.taken_at.clone(),
        release: snapshot.release.clone(),
        cards: snapshot.cards,
        automatic: snapshot.automatic,
        states: SupportStates {
            verified: states.verified,
            read: states.read,
            unverified: states.unverified,
            waiting: states.waiting,
            partial: states.partial,
            manual: states.manual,
            other: states.other,
        },
        buckets: snapshot
            .buckets
            .iter()
            .map(|bucket| SupportBucket {
                label: bucket.label.clone(),
                cards: bucket.cards,
                verified: bucket.verified,
                automatic: bucket.automatic,
            })
            .collect(),
        missing: snapshot.missing_count,
    }
}

fn check_view(run: &RefreshRun) -> RefreshCheck {
    RefreshCheck {
        started_at: run.started_at.clone(),
        finished_at: run.finished_at.clone(),
        state: match run.status {
            RefreshStatus::Running => RefreshState::Running,
            RefreshStatus::Succeeded => RefreshState::Succeeded,
            RefreshStatus::Deferred => RefreshState::Deferred,
            RefreshStatus::Failed => RefreshState::Failed,
        },
        scryfall_updated_at: run.report.scryfall_updated_at.clone(),
        mtgjson_build: run.report.mtgjson_build.clone(),
    }
}

fn names(list: &[String]) -> Vec<String> {
    list.iter().take(MAX_FEED_NAMES).cloned().collect()
}

/// The events of a refresh run at its start: its new cards, its new precons per set, or its
/// failure. A first load is no news.
fn run_events(run: &RefreshRun) -> Vec<FeedEvent> {
    // The start, so the snapshot a run takes reads above the cards it found.
    let at = run.started_at.clone();
    let report = &run.report;
    if run.status == RefreshStatus::Failed {
        return vec![FeedEvent::CheckFailed {
            at,
            failures: report.failures.clone(),
        }];
    }
    if report.first_load {
        return Vec::new();
    }
    let mut events = Vec::new();
    let mut sets: Vec<(&str, Option<&String>, Vec<String>)> = Vec::new();
    for precon in &report.new_precons {
        match sets.iter_mut().find(|(code, _, _)| *code == precon.code) {
            Some((_, _, decks)) => decks.push(precon.name.clone()),
            None => sets.push((
                &precon.code,
                precon.set_name.as_ref(),
                vec![precon.name.clone()],
            )),
        }
    }
    events.extend(
        sets.into_iter()
            .map(|(code, set_name, decks)| FeedEvent::NewPrecons {
                at: at.clone(),
                code: code.to_owned(),
                set_name: set_name.cloned(),
                decks,
            }),
    );
    if report.new_card_count > 0 {
        events.push(FeedEvent::NewCards {
            at,
            count: report.new_card_count,
            names: names(&report.new_cards),
        });
    }
    events
}

/// The event of a snapshot: the cards that play automatically since the one before, or the new
/// release that took it. The oldest stored snapshot has no event, since nothing came before it.
fn snapshot_event(snapshot: &Snapshot, before: &Snapshot) -> Option<FeedEvent> {
    let new_release = snapshot.release.is_some() && snapshot.release != before.release;
    (snapshot.newly_supported_count > 0 || new_release).then(|| FeedEvent::NowAutomatic {
        at: snapshot.taken_at.clone(),
        release: snapshot.release.clone(),
        new_release,
        count: snapshot.newly_supported_count,
        names: names(&snapshot.newly_supported),
    })
}

fn at_of(event: &FeedEvent) -> &str {
    match event {
        FeedEvent::NewCards { at, .. }
        | FeedEvent::NewPrecons { at, .. }
        | FeedEvent::NowAutomatic { at, .. }
        | FeedEvent::CheckFailed { at, .. } => at,
    }
}

/// Every event of `snapshots` and `runs`, the newest first, at most [`MAX_FEED_EVENTS`].
fn feed_of(snapshots: &[Snapshot], runs: &[RefreshRun]) -> Vec<FeedEvent> {
    let mut events: Vec<FeedEvent> = runs.iter().flat_map(run_events).collect();
    events.extend(
        snapshots
            .windows(2)
            .filter_map(|pair| snapshot_event(&pair[0], &pair[1])),
    );
    // Times are UTC in one fixed form, so the text orders them; the sort is stable for equal times.
    events.sort_by(|a, b| at_of(b).cmp(at_of(a)));
    events.truncate(MAX_FEED_EVENTS);
    events
}

#[cfg(test)]
mod tests {
    //! Goal: the feed turns refresh runs and snapshots into events, newest first: precons grouped by set, new cards counted, failures said, a first load and an unchanged release left out, and the names of an event capped; the trend runs oldest first.
    #![allow(clippy::unwrap_used)]

    use mtg_db::{NewPrecon, RefreshId, RefreshReport, StateCounts};
    use rstest::rstest;

    use super::*;

    fn snapshot(id: i64, at: &str, release: Option<&str>, newly: &[&str]) -> Snapshot {
        Snapshot {
            id,
            taken_at: at.to_owned(),
            release: release.map(str::to_owned),
            cards: 100,
            automatic: 90,
            states: StateCounts::default(),
            buckets: Vec::new(),
            missing_count: 10,
            newly_supported_count: u32::try_from(newly.len()).unwrap(),
            newly_supported: newly.iter().map(|n| (*n).to_owned()).collect(),
        }
    }

    fn run(at: &str, status: RefreshStatus, report: RefreshReport) -> RefreshRun {
        RefreshRun {
            id: RefreshId(1),
            started_at: at.to_owned(),
            finished_at: Some(at.to_owned()),
            status,
            release: None,
            report,
        }
    }

    fn precon(code: &str, name: &str) -> NewPrecon {
        NewPrecon {
            code: code.to_owned(),
            name: name.to_owned(),
            set_name: Some(format!("Set {code}")),
        }
    }

    fn kinds(events: &[FeedEvent]) -> Vec<String> {
        events
            .iter()
            .map(|event| match event {
                FeedEvent::NewCards { count, .. } => format!("cards {count}"),
                FeedEvent::NewPrecons { code, decks, .. } => format!("{code} {}", decks.len()),
                FeedEvent::NowAutomatic {
                    release,
                    count,
                    new_release,
                    ..
                } => format!(
                    "{} {count}{}",
                    release.clone().unwrap_or_default(),
                    if *new_release { " new" } else { "" }
                ),
                FeedEvent::CheckFailed { failures, .. } => format!("failed {}", failures.len()),
            })
            .collect()
    }

    #[test]
    fn the_feed_merges_runs_and_snapshots_newest_first() {
        let runs = vec![
            run(
                "2026-11-03T04:00:00Z",
                RefreshStatus::Failed,
                RefreshReport {
                    failures: vec!["fetch: offline".into()],
                    ..RefreshReport::default()
                },
            ),
            run(
                "2026-11-02T04:00:00Z",
                RefreshStatus::Succeeded,
                RefreshReport {
                    new_card_count: 289,
                    new_cards: vec!["A".into()],
                    new_precons: vec![
                        precon("STC", "One"),
                        precon("STC", "Two"),
                        precon("XYZ", "Three"),
                    ],
                    ..RefreshReport::default()
                },
            ),
            run(
                "2026-11-01T04:00:00Z",
                RefreshStatus::Succeeded,
                RefreshReport {
                    first_load: true,
                    new_card_count: 30_000,
                    ..RefreshReport::default()
                },
            ),
        ];
        let snapshots = vec![
            snapshot(3, "2026-11-02T05:00:00Z", Some("v2"), &["Bolt"]),
            snapshot(2, "2026-11-01T06:00:00Z", Some("v1"), &[]),
            snapshot(1, "2026-11-01T05:00:00Z", Some("v1"), &["Old"]),
        ];
        let support = support_of(&snapshots, &runs);
        assert_eq!(
            kinds(&support.feed),
            vec!["failed 1", "v2 1 new", "STC 2", "XYZ 1", "cards 289"]
        );
        assert_eq!(support.snapshot.unwrap().id, 3);
        let trend: Vec<&str> = support
            .history
            .iter()
            .map(|p| p.taken_at.as_str())
            .collect();
        assert_eq!(
            trend,
            vec![
                "2026-11-01T05:00:00Z",
                "2026-11-01T06:00:00Z",
                "2026-11-02T05:00:00Z"
            ]
        );
        assert_eq!(support.last_check.unwrap().state, RefreshState::Failed);
    }

    #[rstest]
    #[case::more_cards(Some("v1"), Some("v1"), 3, Some("v1 3"))]
    #[case::new_release(Some("v2"), Some("v1"), 0, Some("v2 0 new"))]
    #[case::nothing(Some("v1"), Some("v1"), 0, None)]
    #[case::unknown_release(None, Some("v1"), 0, None)]
    fn a_snapshot_is_news_when_cards_joined_or_the_release_changed(
        #[case] now: Option<&str>,
        #[case] before: Option<&str>,
        #[case] newly: usize,
        #[case] expected: Option<&str>,
    ) {
        let names: Vec<String> = (0..newly).map(|n| format!("Card {n}")).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let event = snapshot_event(
            &snapshot(2, "2026-11-02T00:00:00Z", now, &refs),
            &snapshot(1, "2026-11-01T00:00:00Z", before, &[]),
        );
        assert_eq!(
            event.map(|e| kinds(&[e]).remove(0)),
            expected.map(str::to_owned)
        );
    }

    #[test]
    fn an_event_lists_a_capped_number_of_names_and_the_whole_count() {
        let many: Vec<String> = (0..MAX_FEED_NAMES + 9)
            .map(|n| format!("Card {n}"))
            .collect();
        let refs: Vec<&str> = many.iter().map(String::as_str).collect();
        let event = snapshot_event(
            &snapshot(2, "2026-11-02T00:00:00Z", Some("v1"), &refs),
            &snapshot(1, "2026-11-01T00:00:00Z", Some("v1"), &[]),
        )
        .unwrap();
        let FeedEvent::NowAutomatic { count, names, .. } = event else {
            panic!("a newly automatic event")
        };
        assert_eq!(
            (count as usize, names.len()),
            (MAX_FEED_NAMES + 9, MAX_FEED_NAMES)
        );
    }

    #[test]
    fn nothing_stored_is_an_empty_answer() {
        let support = support_of(&[], &[]);
        assert_eq!(support.snapshot, None);
        assert_eq!(support.last_check, None);
        assert_eq!((support.history.len(), support.feed.len()), (0, 0));
    }
}
