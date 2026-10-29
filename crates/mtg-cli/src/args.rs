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
    /// Compile the stored cards and keep the rules that validate as `card_rules` rows the server reads.
    Compile(CompileArgs),
    /// Check proposed rules files (written by hand or by a model) against stored cards: valid, read back as the printed text, and their support state.
    CheckRules(CheckRulesArgs),
    /// Cross-check the rules of the stored precon cards against Forge's card scripts, read from a local checkout: the verbs Forge names that the rules lack, and the unread tail grouped by Forge API and key.
    ForgeCheck(ForgeCheckArgs),
    /// Ask a model for the rules of cards and keep the proposals the checker accepts (needs `ANTHROPIC_API_KEY` and costs money; --dry-run sends nothing).
    Propose(ProposeArgs),
    /// Copy the proposals the checker accepts as `Verified` into the corpus: the rules file in `cards/` and the card's Scryfall record from the cache as its fixture.
    Promote(PromoteArgs),
    /// Print the reference of the rules DSL that `propose` sends to the model: the type tree and its vocabulary.
    DslReference,
    /// Print the JSON schema of a rules file that `propose --schema` sends to an OpenAI-style server.
    DslSchema,
}

/// What `fetch` downloads.
#[derive(Debug, Subcommand)]
pub enum Fetch {
    /// Scryfall's Oracle Cards and rulings bulk files, and with `--printings` every printing.
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

    /// Also download Default Cards, every printing with its flavor and printed names (about 80 MiB).
    #[arg(long)]
    pub printings: bool,
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

/// Options of `forge-check`.
#[derive(Debug, Args)]
pub struct ForgeCheckArgs {
    /// A local checkout of Forge, or its `forge-gui/res/cardsfolder` directory: the scripts are read from it at run time
    /// and nothing of them is stored.
    #[arg(long, value_name = "DIR")]
    pub forge: PathBuf,

    /// Only the precons whose code is this (any case) or whose name contains it (any case).
    #[arg(long, value_name = "CODE_OR_NAME", value_parser = clap::builder::NonEmptyStringValueParser::new())]
    pub deck: Option<String>,

    /// How many rows each ranking shows.
    #[arg(long, value_name = "N", default_value_t = 20)]
    pub top: usize,

    /// How many keys a row of the unread tail names.
    #[arg(long, value_name = "N", default_value_t = 6)]
    pub keys: usize,

    /// Write the whole report, with a line for every card, to this file; the summary is printed.
    #[arg(long, value_name = "FILE")]
    pub out: Option<PathBuf>,
}

/// Options of `compile`.
#[derive(Debug, Args)]
pub struct CompileArgs {
    /// Compile every stored card, not only the cards of the stored precons.
    #[arg(long, conflicts_with = "card")]
    pub all: bool,

    /// Compile only the card with this name (any case; the name of one of its faces also selects it).
    #[arg(long, value_name = "NAME", value_parser = clap::builder::NonEmptyStringValueParser::new())]
    pub card: Option<String>,
}

/// Options of `check-rules`.
#[derive(Debug, Args)]
pub struct CheckRulesArgs {
    /// The card the rules are for (any case; the name of one of its faces also selects it); without it each file is named
    /// after its card, `<slug of the name>.json`.
    #[arg(long, value_name = "NAME", value_parser = clap::builder::NonEmptyStringValueParser::new())]
    pub card: Option<String>,

    /// Also list the clauses that read back.
    #[arg(long)]
    pub verbose: bool,

    /// Each file is one JSON object from a card's name to its rules, checking many cards at once.
    #[arg(long, conflicts_with = "card")]
    pub bundle: bool,

    /// Write the completed rules of each valid proposal into this directory as `<slug>.json`.
    #[arg(long, value_name = "DIR")]
    pub write: Option<PathBuf>,

