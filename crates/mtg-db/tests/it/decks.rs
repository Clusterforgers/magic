//! Goal: stored decks read back exactly as saved (lines in order, printings, notes, the stored check, UTC timestamps), one owner never sees, replaces or deletes another's deck, the cap on decks per owner holds under racing creations, lines cascade with their deck, a removed printing leaves the card, and the database refuses what the server would not send.

use crate::common;

use mtg_db::{CreateDeckError, DbError, DeckDraft, DeckId, DeckLine, DeckZone, StoredDeck};
use mtg_import::ids::PrintingId;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use common::{oracle_of, record, seeded};

fn owner(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn line(zone: DeckZone, name: &str, count: u32) -> DeckLine {
    DeckLine {
        zone,
        oracle_id: oracle_of(name),
        printing_id: None,
        count,
    }
}

fn draft(name: &str, lines: Vec<DeckLine>) -> DeckDraft {
    DeckDraft {
        name: name.to_owned(),
        format: "commander".to_owned(),
        notes: String::new(),
        lines,
        check: None,
    }
}

fn check(tag: &str) -> serde_json::Value {
    json!({"deck_hash": tag, "coverage_hash": "c", "checked_at": "2026-10-19T12:00:00Z", "summary": {}})
}

async fn make(pool: &PgPool, who: u128, name: &str) -> StoredDeck {
    seeded(pool.clone())
        .await
        .decks()
        .create(owner(who), &draft(name, vec![]), 100)
        .await
        .unwrap_or_else(|e| panic!("the deck is stored: {e}"))
}

#[sqlx::test]
async fn a_deck_reads_back_exactly_as_it_was_saved(pool: PgPool) {
    let database = seeded(pool).await;
    let repo = database.decks();
    let bolt = record("Lightning Bolt");
    let saved = DeckDraft {
        name: "Burn, with ünïcode".into(),
        format: "modern".into(),
        notes: "Notes\nover two lines".into(),
        lines: vec![
            line(DeckZone::Main, "Mountain", 20),
            DeckLine {
                printing_id: Some(bolt.printing_id.clone()),
                ..line(DeckZone::Main, "Lightning Bolt", 4)
            },
            line(DeckZone::Sideboard, "Shock", 2),
            line(DeckZone::Commander, "Atraxa, Praetors' Voice", 1),
            line(DeckZone::Companion, "Sol Ring", 1),
            // The same card twice in a zone, as two printings would be.
            line(DeckZone::Main, "Mountain", 3),
        ],
        check: Some(check("abc")),
    };

    let stored = repo.create(owner(1), &saved, 10).await.unwrap();
    assert_eq!(
        stored.lines, saved.lines,
        "lines keep their order and their repeats"
    );
    assert_eq!(
        (
            stored.name.as_str(),
            stored.format.as_str(),
            stored.notes.as_str()
        ),
        ("Burn, with ünïcode", "modern", "Notes\nover two lines")
    );
    assert_eq!(stored.check, saved.check);
    for stamp in [&stored.created_at, &stored.updated_at] {
        assert_eq!(stamp.len(), 20, "{stamp}");
        assert!(
            stamp.ends_with('Z') && stamp.as_bytes()[10] == b'T',
            "{stamp}"
        );
    }

    assert_eq!(
        repo.get(owner(1), stored.id).await.unwrap(),
        Some(stored.clone())
    );
    assert_eq!(repo.list(owner(1)).await.unwrap(), vec![stored]);
    assert_eq!(repo.count(owner(1)).await.unwrap(), 1);
}

#[sqlx::test]
async fn an_empty_draft_is_a_deck(pool: PgPool) {
    let stored = make(&pool, 1, "Empty").await;
    assert_eq!(stored.lines, Vec::new());
    assert_eq!(stored.check, None);
    assert_eq!(stored.notes, "");
}

#[sqlx::test]
async fn one_owner_never_touches_anothers_deck(pool: PgPool) {
    let database = seeded(pool).await;
    let repo = database.decks();
    let theirs = repo
        .create(
            owner(1),
            &draft("Theirs", vec![line(DeckZone::Main, "Shock", 4)]),
            10,
        )
        .await
        .unwrap();
    let mine = repo
        .create(owner(2), &draft("Mine", vec![]), 10)
        .await
        .unwrap();

    assert_eq!(repo.get(owner(2), theirs.id).await.unwrap(), None);
    assert_eq!(repo.list(owner(2)).await.unwrap(), vec![mine.clone()]);
    let attempt = draft("Taken over", vec![line(DeckZone::Main, "Sol Ring", 1)]);
    assert_eq!(
        repo.replace(owner(2), theirs.id, &attempt).await.unwrap(),
        None
    );
    assert!(!repo.delete(owner(2), theirs.id).await.unwrap());
    assert_eq!(
        repo.get(owner(1), theirs.id).await.unwrap(),
        Some(theirs.clone()),
        "nothing of theirs changed"
    );
    // An unknown id, a deck of another owner and a nil owner look alike.
    assert_eq!(
        repo.get(owner(2), DeckId(Uuid::from_u128(99)))
            .await
            .unwrap(),
        None
    );
    assert_eq!(repo.get(owner(0), mine.id).await.unwrap(), None);
    assert!(repo.delete(owner(2), mine.id).await.unwrap());
    assert_eq!(repo.count(owner(2)).await.unwrap(), 0);
}

#[sqlx::test]
async fn the_cap_counts_per_owner_and_frees_up_with_a_delete(pool: PgPool) {
    let database = seeded(pool).await;
    let repo = database.decks();
    let mut first = None;
    for n in 0..3 {
        let deck = repo
            .create(owner(1), &draft(&format!("Deck {n}"), vec![]), 3)
            .await
            .unwrap();
        first.get_or_insert(deck.id);
    }
    let over = repo
        .create(owner(1), &draft("One too many", vec![]), 3)
        .await;
    assert!(
        matches!(over, Err(CreateDeckError::LimitReached { max: 3 })),
        "{over:?}"
    );
    assert_eq!(repo.count(owner(1)).await.unwrap(), 3);

    repo.create(owner(2), &draft("Another owner", vec![]), 3)
        .await
        .unwrap();
    assert!(repo.delete(owner(1), first.unwrap()).await.unwrap());
    repo.create(owner(1), &draft("Room again", vec![]), 3)
        .await
        .unwrap();
    assert_eq!(repo.count(owner(1)).await.unwrap(), 3);
}

#[sqlx::test]
async fn racing_creations_cannot_exceed_the_cap(pool: PgPool) {
    let database = seeded(pool).await;
    let attempts: Vec<_> = (0..12)
        .map(|n| {
            let repo = database.decks();
            tokio::spawn(async move {
                repo.create(owner(1), &draft(&format!("Racer {n}"), vec![]), 4)
                    .await
            })
        })
        .collect();
    let mut created = 0;
    let mut refused = 0;
    for attempt in attempts {
        match attempt.await.unwrap() {
            Ok(_) => created += 1,
            Err(CreateDeckError::LimitReached { max: 4 }) => refused += 1,
            Err(other) => panic!("{other}"),
        }
    }
    assert_eq!((created, refused), (4, 8));
    assert_eq!(database.decks().count(owner(1)).await.unwrap(), 4);
}

#[sqlx::test]
async fn replacing_swaps_everything_in_one_step_and_moves_the_deck_up(pool: PgPool) {
    let database = seeded(pool).await;
    let repo = database.decks();
    let older = repo
        .create(
            owner(1),
            &draft("Older", vec![line(DeckZone::Main, "Shock", 4)]),
            10,
        )
        .await
        .unwrap();
    let newer = repo
        .create(owner(1), &draft("Newer", vec![]), 10)
        .await
        .unwrap();
    assert_eq!(
        repo.list(owner(1))
            .await
            .unwrap()
            .iter()
            .map(|d| d.id)
            .collect::<Vec<_>>(),
        [newer.id, older.id],
        "the most recently saved deck comes first"
    );

    let replacement = DeckDraft {
        name: "Renamed".into(),
        format: "freeform".into(),
        notes: "now with notes".into(),
        lines: vec![
            line(DeckZone::Sideboard, "Sol Ring", 1),
            line(DeckZone::Main, "Island", 30),
            line(DeckZone::Main, "Lightning Bolt", 3),
        ],
        check: Some(check("second")),
    };
    let stored = repo
        .replace(owner(1), older.id, &replacement)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.id, older.id);
    assert_eq!(
        stored.lines, replacement.lines,
        "the old lines are gone, the new keep their order"
    );
    assert_eq!(stored.check, replacement.check);
    assert_eq!(stored.created_at, older.created_at);
    assert_eq!(
        repo.list(owner(1))
            .await
            .unwrap()
            .iter()
            .map(|d| d.id)
            .collect::<Vec<_>>(),
        [older.id, newer.id]
    );

    let cleared = DeckDraft {
        check: None,
        lines: vec![],
        ..replacement
    };
    let stored = repo
        .replace(owner(1), older.id, &cleared)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (stored.lines.len(), stored.check),
        (0, None),
        "a save may take the check away"
    );
}

