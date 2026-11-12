//! Goal: a deck line's art is set and cleared only by its owner and only to a printing of its own card, the art an account prefers is kept per card and owner, and a card's printings list newest first with the set name, frame, finishes and full-art mark an import stores.
#![allow(clippy::unwrap_used)]

use crate::common;

use mtg_db::{DeckDraft, DeckId, DeckLine, DeckZone, PreferredPrintingRepo, PrintingChoice};
use mtg_import::ids::{OracleId, PrintingId};
use mtg_import::scryfall::CardRecord;
use sqlx::PgPool;
use uuid::Uuid;

use common::{another_printing, oracle_of, record, repo, seeded};

const OWNER: u128 = 1;
const STRANGER: u128 = 2;

/// Lightning Bolt's fixture printing and two more: one older in an old frame, one newer, full art and foil.
fn bolt_printings() -> Vec<CardRecord> {
    let bolt = record("Lightning Bolt");
    let mut old = another_printing(&bolt, 11, "lea", Some("1993-08-05"));
    old.set_name = Some("Limited Edition Alpha".into());
    old.frame = Some("1993".into());
    old.finishes = vec!["nonfoil".into()];
    let mut new = another_printing(&bolt, 12, "sld", Some("2030-01-01"));
    new.set_name = Some("Secret Lair Drop".into());
    new.frame = Some("2015".into());
    new.finishes = vec!["nonfoil".into(), "foil".into()];
    new.full_art = true;
    vec![old, new]
}

/// The fixture cards and Bolt's extra printings stored, and a deck of the owner with Bolt in main and sideboard.
async fn prepared(pool: &PgPool) -> (mtg_db::Database, DeckId) {
    let database = seeded(pool.clone()).await;
    database
        .importer()
        .import_printings(bolt_printings().into_iter().map(Ok))
        .await
        .unwrap();
    let line = |zone, name: &str| DeckLine {
        zone,
        oracle_id: oracle_of(name),
        printing_id: None,
        count: 1,
        by_hand: false,
    };
    let draft = DeckDraft {
        name: "Burn".into(),
        format: "modern".into(),
        notes: String::new(),
        lines: vec![
            line(DeckZone::Main, "Lightning Bolt"),
            line(DeckZone::Sideboard, "Lightning Bolt"),
            line(DeckZone::Main, "Shock"),
        ],
        check: None,
    };
    let stored = database
        .decks()
        .create(Uuid::from_u128(OWNER), &draft, 10)
        .await
        .unwrap();
    (database, stored.id)
}

fn printing(n: u128) -> PrintingId {
    PrintingId(Uuid::from_u128(n + 2_000_000).to_string())
}

#[sqlx::test]
async fn a_line_takes_only_a_printing_of_its_card_from_its_owner(pool: PgPool) {
    let (database, deck) = prepared(&pool).await;
    let repo = database.decks();
    let owner = Uuid::from_u128(OWNER);
    let cases = [
        (
            "own card",
            OWNER,
            DeckZone::Main,
            "Lightning Bolt",
            Some(printing(12)),
            PrintingChoice::Set,
        ),
        (
            "cleared",
            OWNER,
            DeckZone::Main,
            "Lightning Bolt",
            None,
            PrintingChoice::Set,
        ),
        (
            "another card's printing",
            OWNER,
            DeckZone::Main,
            "Shock",
            Some(printing(12)),
            PrintingChoice::NotOfCard,
        ),
        (
            "unknown printing",
            OWNER,
            DeckZone::Main,
            "Lightning Bolt",
            Some(printing(99)),
            PrintingChoice::NotOfCard,
        ),
        (
            "malformed printing",
            OWNER,
            DeckZone::Main,
            "Lightning Bolt",
            Some(PrintingId("x".into())),
            PrintingChoice::NotOfCard,
        ),
        (
            "zone without the card",
            OWNER,
            DeckZone::Commander,
            "Lightning Bolt",
            Some(printing(12)),
            PrintingChoice::NoLine,
        ),
        (
            "card not in the deck",
            OWNER,
            DeckZone::Main,
            "Sol Ring",
            None,
            PrintingChoice::NoLine,
        ),
        (
            "another owner",
            STRANGER,
            DeckZone::Main,
            "Lightning Bolt",
            Some(printing(12)),
            PrintingChoice::NoDeck,
        ),
    ];
    for (case, who, zone, card, chosen, outcome) in cases {
        let got = repo
            .set_printing(
                Uuid::from_u128(who),
                deck,
                zone,
                &oracle_of(card),
                chosen.as_ref(),
            )
            .await
            .unwrap();
        assert_eq!(got, outcome, "{case}");
        let stored = repo.get(owner, deck).await.unwrap().unwrap();
        let art: Vec<Option<PrintingId>> =
            stored.lines.iter().map(|l| l.printing_id.clone()).collect();
        let main = if outcome == PrintingChoice::Set {
            chosen
        } else {
            None
        };
        assert_eq!(
            art,
            vec![main, None, None],
            "{case}: only the main line of the card changes, and only on success"
        );
        repo.set_printing(
            owner,
            deck,
            DeckZone::Main,
            &oracle_of("Lightning Bolt"),
            None,
        )
        .await
        .unwrap();
    }
}