    /// The rules files (JSON); with `--card`, exactly one.
    #[arg(value_name = "FILE", required = true)]
    pub files: Vec<PathBuf>,
}

/// The API `propose` asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Provider {
    /// The Anthropic Messages API; the key comes from `ANTHROPIC_API_KEY`.
    Anthropic,
    /// An OpenAI-style chat completions server; a key in `MTG_PROPOSE_API_KEY` is optional.
    Openai,
}

/// Options of `propose`.
#[derive(Debug, Args)]
// Each flag is one independent command-line switch.
#[allow(clippy::struct_excessive_bools)]
pub struct ProposeArgs {
    /// The cards to ask about, by name (any case; the name of a face also selects its card): `--cards "Wind Drake" "Opt"`.
    #[arg(long, value_name = "NAME", num_args = 1.., conflicts_with = "sample", value_parser = clap::builder::NonEmptyStringValueParser::new())]
    pub cards: Vec<String>,

    /// Ask about a seeded sample of this many stored cards that have no hand-written file and are not `Verified`, short ones first.
    #[arg(long, value_name = "N")]
    pub sample: Option<usize>,

    /// The seed of the sample (default 1): the same seed takes the same cards.
    #[arg(long, value_name = "N", requires = "sample", conflicts_with = "cards")]
    pub seed: Option<u64>,

    /// Take sample cards whose name begins with these letters: `A-L`, `A-F,M`.
    #[arg(
        long,
        value_name = "A-L",
        requires = "sample",
        conflicts_with = "cards"
    )]
    pub letters: Option<String>,

    /// Take sample cards only from the stored precon decks (any board), by the same rules and order.
    #[arg(long, requires = "sample", conflicts_with = "cards")]
    pub precons: bool,

    /// The API to ask: the Anthropic Messages API, or an OpenAI-style chat completions server such as Ollama or llama.cpp.
    #[arg(long, value_enum, default_value_t = Provider::Anthropic)]
    pub provider: Provider,

    /// The address of the API (default: the Anthropic API, or `http://localhost:11434/v1` for `--provider openai`).
    #[arg(long, value_name = "URL")]
    pub base_url: Option<String>,

    /// The model id (default `claude-sonnet-5-5` for Anthropic; required for `--provider openai`).
    #[arg(long, value_name = "ID")]
    pub model: Option<String>,

    /// Do not ask an OpenAI-style server for a JSON object (`response_format`), for servers that reject the field.
    #[arg(long)]
    pub no_json_mode: bool,

    /// Send the JSON schema of a rules file (`response_format` `json_schema`) so an OpenAI-style server limits the answer
    /// to valid shapes; replaces the plain JSON-object mode (`mtg-cli dsl-schema` prints it).
    #[arg(long, conflicts_with = "no_json_mode")]
    pub schema: bool,

    /// The most tokens an answer of an OpenAI-style server may have, a reasoning model's thinking included.
    #[arg(long, value_name = "N", default_value_t = crate::propose::c11i_openai::C11I_DEFAULT_ANSWER_TOKENS)]
    pub max_answer_tokens: u32,

    /// The most requests to send (a repair round is a request).
    #[arg(long, value_name = "N", default_value_t = 100)]
    pub max_requests: u32,

    /// The most input tokens to spend, cached tokens included; a request is not sent when its estimate would pass this.
    #[arg(long, value_name = "N", default_value_t = 3_000_000)]
    pub max_input_tokens: u64,

    /// How many hand-written examples each prompt holds.
    #[arg(long, value_name = "N", default_value_t = 4, value_parser = clap::value_parser!(u8).range(3..=5))]
    pub examples: u8,

    /// Where the proposals go (default: `proposals/propose` in the cache directory). Accepted ones are `<slug>.json`, the rest are under `rejected/`.
    #[arg(long, value_name = "DIR")]
    pub out: Option<PathBuf>,

    /// Skip the cards that already have a saved proposal in the output directory.
    #[arg(long)]
    pub resume: bool,

    /// Print the prompt of the first card and the estimated size of the run, and send nothing (no API key is needed).
    #[arg(long)]
    pub dry_run: bool,

    /// Then promote the run's accepted `Verified` proposals into this `mtg-cards` crate directory (`crates/mtg-cards` when no
    /// directory follows), as `promote` does.
    #[arg(long, value_name = "DIR", num_args = 0..=1, default_missing_value = crate::w2a_promote::W2A_DEFAULT_OUT)]
    pub promote: Option<PathBuf>,
}

