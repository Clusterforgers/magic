//! Goal: `forge-check` reads a checkout laid out like Forge's, matches its scripts with the stored precon cards by name, reports the items the rules lack and the cards without a script, writes the whole report with a line for every card to a file, and refuses a checkout without scripts or a deck that does not exist.

use std::fs;
use std::path::Path;

use mtg_cli::args::ForgeCheckArgs;
use mtg_cli::error::CliError;
use mtg_cli::forge::forge_check;
use mtg_cli::ingest::{IngestOptions, ingest};
use mtg_db::Database;
use sqlx::PgPool;

use crate::common::{fixture_cache, progress};

/// Scripts of invented content in Forge's layout, for three of the five cards of the fixture precon.
fn checkout(root: &Path) {
    let folder = root.join("forge-gui/res/cardsfolder");
    for (path, text) in [
        (
            "s/sol_ring.txt",
            "Name:Sol Ring\nManaCost:1\nTypes:Artifact\nA:AB$ Mana | Cost$ T | Produced$ C | Amount$ 2\n",
        ),
        (
            "f/fire_ice.txt",
            "Name:Fire\nA:SP$ Zork | ValidTgts$ Any | Frobnicate$ True\nALTERNATE\nName:Ice\nA:SP$ Draw | NumCards$ 1\n",
        ),
        (
            "a/atraxa.txt",
            "Name:Atraxa, Praetors\u{2019} Voice\nK:Flying\nT:Mode$ Quux | Execute$ TrigQuux\nSVar:TrigQuux:DB$ Zork\n",
        ),
    ] {
        let file = folder.join(path);
        let parent = file.parent().unwrap_or(&folder);
        fs::create_dir_all(parent).unwrap_or_else(|e| panic!("{}: {e}", parent.display()));
        fs::write(&file, text).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    }
}

async fn stored(pool: PgPool, cache_dir: &Path) -> Database {
    let cache = fixture_cache(cache_dir);
    let db = Database::from_pool(pool);
    let (progress, _) = progress();
    let options = IngestOptions {
        force: false,
        batch_size: 500,
    };
    ingest(&cache, &db, &options, &progress)
        .await
        .unwrap_or_else(|e| panic!("ingest runs: {e}"));
    db
}

fn args(forge: &Path, out: Option<&Path>) -> ForgeCheckArgs {
    ForgeCheckArgs {
        forge: forge.to_path_buf(),
        deck: None,
        top: 20,
        keys: 6,
        out: out.map(Path::to_path_buf),
    }
}

#[sqlx::test]
async fn the_scripts_are_matched_by_name_and_the_report_names_what_the_rules_lack(pool: PgPool) {
    let cache = tempfile::tempdir().unwrap();
    let db = stored(pool, cache.path()).await;
    let forge = tempfile::tempdir().unwrap();
    checkout(forge.path());

    let text = forge_check(&db, &args(forge.path(), None)).await.unwrap();

    assert!(
        text.starts_with("forge-check: 5 stored cards, 3 with a Forge script (60.0%), 2 without\n"),
        "{text}"
    );
    for wanted in [
        "state of those cards:",
        "items their scripts name: ",
        "unread tail:",
        "share of the grouped tail one reader per group covers:",
    ] {
        assert!(text.contains(wanted), "{wanted} missing from\n{text}");
    }
    // The detail of every card is for the file.
    assert!(!text.contains("our verbs"), "{text}");
}

#[sqlx::test]
async fn the_whole_report_goes_to_a_file_with_a_line_for_every_card(pool: PgPool) {
    let cache = tempfile::tempdir().unwrap();
    let db = stored(pool, cache.path()).await;
    let forge = tempfile::tempdir().unwrap();
    checkout(forge.path());
    let out = forge.path().join("report.txt");

    let summary = forge_check(&db, &args(forge.path(), Some(&out)))
        .await
        .unwrap();

    assert!(summary.contains("forge-check: written to "), "{summary}");
    let full = fs::read_to_string(&out).unwrap();
    for wanted in [
        "  Sol Ring [",
        "  Fire // Ice [",
        "  Atraxa, Praetors' Voice [",
        "Zork [unmapped]",
        "stored cards with no Forge script:\n  Arcane Signet\n  Forest\n",
        "      our verbs: ",
    ] {
        assert!(full.contains(wanted), "{wanted} missing from\n{full}");
    }
}

#[sqlx::test]
async fn a_checkout_without_scripts_and_an_unknown_deck_are_refused(pool: PgPool) {
    let cache = tempfile::tempdir().unwrap();
    let db = stored(pool, cache.path()).await;
    let empty = tempfile::tempdir().unwrap();

    let error = forge_check(&db, &args(empty.path(), None))
        .await
        .unwrap_err();
    assert!(matches!(error, CliError::Config(_)), "{error}");
    assert!(
        error.to_string().contains("no Forge card scripts"),
        "{error}"
    );

    let forge = tempfile::tempdir().unwrap();
    checkout(forge.path());
    let mut wanted = args(forge.path(), None);
    wanted.deck = Some("no such deck".to_owned());
    let error = forge_check(&db, &wanted).await.unwrap_err();
    assert!(error.to_string().contains("no such deck"), "{error}");
}