#[sqlx::test]
async fn a_cleared_line_shows_the_default_again(pool: PgPool) {
    let (database, deck) = prepared(&pool).await;
    let repo = database.decks();
    let owner = Uuid::from_u128(OWNER);
    let bolt = oracle_of("Lightning Bolt");
    let set = |p| repo.set_printing(owner, deck, DeckZone::Sideboard, &bolt, p);
    let old = printing(11);
    assert_eq!(set(Some(&old)).await.unwrap(), PrintingChoice::Set);
    assert_eq!(
        repo.get(owner, deck).await.unwrap().unwrap().lines[1].printing_id,
        Some(printing(11))
    );
    assert_eq!(set(None).await.unwrap(), PrintingChoice::Set);
    assert_eq!(
        repo.get(owner, deck).await.unwrap().unwrap().lines[1].printing_id,
        None
    );
}

#[sqlx::test]
async fn preferred_art_is_kept_per_owner_and_card(pool: PgPool) {
    let (database, _) = prepared(&pool).await;
    let prefs = database.preferred_printings();
    let (owner, stranger) = (Uuid::from_u128(OWNER), Uuid::from_u128(STRANGER));
    let bolt = oracle_of("Lightning Bolt");

    assert!(prefs.set(owner, &bolt, &printing(11)).await.unwrap());
    assert!(
        prefs.set(owner, &bolt, &printing(12)).await.unwrap(),
        "a second choice replaces the first"
    );
    assert!(
        !prefs
            .set(owner, &oracle_of("Shock"), &printing(12))
            .await
            .unwrap(),
        "another card's printing"
    );
    assert!(
        !prefs.set(owner, &bolt, &printing(99)).await.unwrap(),
        "an unknown printing"
    );
    assert!(
        !prefs
            .set(owner, &OracleId("nope".into()), &printing(12))
            .await
            .unwrap()
    );

    let mine = prefs.all(owner).await.unwrap();
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[&bolt], printing(12));
    assert!(
        prefs.all(stranger).await.unwrap().is_empty(),
        "one account never sees another's"
    );
    assert!(!prefs.clear(stranger, &bolt).await.unwrap());

    let mut tx = pool.begin().await.unwrap();
    assert_eq!(
        PreferredPrintingRepo::delete_all_of(&mut tx, owner)
            .await
            .unwrap(),
        1
    );
    tx.commit().await.unwrap();
    assert!(prefs.all(owner).await.unwrap().is_empty());
    assert!(!prefs.clear(owner, &bolt).await.unwrap());
}

#[sqlx::test]
async fn a_cards_printings_list_newest_first_with_their_look(pool: PgPool) {
    prepared(&pool).await;
    let printings = repo(&pool)
        .printings_of(&oracle_of("Lightning Bolt"))
        .await
        .unwrap();
    let sets: Vec<&str> = printings.iter().map(|p| p.set.as_str()).collect();
    assert_eq!(sets.first(), Some(&"sld"));
    assert_eq!(sets.last(), Some(&"lea"));
    let newest = &printings[0];
    assert_eq!(newest.set_name.as_deref(), Some("Secret Lair Drop"));
    assert_eq!(newest.frame.as_deref(), Some("2015"));
    assert_eq!(newest.finishes, ["nonfoil", "foil"]);
    assert!(newest.full_art);
    let oldest = printings.last().unwrap();
    assert_eq!(oldest.frame.as_deref(), Some("1993"));
    assert!(!oldest.full_art);
    assert!(
        repo(&pool)
            .printings_of(&OracleId("nope".into()))
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        repo(&pool)
            .printings_of(&OracleId(Uuid::from_u128(5).to_string()))
            .await
            .unwrap()
            .is_empty()
    );
}
