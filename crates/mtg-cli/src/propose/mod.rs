//! `propose`: sends a card, the DSL reference and the nearest hand-written files to a model and keeps the proposals the
//! checker accepts (`docs/cards-dsl.md`, Model-assisted reading). The person who runs it supplies the API key and pays.

pub mod anthropic;
pub mod budget;
pub mod examples;
pub mod fake;
pub mod model;
pub mod outcome;
pub mod prompt;
pub mod select;
pub mod session;
pub mod store;

use std::fmt::Write as _;
use std::path::PathBuf;

use mtg_cards::PrintedCard;
use mtg_cards::builtin;
use mtg_cards::reference::dsl_reference;
use mtg_db::Database;

use crate::args::ProposeArgs;
use crate::audit::stored_cards;
use crate::cache::Cache;
use crate::compile::names_of;
use crate::error::CliError;
use crate::progress::Progress;
use anthropic::{AnthropicClient, AnthropicConfig, ApiKey};
use examples::ExampleIndex;
use model::{ModelClient, estimate_tokens};
use outcome::ProposeReport;
use session::{Context, Options, Session};
use store::Store;

/// The model asked when `--model` is not given.
pub const DEFAULT_MODEL: &str = "claude-sonnet-5-5";

/// The folder of the proposals when `--out` is not given, beside the downloaded files of the cache.
pub fn default_out(cache: &Cache) -> PathBuf {
    cache.path("proposals/propose")
}

/// The context every prompt shares: the checker's inputs, the system prompt and the examples.
pub fn context(cards: &[PrintedCard]) -> Result<Context<'static>, CliError> {
    let library = builtin::library().map_err(|errors| {
        CliError::Config(format!("the embedded rules files do not load:\n{errors}"))
    })?;
    Ok(Context {
        library,
        names: names_of(cards),
        caps: mtg_engine::caps(),
        system: prompt::system_prompt(&dsl_reference()),
        examples: ExampleIndex::build(library, cards),
    })
}

/// Asks the model about the cards at `targets` (positions in `cards`) and saves what the checker accepts in `store`.
pub fn propose_cards(
    cards: &[PrintedCard],
    targets: &[usize],
    client: &dyn ModelClient,
    options: &Options,
    store: &Store,
    progress: &Progress,
) -> Result<ProposeReport, CliError> {
    let ctx = context(cards)?;
    let chosen: Vec<&PrintedCard> = targets.iter().filter_map(|at| cards.get(*at)).collect();
    Session::new(&ctx, client, options, store, progress).run(&chosen)
}

/// What a run would send, without sending anything: the prompt of the first card and the estimated size of the run.
pub fn dry_run(
    cards: &[PrintedCard],
    targets: &[usize],
    options: &Options,
) -> Result<String, CliError> {
    let ctx = context(cards)?;
    let Some(first) = targets.first().and_then(|at| cards.get(*at)) else {
        return Err(CliError::Config("no card is selected".into()));
    };
    let near = ctx.examples.nearest(first, options.examples);
    let message = prompt::card_prompt(first, &near);
    let request = prompt::first_request(&options.model, &ctx.system, message.clone());
    let per_request = request.estimated_tokens();
    let count = u64::try_from(targets.len()).unwrap_or(u64::MAX);
    let mut text = format!(
        "propose --dry-run: nothing is sent\nmodel: {}\ncards: {} (first: {})\n",
        options.model,
        targets.len(),
        first.name
    );
    let _ = writeln!(
        text,
        "system prompt: {} bytes (the instructions and the DSL reference of {} bytes; `mtg-cli dsl-reference` prints it)\ncard prompt: {} bytes with {} examples",
        ctx.system.len(),
        ctx.system.len() - prompt::instructions().len(),
        message.len(),
        near.len()
    );
    let _ = writeln!(
        text,
        "estimated input tokens of the first request: {per_request} (an upper estimate, 3 bytes a token)\n\
         estimated input tokens of the run: at most {} for the first requests of {count} cards, {} with a repair round for each (the budget is {} tokens and {} requests)",
        per_request.saturating_mul(count),
        per_request.saturating_mul(count).saturating_mul(2),
        options.max_input_tokens,
        options.max_requests
    );
    let _ = writeln!(
        text,
        "the system prompt is sent with a cache marker, so a billed request reads it from the cache at a fraction of the price; every token counts against --max-input-tokens\n\n--- system prompt: the instructions ---\n{}[the DSL reference follows: {} tokens]\n\n--- card prompt ---\n{message}",
        prompt::instructions(),
        estimate_tokens(ctx.system.len() - prompt::instructions().len())
    );
    Ok(text.trim_end().to_owned())
}

