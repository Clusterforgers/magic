//! `propose`: sends a card, the DSL reference and the nearest hand-written files to a model and keeps the proposals the
//! checker accepts (`docs/cards-dsl.md`, Model-assisted reading). The person who runs it supplies the API key and pays.

pub mod anthropic;
pub mod budget;
pub mod c11i_http;
pub mod c11i_openai;
pub mod examples;
pub mod fake;
pub mod forge_hint;
pub mod model;
pub mod outcome;
pub mod prompt;
pub mod select;
pub mod session;
pub mod status;
pub mod store;
pub mod w19b_solved;

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use mtg_cards::PrintedCard;
use mtg_cards::builtin;
use mtg_cards::reference::dsl_reference;
use mtg_db::Database;

use crate::args::{PromoteArgs, ProposeArgs, Provider};
use crate::audit::stored_cards;
use crate::cache::Cache;
use crate::compile::names_of;
use crate::error::CliError;
use crate::progress::Progress;
use crate::w2a_promote::w2a_promote;
use anthropic::{AnthropicClient, AnthropicConfig, ApiKey};
use c11i_openai::{OpenAi, OpenAiClient};
use examples::ExampleIndex;
use forge_hint::{ForgeHints, HintState};
use model::{ModelClient, estimate_tokens};
use outcome::ProposeReport;
use session::{Context, Options, Session};
use store::Store;
use w19b_solved::W19bSolvedExamples;

/// The time between two status lines of a run.
pub const STATUS_EVERY: Duration = Duration::from_secs(30);

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
        solved_examples: 0,
        forge: None,
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
    propose_in(
        &context(cards)?,
        cards,
        targets,
        client,
        options,
        store,
        progress,
    )
}

/// [`propose_cards`] with the context given, so that a run builds it once.
pub fn propose_in(
    ctx: &Context<'_>,
    cards: &[PrintedCard],
    targets: &[usize],
    client: &dyn ModelClient,
    options: &Options,
    store: &Store,
    progress: &Progress,
) -> Result<ProposeReport, CliError> {
    let chosen: Vec<&PrintedCard> = targets.iter().filter_map(|at| cards.get(*at)).collect();
    Session::new(ctx, client, options, store, progress).run(&chosen)
}

/// What a run would send, without sending anything: the prompt of the first card and the estimated size of the run.
pub fn dry_run(
    cards: &[PrintedCard],
    targets: &[usize],
    options: &Options,
) -> Result<String, CliError> {
    dry_run_in(&context(cards)?, cards, targets, options)
}

/// [`dry_run`] with the context given.
pub fn dry_run_in(
    ctx: &Context<'_>,
    cards: &[PrintedCard],
    targets: &[usize],
    options: &Options,
) -> Result<String, CliError> {
    let Some(first) = targets.first().and_then(|at| cards.get(*at)) else {
        return Err(CliError::Config("no card is selected".into()));
    };
    let built = ctx.card_message(first, options.examples);
    let message = built.text;
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
        built.examples
    );
    let hint_line = match built.hint {
        HintState::Off => "",
        HintState::Hinted => "forge hint: the script lines of the first card are in its prompt\n",
        HintState::Unhinted => {
            "forge hint: the first card has no script, or nothing in it to show\n"
        }
    };
    text.push_str(hint_line);
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

/// The options of a run from the command line; a local provider has no default model.
pub fn options_of(args: &ProposeArgs) -> Result<Options, CliError> {
    let model = match (&args.model, args.provider) {
        (Some(model), _) => model.clone(),
        (None, Provider::Anthropic) => DEFAULT_MODEL.to_owned(),
        (None, Provider::Openai) => {
            return Err(CliError::Config(
                "--provider openai needs --model: the id the server knows, such as `gpt-oss:20b`"
                    .into(),
            ));
        }
    };
    Ok(Options {
        model,
        examples: usize::from(args.examples),
        resume: args.resume,
        max_requests: args.max_requests,
        max_input_tokens: args.max_input_tokens,
        jobs: args.jobs,
        status_every: STATUS_EVERY,
    })
}

/// What `--schema` with the Anthropic provider prints: that API takes no schema, so the flag does nothing there.
pub const W3A_SCHEMA_IGNORED: &str = "--schema is ignored: only --provider openai sends a schema; the Anthropic API is asked as usual";

/// What `--reasoning` with the Anthropic provider prints: only an OpenAI-style server is sent the effort.
pub const T18A_REASONING_IGNORED: &str =
    "--reasoning is ignored: only --provider openai sends a reasoning effort";

