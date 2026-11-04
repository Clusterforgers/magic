//! Goal: a draft stores its source and whether the Oracle text matched, and a Forge draft with a matching text is `approved` without a verdict until a `wrong` counts, whatever the verdicts, while any other draft needs a human read.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use mtg_db::{NewDraft, ReadState, SOURCE_FORGE, Verdict};
use mtg_import::ids::OracleId;
use sqlx::PgPool;
use uuid::Uuid;

use common::drafts::{draft, key};
use common::{oracle_of, seeded};

/// Verdicts by account (none is the server) in the order they are given.
type Sequence = &'static [(Option<u128>, Verdict)];

/// A Forge draft: whether its Oracle text matched, the verdicts given on it, and the state and trust flag expected.
type Case = (&'static str, bool, Sequence, ReadState, bool);

const CORRECT: Verdict = Verdict::Correct;
const WRONG: Verdict = Verdict::Wrong;

const CASES: &[Case] = &[
    ("matched, no verdict", true, &[], ReadState::Approved, true),
    (
        "matched, one wrong",
        true,
        &[(Some(1), WRONG)],
        ReadState::Wrong,
        true,
    ),
    (
        "matched, the server's wrong",
        true,
        &[(None, WRONG)],
        ReadState::Wrong,
        true,
    ),
    (
        "matched, wrong then correct by the same account",
        true,
        &[(Some(1), WRONG), (Some(1), CORRECT)],
        ReadState::Approved,
        true,
    ),
    (
        "unmatched, no verdict",
        false,
        &[],
        ReadState::Pending,
        false,
    ),
    (
        "unmatched, one correct",
        false,
        &[(Some(1), CORRECT)],
        ReadState::Approved,
        false,
    ),
];

#[sqlx::test]
async fn the_source_and_the_oracle_match_are_stored_with_the_draft(pool: PgPool) {
    let database = seeded(pool).await;
    let (bolt, ring) = (oracle_of("Lightning Bolt"), oracle_of("Sol Ring"));
    let forge = NewDraft {
        source: SOURCE_FORGE.into(),
        oracle_matches: true,
        proof: None,
        ..draft(&bolt, 1)
    };
    database
        .drafts()
        .insert(&[forge, draft(&ring, 2)])
        .await
        .unwrap();
    let current = database.drafts().current().await.unwrap();
    let of = |card: &OracleId| current.iter().find(|d| &d.oracle_id == card).unwrap();
    assert_eq!(
        (of(&bolt).source.as_str(), of(&bolt).oracle_matches),
        ("forge", true)
    );
    assert_eq!(
        (of(&ring).source.as_str(), of(&ring).oracle_matches),
        ("sweep", false),
        "a draft written without a source is a sweep draft"
    );
}

#[sqlx::test]
async fn a_forge_draft_with_a_matching_oracle_text_is_approved_until_a_wrong_counts(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let bolt = oracle_of("Lightning Bolt");
    for (name, matches, verdicts, state, forge) in CASES {
        let row = NewDraft {
            source: SOURCE_FORGE.into(),
            oracle_matches: *matches,
            ..draft(&bolt, 1)
        };
        database.drafts().insert(&[row]).await.unwrap();
        for (who, verdict) in *verdicts {
            let note = (*verdict == WRONG).then_some("it does not work");
            let by = who.map(Uuid::from_u128);
            database
                .reads()
                .append(by, &key(&bolt, 1), *verdict, note)
                .await
                .unwrap();
        }
        let all = database
            .reads()
            .state_of(std::slice::from_ref(&bolt))
            .await
            .unwrap();
        assert_eq!((all[0].state, all[0].forge), (*state, *forge), "{name}");
        sqlx::query("DELETE FROM card_drafts")
            .execute(&pool)
            .await
            .unwrap();
    }
}