/// The options of a run from the command line.
pub fn options_of(args: &ProposeArgs) -> Options {
    Options {
        model: args.model.clone(),
        examples: usize::from(args.examples),
        resume: args.resume,
        max_requests: args.max_requests,
        max_input_tokens: args.max_input_tokens,
    }
}

/// The cards the arguments select, as positions in `cards`.
pub fn targets_of(cards: &[PrintedCard], args: &ProposeArgs) -> Result<Vec<usize>, CliError> {
    if !args.cards.is_empty() {
        return select::by_names(cards, &args.cards);
    }
    let Some(count) = args.sample else {
        return Err(CliError::Config(
            "name the cards with --cards, or take a sample of them with --sample N".into(),
        ));
    };
    let letters = args
        .letters
        .as_deref()
        .map(str::parse::<select::Letters>)
        .transpose()
        .map_err(|message| CliError::Config(format!("--letters: {message}")))?;
    let library = builtin::library().map_err(|errors| {
        CliError::Config(format!("the embedded rules files do not load:\n{errors}"))
    })?;
    Ok(select::sample(
        cards,
        library,
        &mtg_engine::caps(),
        count,
        args.seed.unwrap_or(1),
        letters.as_ref(),
    ))
}

/// Runs `propose` from the command line and returns the summary (or the dry run).
pub async fn command(
    database_url: Option<String>,
    cache: &Cache,
    args: &ProposeArgs,
    progress: Progress,
) -> Result<String, CliError> {
    let url = database_url.ok_or_else(|| {
        CliError::Config("no database: set DATABASE_URL or pass --database-url".into())
    })?;
    let db = Database::connect(&url).await?;
    let env = |name: &str| std::env::var(name).ok();
    run_command(
        &db,
        cache,
        args,
        progress,
        &env,
        &AnthropicConfig::default(),
    )
    .await
}

/// `propose` against a database, an environment and an API address: the key is read from `env` and only when something
/// is sent.
pub async fn run_command(
    db: &Database,
    cache: &Cache,
    args: &ProposeArgs,
    progress: Progress,
    env: &dyn Fn(&str) -> Option<String>,
    api: &AnthropicConfig,
) -> Result<String, CliError> {
    let cards = stored_cards(db).await?;
    let targets = targets_of(&cards, args)?;
    let options = options_of(args);
    if args.dry_run {
        return dry_run(&cards, &targets, &options);
    }
    if targets.is_empty() {
        return Err(CliError::Config("no card is selected".into()));
    }
    let key = ApiKey::from_env(env)?;
    let client = AnthropicClient::new(
        key,
        api,
        tokio::runtime::Handle::current(),
        progress.clone(),
    )?;
    let store = Store::new(args.out.clone().unwrap_or_else(|| default_out(cache)));
    // The client blocks on the runtime, so the loop runs on a blocking thread.
    let report = tokio::task::spawn_blocking(move || {
        propose_cards(&cards, &targets, &client, &options, &store, &progress)
    })
    .await
    .map_err(|error| CliError::Config(format!("the run stopped: {error}")))??;
    Ok(report.render())
}
