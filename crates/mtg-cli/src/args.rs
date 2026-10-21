//! The command line.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// Downloads Scryfall and MTGJSON data and loads it into Postgres.
#[derive(Debug, Parser)]
#[command(name = "mtg-cli", version, about, propagate_version = true)]
pub struct Cli {
    /// Directory for downloaded files (default: the user's cache directory, under `mtg-cli`).
    #[arg(long, global = true, env = "MTG_CACHE_DIR", value_name = "DIR")]
    pub cache: Option<PathBuf>,

    /// Postgres to read and write.
    #[arg(
        long,
        global = true,
        env = "DATABASE_URL",
        value_name = "URL",
        hide_env_values = true
    )]
    pub database_url: Option<String>,

    /// What to do.
    #[command(subcommand)]
    pub command: Command,
}

/// The commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Download data into the cache.
    Fetch {
        /// What to download.
        #[command(subcommand)]
        what: Fetch,
    },
    /// Load the cached files into the database; inputs that were loaded before are skipped.
    Ingest(IngestArgs),
    /// Show what is cached, the latest import runs and the row counts.
    Status,
    /// Report how well the compiler and the engine cover the stored precons, and what would free the most cards.
    Coverage(CoverageArgs),
    /// List a seeded sample of the cards the compiler reads: oracle text beside the reading and the JSON.
    Audit(AuditArgs),
}

/// What `fetch` downloads.
#[derive(Debug, Subcommand)]
pub enum Fetch {
    /// Scryfall's Oracle Cards and rulings bulk files.
    Scryfall(ScryfallArgs),
    /// The MTGJSON deck index and the decks it lists.
    Decks(DecksArgs),
}

/// Options of `fetch scryfall`.
#[derive(Debug, Args)]
pub struct ScryfallArgs {
    /// Download even when the cached files are current.
    #[arg(long)]
    pub force: bool,
}

/// Options of `fetch decks`.
#[derive(Debug, Args)]
pub struct DecksArgs {
    /// Fetch at most this many decks, the newest first.
    #[arg(long, value_name = "N")]
    pub limit: Option<usize>,

    /// Fetch only decks whose name contains this text, or whose file name it is (repeatable).
    #[arg(long, value_name = "NAME")]
    pub only: Vec<String>,

    /// Deck type to fetch (repeatable; `all` for every type); the default is "Commander Deck".
    #[arg(long = "type", value_name = "TYPE")]
    pub deck_type: Vec<String>,

    /// Download even when the cached files are current.
    #[arg(long)]
    pub force: bool,
}

/// Options of `coverage`.
#[derive(Debug, Args)]
pub struct CoverageArgs {
    /// Only the precons whose code is this (any case) or whose name contains it (any case).
    #[arg(long, value_name = "CODE_OR_NAME", value_parser = clap::builder::NonEmptyStringValueParser::new())]
    pub deck: Option<String>,

    /// How many blockers each of the two rankings shows.
    #[arg(long, value_name = "N", default_value_t = mtg_cards::coverage::DEFAULT_TOP)]
    pub top: usize,
}

/// How many cards `audit` lists by default.
pub const DEFAULT_AUDIT_COUNT: usize = 100;

/// Options of `audit`.
#[derive(Debug, Args)]
pub struct AuditArgs {
    /// How many cards to list.
    #[arg(long, value_name = "N", default_value_t = DEFAULT_AUDIT_COUNT)]
    pub count: usize,

    /// The seed of the sample: the same seed lists the same cards.
    #[arg(long, value_name = "N", default_value_t = 1)]
    pub seed: u64,

    /// Write the listing to this file instead of printing it.
    #[arg(value_name = "FILE")]
    pub file: Option<PathBuf>,
}

/// Options of `ingest`.
#[derive(Debug, Args)]
pub struct IngestArgs {
    /// Read and check the cached files, write nothing and open no database connection.
    #[arg(long)]
    pub dry_run: bool,

    /// Load inputs that were loaded before.
    #[arg(long)]
    pub force: bool,

    /// Records per database transaction.
    #[arg(long, value_name = "N", default_value_t = mtg_db::import::DEFAULT_BATCH_SIZE)]
    pub batch_size: usize,
}

#[cfg(test)]
mod tests {
    //! Goal: the command line is well formed, takes the documented options in any position and refuses unknown ones.
    use clap::CommandFactory;
    use rstest::rstest;

    use super::*;

    #[test]
    fn the_definition_is_consistent() {
        Cli::command().debug_assert();
    }