/// The client of the provider the arguments name, reaching `api` (its address replaced by `--base-url`). The Anthropic
/// key is required; a key for a local server is optional.
pub fn c11i_client_of(
    args: &ProposeArgs,
    env: &dyn Fn(&str) -> Option<String>,
    api: &AnthropicConfig,
    progress: Progress,
) -> Result<Box<dyn ModelClient>, CliError> {
    let handle = tokio::runtime::Handle::current();
    let mut config = api.clone();
    config.timeout = Duration::from_secs(args.request_timeout);
    config.attempts = args.retries.saturating_add(1);
    Ok(match args.provider {
        Provider::Anthropic => {
            if let Some(base) = &args.base_url {
                config.base_url.clone_from(base);
            }
            let key = ApiKey::from_env(env)?;
            if args.schema {
                progress.line(W3A_SCHEMA_IGNORED);
            }
            if args.reasoning.is_some() {
                progress.line(T18A_REASONING_IGNORED);
            }
            Box::new(AnthropicClient::new(key, &config, handle, progress)?)
        }
        Provider::Openai => {
            config.base_url = args
                .base_url
                .clone()
                .unwrap_or_else(|| c11i_openai::C11I_DEFAULT_BASE.to_owned());
            let dialect = OpenAi {
                json_mode: !args.no_json_mode,
                schema: args.schema,
                max_answer_tokens: args.max_answer_tokens,
                reasoning: args.reasoning,
            };
            let key = c11i_openai::c11i_key_from_env(env);
            Box::new(OpenAiClient::new(dialect, key, &config, handle, progress)?)
        }
    })
}

/// The cards the arguments select, as positions in `cards`; a sample draws only from the oracle ids of `within` when given.
pub fn targets_of(
    cards: &[PrintedCard],
    args: &ProposeArgs,
    within: Option<&BTreeSet<String>>,
) -> Result<Vec<usize>, CliError> {
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
        within,
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
    let within = if args.precons {
        Some(precon_oracle_ids(db).await?)
    } else {
        None
    };
    let targets = targets_of(&cards, args, within.as_ref())?;
    let options = options_of(args)?;
    let ctx = w19b_context_of(db, &cards, args).await?;
    if args.dry_run {
        return dry_run_in(&ctx, &cards, &targets, &options);
    }
    if targets.is_empty() {
        return Err(CliError::Config("no card is selected".into()));
    }
    let client = c11i_client_of(args, env, api, progress.clone())?;
    let store = Store::new(args.out.clone().unwrap_or_else(|| default_out(cache)));
    let accepted: Vec<PathBuf> = targets
        .iter()
        .map(|&index| store.accepted_path(&cards[index].name))
        .collect();
    // The client blocks on the runtime, so the loop runs on a blocking thread.
    let report = tokio::task::spawn_blocking(move || {
        propose_in(
            &ctx, &cards, &targets, &*client, &options, &store, &progress,
        )
    })
    .await
    .map_err(|error| CliError::Config(format!("the run stopped: {error}")))??;
    let mut text = report.render();
    if let Some(out) = &args.promote {
        text.push('\n');
        text.push_str(&w2a_promote_run(db, cache, out, accepted).await?);
    }
    Ok(text)
}

/// The oracle ids of the cards the stored precon decks name, the pool of `--precons`.
async fn precon_oracle_ids(db: &Database) -> Result<BTreeSet<String>, CliError> {
    let ids = db.precons().oracle_ids().await?;
    Ok(ids.into_iter().map(|id| id.0).collect())
}

/// The Forge scripts of `--forge-hint DIR`, opened before the first request so that a wrong folder ends the run at once.
pub fn forge_hints_of(args: &ProposeArgs) -> Result<Option<ForgeHints>, CliError> {
    args.forge_hint.as_deref().map(ForgeHints::open).transpose()
}

/// The context of a run; with `--solved-examples K` its examples are the K nearest solved cards of the database, and with
/// `--forge-hint DIR` it holds the Forge scripts.
async fn w19b_context_of(
    db: &Database,
    cards: &[PrintedCard],
    args: &ProposeArgs,
) -> Result<Context<'static>, CliError> {
    let mut ctx = context(cards)?;
    ctx.forge = forge_hints_of(args)?;
    ctx.solved_examples = args.solved_examples;
    if args.solved_examples > 0 {
        let solved =
            W19bSolvedExamples::load(db, args.solved_examples, args.examples_train_only).await?;
        ctx.examples = std::mem::take(&mut ctx.examples).w19b_with_solved(solved);
    }
    Ok(ctx)
}

/// Promotes the accepted proposals a run saved, into the `mtg-cards` crate directory `out`, as `promote` does.
async fn w2a_promote_run(
    db: &Database,
    cache: &Cache,
    out: &Path,
    accepted: Vec<PathBuf>,
) -> Result<String, CliError> {
    let files: Vec<PathBuf> = accepted.into_iter().filter(|path| path.is_file()).collect();
    if files.is_empty() {
        return Ok("nothing to promote".into());
    }
    let args = PromoteArgs {
        dry_run: false,
        partial: false,
        out: Some(out.to_path_buf()),
        files,
    };
    Ok(w2a_promote(db, cache, &args).await?.render())
}
