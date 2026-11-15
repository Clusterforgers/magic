//! Goal: a deck sits in any number of its owner's groups, joins and leaves one group without touching the others, asking twice changes nothing, one owner never files into or out of another's groups or decks, deleting a group or a deck removes only its own memberships, and the migration that allowed several groups keeps every membership already stored.

use crate::common;

use mtg_db::{DeckDraft, DeckGroupRepo, DeckId, Filing};
use sqlx::PgPool;
use uuid::Uuid;

fn owner(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

async fn deck(pool: &PgPool, who: u128, name: &str) -> DeckId {
    let draft = DeckDraft {
        name: name.to_owned(),
        format: "commander".to_owned(),
        notes: String::new(),
        lines: vec![],
        check: None,
    };
    let stored = common::db(pool.clone())
        .decks()
        .create(owner(who), &draft, 100)
        .await
        .unwrap_or_else(|e| panic!("the deck is stored: {e}"));
    stored.id
}

async fn group(repo: &DeckGroupRepo, who: u128, name: &str) -> Uuid {
    repo.create(owner(who), name, 50)
        .await
        .unwrap_or_else(|e| panic!("the group is stored: {e}"))
        .id
}

/// The decks of each group of `who` in the owner's order, each list sorted.
async fn shelf(repo: &DeckGroupRepo, who: u128) -> Vec<Vec<DeckId>> {
    let mut shelf: Vec<Vec<DeckId>> = repo
        .list(owner(who))
        .await
        .unwrap_or_else(|e| panic!("the groups are listed: {e}"))
        .into_iter()
        .map(|group| group.decks)
        .collect();
    for decks in &mut shelf {
        decks.sort_by_key(|deck| deck.0);
    }
    shelf
}

fn sorted(mut decks: Vec<DeckId>) -> Vec<DeckId> {
    decks.sort_by_key(|deck| deck.0);
    decks
}

#[sqlx::test]
async fn a_deck_joins_several_groups_and_leaves_one_at_a_time(pool: PgPool) {
    let repo = common::db(pool.clone()).deck_groups();
    let (one, two) = (deck(&pool, 1, "One").await, deck(&pool, 1, "Two").await);
    let (fun, hard, dungeon) = (
        group(&repo, 1, "Fun").await,
        group(&repo, 1, "Hardcore").await,
        group(&repo, 1, "Dungeon").await,
    );

    for (deck, group) in [(one, fun), (one, hard), (two, hard)] {
        assert_eq!(
            repo.add_deck(owner(1), deck, group).await.unwrap(),
            Filing::Done
        );
    }
    assert_eq!(
        shelf(&repo, 1).await,
        [vec![one], sorted(vec![one, two]), vec![]]
    );

    // Leaving one group keeps the deck in the other.
    assert_eq!(
        repo.remove_deck(owner(1), one, hard).await.unwrap(),
        Filing::Done
    );
    assert_eq!(shelf(&repo, 1).await, [vec![one], vec![two], vec![]]);
    // The last group does not take the deck away from its owner.
    assert_eq!(
        repo.remove_deck(owner(1), one, fun).await.unwrap(),
        Filing::Done
    );
    assert_eq!(shelf(&repo, 1).await, [vec![], vec![two], vec![]]);
    let decks = common::db(pool.clone())
        .decks()
        .list(owner(1))
        .await
        .unwrap();
    assert_eq!(decks.len(), 2);

    // A deck can be in every group at once.
    for group in [fun, hard, dungeon] {
        repo.add_deck(owner(1), one, group).await.unwrap();
    }
    assert!(shelf(&repo, 1).await.iter().all(|g| g.contains(&one)));
}

#[sqlx::test]
async fn asking_twice_changes_nothing(pool: PgPool) {
    let repo = common::db(pool.clone()).deck_groups();
    let one = deck(&pool, 1, "One").await;
    let fun = group(&repo, 1, "Fun").await;

    // Taking a deck out of a group it is not in is fine.
    assert_eq!(
        repo.remove_deck(owner(1), one, fun).await.unwrap(),
        Filing::Done
    );
    for _ in 0..2 {
        assert_eq!(
            repo.add_deck(owner(1), one, fun).await.unwrap(),
            Filing::Done
        );
    }
    assert_eq!(shelf(&repo, 1).await, [vec![one]]);
    for _ in 0..2 {
        assert_eq!(
            repo.remove_deck(owner(1), one, fun).await.unwrap(),
            Filing::Done
        );
    }
    assert_eq!(shelf(&repo, 1).await, [Vec::<DeckId>::new()]);
}

#[sqlx::test]
async fn one_owner_never_files_into_or_out_of_anothers_groups_or_decks(pool: PgPool) {
    let repo = common::db(pool.clone()).deck_groups();
    let (anns_deck, bos_deck) = (deck(&pool, 1, "Ann's").await, deck(&pool, 2, "Bob's").await);
    let (anns_group, bos_group) = (group(&repo, 1, "Fun").await, group(&repo, 2, "Fun").await);
    repo.add_deck(owner(1), anns_deck, anns_group)
        .await
        .unwrap();
    repo.add_deck(owner(2), bos_deck, bos_group).await.unwrap();

    let stranger = DeckId(Uuid::from_u128(99));
    for (who, deck, group, expected) in [
        (2, anns_deck, bos_group, Filing::NoDeck),
        (2, bos_deck, anns_group, Filing::NoGroup),
        (2, anns_deck, anns_group, Filing::NoDeck),
        (2, stranger, bos_group, Filing::NoDeck),
        (2, bos_deck, Uuid::from_u128(98), Filing::NoGroup),
    ] {
        assert_eq!(
            repo.add_deck(owner(who), deck, group).await.unwrap(),
            expected,
            "add {deck:?} to {group}"
        );
        assert_eq!(
            repo.remove_deck(owner(who), deck, group).await.unwrap(),
            expected,
            "remove {deck:?} from {group}"
        );
    }
    assert_eq!(shelf(&repo, 1).await, [vec![anns_deck]]);
    assert_eq!(shelf(&repo, 2).await, [vec![bos_deck]]);
}

#[sqlx::test]
async fn deleting_a_group_or_a_deck_removes_only_its_own_memberships(pool: PgPool) {
    let repo = common::db(pool.clone()).deck_groups();
    let decks = common::db(pool.clone()).decks();
    let (one, two) = (deck(&pool, 1, "One").await, deck(&pool, 1, "Two").await);
    let (fun, hard) = (group(&repo, 1, "Fun").await, group(&repo, 1, "Hard").await);
    for (deck, group) in [(one, fun), (one, hard), (two, fun), (two, hard)] {
        repo.add_deck(owner(1), deck, group).await.unwrap();
    }

    assert!(repo.delete(owner(1), fun).await.unwrap());
    assert_eq!(shelf(&repo, 1).await, [sorted(vec![one, two])]);
    assert!(decks.delete(owner(1), two).await.unwrap());
    assert_eq!(shelf(&repo, 1).await, [vec![one]]);
}

#[sqlx::test]
async fn a_membership_is_unique_per_deck_and_group(pool: PgPool) {
    let repo = common::db(pool.clone()).deck_groups();
    let one = deck(&pool, 1, "One").await;
    let (fun, hard) = (group(&repo, 1, "Fun").await, group(&repo, 1, "Hard").await);
    let insert = "INSERT INTO deck_group_members (deck_id, group_id) VALUES ($1, $2)";

    sqlx::query(insert)
        .bind(one.0)
        .bind(fun)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(insert)
        .bind(one.0)
        .bind(hard)
        .execute(&pool)
        .await
        .unwrap();
    let again = sqlx::query(insert)
        .bind(one.0)
        .bind(fun)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(
        again
            .as_database_error()
            .is_some_and(sqlx::error::DatabaseError::is_unique_violation)
    );
}

#[sqlx::test(migrations = false)]
async fn the_many_groups_migration_keeps_every_membership(pool: PgPool) {
    const MANY_GROUPS: i64 = 20_261_115_000_028;
    let all = sqlx::migrate!("./migrations");
    let before: Vec<_> = all
        .iter()
        .filter(|migration| migration.version < MANY_GROUPS)
        .cloned()
        .collect();
    assert!(all.iter().any(|migration| migration.version == MANY_GROUPS));
    let old = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(before),
        ..sqlx::migrate::Migrator::DEFAULT
    };
    old.run(&pool).await.unwrap();

    // Memberships as the one-group rule stored them.
    let (ann, bob) = (owner(1), owner(2));
    let mut decks = Vec::new();
    for (who, name) in [(ann, "One"), (ann, "Two"), (ann, "Three"), (bob, "Bob's")] {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO decks (owner, name, format) VALUES ($1, $2, 'commander') RETURNING id",
        )
        .bind(who)
        .bind(name)
        .fetch_one(&pool)
        .await
        .unwrap();
        decks.push(id);
    }
    let mut groups = Vec::new();
    for (who, name, position) in [(ann, "Fun", 0), (ann, "Hard", 1), (bob, "Fun", 0)] {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO deck_groups (owner, name, position) VALUES ($1, $2, $3) RETURNING id",
        )
        .bind(who)
        .bind(name)
        .bind(position)
        .fetch_one(&pool)
        .await
        .unwrap();
        groups.push(id);
    }
    let held = [
        (decks[0], groups[0]),
        (decks[1], groups[0]),
        (decks[2], groups[1]),
        (decks[3], groups[2]),
    ];
    let insert = "INSERT INTO deck_group_members (deck_id, group_id) VALUES ($1, $2)";
    for (deck, group) in held {
        sqlx::query(insert)
            .bind(deck)
            .bind(group)
            .execute(&pool)
            .await
            .unwrap();
    }
    // The old table refuses a second group for a deck.
    assert!(
        sqlx::query(insert)
            .bind(decks[0])
            .bind(groups[1])
            .execute(&pool)
            .await
            .is_err()
    );

    all.run(&pool).await.unwrap();

    let kept: Vec<(Uuid, Uuid)> =
        sqlx::query_as("SELECT deck_id, group_id FROM deck_group_members ORDER BY deck_id")
            .fetch_all(&pool)
            .await
            .unwrap();
    let mut expected = held.to_vec();
    expected.sort();
    assert_eq!(kept, expected);

    // A deck joins a second group now, and the repository lists it under both.
    let repo = common::db(pool.clone()).deck_groups();
    assert_eq!(
        repo.add_deck(ann, DeckId(decks[0]), groups[1])
            .await
            .unwrap(),
        Filing::Done
    );
    let listed = repo.list(ann).await.unwrap();
    assert_eq!(listed.len(), 2);
    assert!(listed.iter().all(|g| g.decks.contains(&DeckId(decks[0]))));
    let duplicate = sqlx::query(insert)
        .bind(decks[0])
        .bind(groups[1])
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(
        duplicate
            .as_database_error()
            .is_some_and(sqlx::error::DatabaseError::is_unique_violation)
    );
}