    #[rstest]
    #[case(&["mtg-cli", "status"])]
    #[case(&["mtg-cli", "ingest"])]
    #[case(&["mtg-cli", "ingest", "--dry-run", "--force", "--batch-size", "100"])]
    #[case(&["mtg-cli", "fetch", "scryfall"])]
    #[case(&["mtg-cli", "fetch", "scryfall", "--force"])]
    #[case(&["mtg-cli", "fetch", "decks", "--limit", "3", "--only", "Atraxa", "--only", "Eldrazi"])]
    #[case(&["mtg-cli", "fetch", "decks", "--type", "all", "--type", "Theme Deck"])]
    #[case(&["mtg-cli", "--cache", "/tmp/c", "status"])]
    #[case(&["mtg-cli", "status", "--cache", "/tmp/c", "--database-url", "postgres://x/y"])]
    #[case(&["mtg-cli", "coverage"])]
    #[case(&["mtg-cli", "coverage", "--deck", "C21", "--top", "5"])]
    #[case(&["mtg-cli", "coverage", "--top", "0", "--deck", "Quantum Quandrix"])]
    #[case(&["mtg-cli", "--database-url", "postgres://x/y", "coverage"])]
    #[case(&["mtg-cli", "audit"])]
    #[case(&["mtg-cli", "audit", "--count", "20", "--seed", "7", "out.txt"])]
    fn documented_invocations_parse(#[case] args: &[&str]) {
        assert!(Cli::try_parse_from(args).is_ok(), "{args:?}");
    }

    #[rstest]
    #[case(&["mtg-cli"])]
    #[case(&["mtg-cli", "fetch"])]
    #[case(&["mtg-cli", "fetch", "everything"])]
    #[case(&["mtg-cli", "fetch", "decks", "--limit", "many"])]
    #[case(&["mtg-cli", "ingest", "--batch-size", "-1"])]
    #[case(&["mtg-cli", "status", "--frobnicate"])]
    #[case(&["mtg-cli", "coverage", "--top", "many"])]
    #[case(&["mtg-cli", "coverage", "--top", "-1"])]
    #[case(&["mtg-cli", "coverage", "--deck"])]
    #[case(&["mtg-cli", "coverage", "--deck", ""])]
    #[case(&["mtg-cli", "coverage", "C21"])]
    #[case(&["mtg-cli", "audit", "--count", "many"])]
    #[case(&["mtg-cli", "audit", "--seed", "-1"])]
    #[case(&["mtg-cli", "audit", "a.txt", "b.txt"])]
    fn bad_invocations_are_refused(#[case] args: &[&str]) {
        assert!(Cli::try_parse_from(args).is_err(), "{args:?}");
    }

    #[test]
    fn options_land_in_their_fields() {
        let cli = Cli::try_parse_from([
            "mtg-cli", "--cache", "/c", "fetch", "decks", "--limit", "5", "--only", "A", "--type",
            "all",
        ])
        .unwrap();
        assert_eq!(cli.cache, Some(PathBuf::from("/c")));
        let Command::Fetch {
            what: Fetch::Decks(args),
        } = cli.command
        else {
            panic!("expected fetch decks");
        };
        assert_eq!(
            (args.limit, args.only, args.deck_type),
            (Some(5), vec!["A".into()], vec!["all".into()])
        );
    }

    #[rstest]
    #[case(&["mtg-cli", "audit"], 100, 1, None)]
    #[case(&["mtg-cli", "audit", "--seed", "9"], 100, 9, None)]
    #[case(&["mtg-cli", "audit", "x.txt", "--count", "5"], 5, 1, Some("x.txt"))]
    fn audit_options_land_in_their_fields(
        #[case] args: &[&str],
        #[case] count: usize,
        #[case] seed: u64,
        #[case] file: Option<&str>,
    ) {
        let cli = Cli::try_parse_from(args).unwrap();
        let Command::Audit(args) = cli.command else {
            panic!("expected audit");
        };
        assert_eq!(
            (args.count, args.seed, args.file),
            (count, seed, file.map(PathBuf::from))
        );
    }

    #[rstest]
    #[case(&["mtg-cli", "coverage"], None, 20)]
    #[case(&["mtg-cli", "coverage", "--deck", "C21"], Some("C21"), 20)]
    #[case(&["mtg-cli", "coverage", "--top", "7"], None, 7)]
    #[case(&["mtg-cli", "coverage", "--top", "3", "--deck", "Quantum Quandrix"], Some("Quantum Quandrix"), 3)]
    fn coverage_options_land_in_their_fields(
        #[case] args: &[&str],
        #[case] deck: Option<&str>,
        #[case] top: usize,
    ) {
        let cli = Cli::try_parse_from(args).unwrap();
        let Command::Coverage(args) = cli.command else {
            panic!("expected coverage");
        };
        assert_eq!((args.deck.as_deref(), args.top), (deck, top));
    }
}