/// Options of `promote`.
#[derive(Debug, Args)]
pub struct PromoteArgs {
    /// List what would be promoted and why the other files are skipped, and write nothing.
    #[arg(long)]
    pub dry_run: bool,

    /// Also promote the proposals accepted as `Partial`.
    #[arg(long)]
    pub partial: bool,

    /// The `mtg-cards` crate directory, holding `cards/` and `tests/fixtures/corpus/` (default: `crates/mtg-cards`).
    #[arg(long, value_name = "DIR")]
    pub out: Option<PathBuf>,

    /// The proposal files, each named `<slug>.json` after its card.
    #[arg(value_name = "FILE", required = true)]
    pub files: Vec<PathBuf>,
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
    #[case(&["mtg-cli", "compile"])]
    #[case(&["mtg-cli", "compile", "--all"])]
    #[case(&["mtg-cli", "compile", "--card", "Lightning Bolt"])]
    #[case(&["mtg-cli", "--database-url", "postgres://x/y", "compile", "--all"])]
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
    #[case(&["mtg-cli", "compile", "--all", "--card", "Shock"])]
    #[case(&["mtg-cli", "compile", "--card"])]
    #[case(&["mtg-cli", "compile", "--card", ""])]
    #[case(&["mtg-cli", "compile", "Shock"])]
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
    #[case(&["mtg-cli", "compile"], false, None)]
    #[case(&["mtg-cli", "compile", "--all"], true, None)]
    #[case(&["mtg-cli", "compile", "--card", "Fire // Ice"], false, Some("Fire // Ice"))]
    fn compile_options_land_in_their_fields(
        #[case] args: &[&str],
        #[case] all: bool,
        #[case] card: Option<&str>,
    ) {
        let cli = Cli::try_parse_from(args).unwrap();
        let Command::Compile(args) = cli.command else {
            panic!("expected compile");
        };
        assert_eq!((args.all, args.card.as_deref()), (all, card));
    }

    #[rstest]
    #[case(&["mtg-cli", "check-rules", "a.json"], None, false, false, None, 1)]
    #[case(&["mtg-cli", "check-rules", "--card", "Fire // Ice", "--verbose", "a.json"], Some("Fire // Ice"), true, false, None, 1)]
    #[case(&["mtg-cli", "check-rules", "--bundle", "--write", "out", "a.json", "b.json"], None, false, true, Some("out"), 2)]
    fn check_rules_options_land_in_their_fields(
        #[case] args: &[&str],
        #[case] card: Option<&str>,
        #[case] verbose: bool,
        #[case] bundle: bool,
        #[case] write: Option<&str>,
        #[case] files: usize,
    ) {
        let cli = Cli::try_parse_from(args).unwrap();
        let Command::CheckRules(args) = cli.command else {
            panic!("expected check-rules");
        };
        assert_eq!(
            (
                args.card.as_deref(),
                args.verbose,
                args.bundle,
                args.write,
                args.files.len()
            ),
            (card, verbose, bundle, write.map(PathBuf::from), files)
        );
    }

    #[test]
    fn check_rules_needs_a_file_and_a_card_excludes_a_bundle() {
        assert!(Cli::try_parse_from(["mtg-cli", "check-rules"]).is_err());
        assert!(
            Cli::try_parse_from([
                "mtg-cli",
                "check-rules",
                "--bundle",
                "--card",
                "X",
                "a.json"
            ])
            .is_err()
        );
    }