#[sqlx::test]
async fn lines_go_with_their_deck_and_a_removed_printing_leaves_the_card(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let repo = database.decks();
    let bolt = record("Lightning Bolt");
    let stored = repo
        .create(
            owner(1),
            &draft(
                "Art",
                vec![DeckLine {
                    printing_id: Some(bolt.printing_id.clone()),
                    ..line(DeckZone::Main, "Lightning Bolt", 4)
                }],
            ),
            10,
        )
        .await
        .unwrap();

    sqlx::raw_sql("DELETE FROM printings")
        .execute(&pool)
        .await
        .unwrap();
    let after = repo.get(owner(1), stored.id).await.unwrap().unwrap();
    assert_eq!(after.lines, vec![line(DeckZone::Main, "Lightning Bolt", 4)]);

    assert!(repo.delete(owner(1), stored.id).await.unwrap());
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM deck_cards")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(left, 0);
}

#[sqlx::test]
async fn the_database_refuses_what_the_server_would_never_send(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let repo = database.decks();
    let ghost = DeckLine {
        oracle_id: mtg_import::ids::OracleId(Uuid::from_u128(5).to_string()),
        ..line(DeckZone::Main, "Shock", 1)
    };
    let long = |n: usize| "x".repeat(n);
    let bad_drafts = [
        ("an unknown card", draft("Ghosts", vec![ghost])),
        (
            "a zero count",
            draft("None", vec![line(DeckZone::Main, "Shock", 0)]),
        ),
        (
            "a huge count",
            draft("Many", vec![line(DeckZone::Main, "Shock", 1000)]),
        ),
        (
            "a printing that does not exist",
            draft(
                "Art",
                vec![DeckLine {
                    printing_id: Some(PrintingId(Uuid::from_u128(6).to_string())),
                    ..line(DeckZone::Main, "Shock", 1)
                }],
            ),
        ),
        ("an empty name", draft("", vec![])),
        ("a long name", draft(&long(201), vec![])),
        (
            "a shouting format",
            DeckDraft {
                format: "Commander".into(),
                ..draft("F", vec![])
            },
        ),
        (
            "an empty format",
            DeckDraft {
                format: String::new(),
                ..draft("F", vec![])
            },
        ),
        (
            "long notes",
            DeckDraft {
                notes: long(20_001),
                ..draft("N", vec![])
            },
        ),
        (
            "a check that is no object",
            DeckDraft {
                check: Some(json!([1])),
                ..draft("C", vec![])
            },
        ),
    ];
    for (what, bad) in bad_drafts {
        let created = repo.create(owner(1), &bad, 10).await;
        assert!(
            matches!(created, Err(CreateDeckError::Database(DbError::Query(_)))),
            "{what}: {created:?}"
        );
        assert_eq!(
            repo.count(owner(1)).await.unwrap(),
            0,
            "{what}: nothing was stored"
        );
    }

    // A failed replacement leaves the deck as it was, lines included.
    let kept = repo
        .create(
            owner(1),
            &draft("Kept", vec![line(DeckZone::Main, "Shock", 4)]),
            10,
        )
        .await
        .unwrap();
    let broken = draft(
        "Broken",
        vec![
            line(DeckZone::Main, "Sol Ring", 2),
            line(DeckZone::Main, "Shock", 0),
        ],
    );
    assert!(repo.replace(owner(1), kept.id, &broken).await.is_err());
    assert_eq!(repo.get(owner(1), kept.id).await.unwrap(), Some(kept));
}

#[sqlx::test]
async fn a_name_of_exactly_the_limit_is_kept(pool: PgPool) {
    let database = seeded(pool).await;
    let name = "n".repeat(200);
    let notes = "z".repeat(20_000);
    let stored = database
        .decks()
        .create(
            owner(1),
            &DeckDraft {
                notes: notes.clone(),
                ..draft(&name, vec![])
            },
            10,
        )
        .await
        .unwrap();
    assert_eq!((stored.name, stored.notes), (name, notes));
}