    #[rstest]
    #[case(&["mtg-cli", "forge-check", "--forge", "f"], "f", None, (20, 6), None)]
    #[case(&["mtg-cli", "forge-check", "--forge", "f", "--deck", "C21", "--top", "5", "--keys", "9", "--out", "r.txt"], "f", Some("C21"), (5, 9), Some("r.txt"))]
    fn forge_check_options_land_in_their_fields(
        #[case] args: &[&str],
        #[case] forge: &str,
        #[case] deck: Option<&str>,
        #[case] rows: (usize, usize),
        #[case] out: Option<&str>,
    ) {
        let cli = Cli::try_parse_from(args).unwrap();
        let Command::ForgeCheck(args) = cli.command else {
            panic!("expected forge-check");
        };
        assert_eq!(
            (
                args.forge,
                args.deck.as_deref(),
                (args.top, args.keys),
                args.out
            ),
            (PathBuf::from(forge), deck, rows, out.map(PathBuf::from))
        );
    }

    #[test]
    fn forge_check_needs_the_checkout() {
        assert!(Cli::try_parse_from(["mtg-cli", "forge-check"]).is_err());
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

    fn propose(args: &[&str]) -> Result<ProposeArgs, clap::Error> {
        let mut line = vec!["mtg-cli", "propose"];
        line.extend_from_slice(args);
        match Cli::try_parse_from(line)?.command {
            Command::Propose(args) => Ok(args),
            other => panic!("expected propose, got {other:?}"),
        }
    }

    #[test]
    fn propose_defaults_are_a_modest_budget_and_the_documented_model() {
        let args = propose(&["--cards", "Opt"]).unwrap();
        assert_eq!(
            (
                args.provider,
                args.model.as_deref(),
                args.max_requests,
                args.max_input_tokens,
                args.examples
            ),
            (Provider::Anthropic, None, 100, 3_000_000, 4)
        );
        assert!(!args.resume && !args.dry_run && args.out.is_none());
        assert!(args.base_url.is_none() && !args.no_json_mode && !args.schema);
        assert_eq!(args.max_answer_tokens, 16_384);
    }

    #[test]
    fn propose_takes_a_local_provider_its_address_and_the_json_mode_switch() {
        let args = propose(&[
            "--cards",
            "Opt",
            "--provider",
            "openai",
            "--base-url",
            "http://gpu:8080/v1",
            "--model",
            "gpt-oss:20b",
            "--no-json-mode",
        ])
        .unwrap();
        assert_eq!(args.provider, Provider::Openai);
        assert_eq!(args.base_url.as_deref(), Some("http://gpu:8080/v1"));
        assert_eq!(args.model.as_deref(), Some("gpt-oss:20b"));
        assert!(args.no_json_mode);
        assert!(propose(&["--cards", "Opt", "--provider", "ollama"]).is_err());
    }

    #[test]
    fn w3a_the_schema_flag_replaces_json_mode_and_its_command_takes_no_arguments() {
        let args = propose(&["--cards", "Opt", "--provider", "openai", "--schema"]).unwrap();
        assert!(args.schema && !args.no_json_mode);
        assert!(propose(&["--cards", "Opt", "--schema", "--no-json-mode"]).is_err());
        assert!(matches!(
            Cli::try_parse_from(["mtg-cli", "dsl-schema"])
                .unwrap()
                .command,
            Command::DslSchema
        ));
        assert!(Cli::try_parse_from(["mtg-cli", "dsl-schema", "x"]).is_err());
    }

    #[test]
    fn propose_takes_several_names_after_one_flag_names_with_commas_and_a_sample() {
        let args = propose(&["--cards", "Alesha, Who Smiles at Death", "Opt", "--resume"]).unwrap();
        assert_eq!(args.cards, ["Alesha, Who Smiles at Death", "Opt"]);
        assert!(args.resume);
        let sample = propose(&[
            "--sample",
            "40",
            "--seed",
            "20261029",
            "--letters",
            "A-L",
            "--out",
            "/tmp/p",
        ])
        .unwrap();
        assert!(!sample.precons);
        let precons = propose(&["--sample", "5", "--precons", "--seed", "3", "--resume"]).unwrap();
        assert!(precons.precons && precons.resume);
        assert_eq!((precons.sample, precons.seed), (Some(5), Some(3)));
        assert_eq!(
            (
                sample.sample,
                sample.seed,
                sample.letters.as_deref(),
                sample.out
            ),
            (
                Some(40),
                Some(20_261_029),
                Some("A-L"),
                Some(PathBuf::from("/tmp/p"))
            )
        );
    }

    #[rstest]
    #[case::names_and_a_sample(&["--cards", "Opt", "--sample", "3"])]
    #[case::seed_without_a_sample(&["--cards", "Opt", "--seed", "3"])]
    #[case::letters_without_a_sample(&["--letters", "A-L"])]
    #[case::precons_without_a_sample(&["--precons"])]
    #[case::precons_with_names(&["--cards", "Opt", "--precons"])]
    #[case::too_few_examples(&["--cards", "Opt", "--examples", "2"])]
    #[case::too_many_examples(&["--cards", "Opt", "--examples", "6"])]
    #[case::an_empty_name(&["--cards", ""])]
    fn propose_refuses_options_that_do_not_fit_together(#[case] args: &[&str]) {
        assert!(propose(args).is_err());
    }

    #[test]
    fn the_dsl_reference_command_takes_no_arguments() {
        assert!(matches!(
            Cli::try_parse_from(["mtg-cli", "dsl-reference"])
                .unwrap()
                .command,
            Command::DslReference
        ));
        assert!(Cli::try_parse_from(["mtg-cli", "dsl-reference", "x"]).is_err());
    }
}

#[cfg(test)]
mod w2a_tests {
    //! Goal: `promote` takes its files, the dry run, the partial switch and the crate directory, and `propose --promote`
    //! names the default crate directory unless a directory follows.
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(&["a.json"], false, false, None)]
    #[case(&["--dry-run", "a.json", "b.json"], true, false, None)]
    #[case(&["--partial", "--out", "x", "a.json"], false, true, Some("x"))]
    fn promote_reads_its_switches(
        #[case] line: &[&str],
        #[case] dry_run: bool,
        #[case] partial: bool,
        #[case] out: Option<&str>,
    ) {
        let mut words = vec!["mtg-cli", "promote"];
        words.extend_from_slice(line);
        let Command::Promote(args) = Cli::try_parse_from(words).unwrap().command else {
            panic!("expected promote");
        };
        assert_eq!((args.dry_run, args.partial), (dry_run, partial));
        assert_eq!(args.out, out.map(PathBuf::from));
        assert!(
            args.files
                .iter()
                .all(|file| file.extension().unwrap() == "json")
        );
    }

    #[test]
    fn promote_needs_a_file() {
        assert!(Cli::try_parse_from(["mtg-cli", "promote", "--dry-run"]).is_err());
    }

    #[rstest]
    #[case(&["--cards", "Opt"], None)]
    #[case(&["--cards", "Opt", "--promote"], Some("crates/mtg-cards"))]
    #[case(&["--promote", "x", "--cards", "Opt"], Some("x"))]
    fn propose_promotes_into_the_named_or_default_crate(
        #[case] line: &[&str],
        #[case] out: Option<&str>,
    ) {
        let mut words = vec!["mtg-cli", "propose"];
        words.extend_from_slice(line);
        let Command::Propose(args) = Cli::try_parse_from(words).unwrap().command else {
            panic!("expected propose");
        };
        assert_eq!(args.promote, out.map(PathBuf::from));
    }
}
