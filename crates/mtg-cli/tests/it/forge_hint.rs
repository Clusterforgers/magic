//! Goal: `propose --forge-hint DIR` over a scripted model and a folder of invented scripts: the script lines of a card that has one reach its prompt in a section of their own and nowhere else, a card without a script is asked as before and counted, the modes are saved beside each proposal, the hint counts against the input budget, and a wrong folder ends the run before any request.

use std::fs;
use std::path::Path;
use std::time::Duration;

use mtg_cards::{PrintedCard, PrintedFace};
use mtg_cli::error::CliError;
use mtg_cli::progress::{Captured, Progress};
use mtg_cli::propose::fake::ScriptedClient;
use mtg_cli::propose::outcome::ProposeReport;
use mtg_cli::propose::session::{Context, Options};
use mtg_cli::propose::store::Store;
use mtg_cli::propose::{context, dry_run_in, forge_hints_of, propose_in, select};

use crate::propose_http::args;

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/forge_hint");
const FLYING: &str = r#"{"faces":[{"abilities":[{"Keyword":"Flying"}]}]}"#;
const HEADING: &str = "Forge script of this card (another engine's reading of the same card)";

fn card(name: &str) -> PrintedCard {
    PrintedCard {
        oracle_id: format!("00000000-0000-0000-0000-{:012}", name.len()),
        name: name.to_owned(),
        keywords: vec!["Flying".into()],
        layout: "normal".into(),
        faces: vec![PrintedFace {
            name: name.to_owned(),
            mana_cost: "{1}{U}".into(),
            type_line: "Creature — Bird".into(),
            oracle_text: "Flying".into(),
            power: Some("1".into()),
            toughness: Some("3".into()),
            ..PrintedFace::default()
        }],
    }
}

/// A card with a script, one whose script has nothing to show, and one with no script.
fn cards() -> Vec<PrintedCard> {
    ["Fenwick Scribe", "Plain Ox", "Unwritten Wisp"]
        .map(card)
        .to_vec()
}

fn options(max_input_tokens: u64) -> Options {
    Options {
        model: "claude-sonnet-5-5".into(),
        examples: 4,
        resume: false,
        max_requests: 100,
        max_input_tokens,
        jobs: 1,
        status_every: Duration::ZERO,
    }
}

/// The context the command builds for these flags (the hand-written examples stand in for the database ones).
fn context_of(flags: &[&str], cards: &[PrintedCard]) -> Result<Context<'static>, CliError> {
    let mut line = vec!["--cards", "Fenwick Scribe"];
    line.extend_from_slice(flags);
    let args = args(&line);
    let mut ctx = context(cards)?;
    ctx.forge = forge_hints_of(&args)?;
    ctx.solved_examples = args.solved_examples;
    Ok(ctx)
}

struct Run {
    report: ProposeReport,
    client: ScriptedClient,
    progress: Captured,
}

fn run(dir: &Path, ctx: &Context<'_>, cards: &[PrintedCard], max_input_tokens: u64) -> Run {
    let client = cards.iter().fold(ScriptedClient::new(), |client, card| {
        client.answer(&card.name, FLYING)
    });
    let names: Vec<String> = cards.iter().map(|card| card.name.clone()).collect();
    let targets = select::by_names(cards, &names).unwrap_or_default();
    let (progress, captured) = Progress::buffer();
    let report = propose_in(
        ctx,
        cards,
        &targets,
        &client,
        &options(max_input_tokens),
        &Store::new(dir.join("out")),
        &progress,
    )
    .unwrap_or_else(|error| panic!("the run works: {error}"));
    Run {
        report,
        client,
        progress: captured,
    }
}

/// The text of the first message of the first request about `name`.
fn prompt_of(run: &Run, name: &str) -> String {
    let marker = format!("Card: {name}\n");
    run.client
        .requests()
        .iter()
        .filter_map(|request| request.messages.first())
        .map(|message| message.text.clone())
        .find(|text| text.starts_with(&marker))
        .unwrap_or_else(|| panic!("no request about {name}"))
}

fn all_text_under(dir: &Path) -> String {
    let mut text = String::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in fs::read_dir(&next).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                text.push_str(&fs::read_to_string(&path).unwrap_or_default());
            }
        }
    }
    text
}

#[test]
fn the_script_lines_of_a_card_are_a_section_of_its_prompt_and_a_card_without_one_is_asked_as_before()
 {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context_of(&["--forge-hint", FIXTURE], &cards()).unwrap();
    let run = run(dir.path(), &ctx, &cards(), 50_000_000);

    let hinted = prompt_of(&run, "Fenwick Scribe");
    let at = |part: &str| {
        hinted
            .find(part)
            .unwrap_or_else(|| panic!("missing {part:?} in\n{hinted}"))
    };
    let order = [
        at("Oracle text:\nFlying"),
        at(HEADING),
        at("K:Flying\nT:Mode$ ChangesZone"),
        at("SVar:TrigInk:DB$ Ink | Amount$ 2"),
        at("Write the rules of Fenwick Scribe now."),
    ];
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{hinted}");
    for left_out in [
        "Name:",
        "ManaCost",
        "Types:",
        "PT:",
        "Oracle:",
        "AIPreference",
        "PlayMain1",
        "DeckHas",
    ] {
        assert!(!hinted.contains(left_out), "{left_out} is in\n{hinted}");
    }
    for plain in ["Plain Ox", "Unwritten Wisp"] {
        let text = prompt_of(&run, plain);
        assert!(
            !text.contains("Forge") && !text.contains("K:Flying"),
            "{text}"
        );
    }

    let tally = &run.report.tally;
    assert_eq!((tally.verified, tally.hinted, tally.unhinted), (3, 1, 2));
    let summary = run.report.render();
    assert!(
        summary.contains("Forge hint: hinted               1\n"),
        "{summary}"
    );
    assert!(
        summary.contains("Forge hint: unhinted             2\n"),
        "{summary}"
    );

    // The hint is a prompt: no line of a script is saved or printed.
    let saved = all_text_under(&dir.path().join("out"));
    for line in ["TrigInk", "ChangesZone", "Singe"] {
        assert!(
            !saved.contains(line) && !run.progress.text().contains(line),
            "{line}"
        );
    }
}

#[test]
fn the_hint_modes_are_saved_for_each_card_so_that_runs_can_be_told_apart() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context_of(
        &["--solved-examples", "0", "--forge-hint", FIXTURE],
        &cards(),
    )
    .unwrap();
    run(dir.path(), &ctx, &cards(), 50_000_000);

    let meta = |slug: &str| -> serde_json::Value {
        let text = fs::read_to_string(dir.path().join(format!("out/meta/{slug}.json")))
            .unwrap_or_else(|error| panic!("{slug}: {error}"));
        serde_json::from_str(&text).unwrap_or_default()
    };
    assert_eq!(
        meta("fenwick_scribe"),
        serde_json::json!({"solved_examples": 0, "forge_hint": "hinted"})
    );
    for slug in ["plain_ox", "unwritten_wisp"] {
        assert_eq!(meta(slug)["forge_hint"], "unhinted", "{slug}");
    }

    // A run of the other arm: solved examples alone are recorded, with the hint off.
    let other = tempfile::tempdir().unwrap();
    let mut solved = context_of(&["--solved-examples", "5"], &cards()).unwrap();
    assert!(solved.forge.is_none());
    solved.solved_examples = 5;
    run(other.path(), &solved, &cards(), 50_000_000);
    let text = fs::read_to_string(other.path().join("out/meta/fenwick_scribe.json")).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&text).unwrap(),
        serde_json::json!({"solved_examples": 5, "forge_hint": "off"})
    );
}

#[test]
fn without_the_flag_the_prompt_the_summary_and_the_folder_are_those_of_before() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context_of(&[], &cards()).unwrap();
    assert!(ctx.forge.is_none());
    let run = run(dir.path(), &ctx, &cards(), 50_000_000);

    assert!(!prompt_of(&run, "Fenwick Scribe").contains("Forge"));
    let summary = run.report.render();
    assert!(!summary.contains("Forge hint"), "{summary}");
    assert_eq!((run.report.tally.hinted, run.report.tally.unhinted), (0, 0));
    assert!(!dir.path().join("out/meta").exists());
    assert!(dir.path().join("out/fenwick_scribe.json").is_file());
}

/// The estimate a dry run states for the first request.
fn estimate(ctx: &Context<'_>) -> u64 {
    let cards = cards();
    let text = dry_run_in(ctx, &cards, &[0], &options(50_000_000))
        .unwrap_or_else(|error| panic!("the dry run works: {error}"));
    text.lines()
        .find_map(|line| line.strip_prefix("estimated input tokens of the first request: "))
        .and_then(|rest| rest.split(' ').next())
        .and_then(|digits| digits.parse().ok())
        .unwrap_or_else(|| panic!("the dry run states the estimate:\n{text}"))
}

#[test]
fn the_hint_counts_against_the_input_budget() {
    let plain = context_of(&[], &cards()).unwrap();
    let hinted = context_of(&["--forge-hint", FIXTURE], &cards()).unwrap();
    let (without, with) = (estimate(&plain), estimate(&hinted));
    assert!(with > without, "{with} > {without}");

    let one = &cards()[..1];
    // Room for the prompt without the hint, not for the one with it.
    let dir = tempfile::tempdir().unwrap();
    let fits = run(&dir.path().join("a"), &plain, one, without);
    assert_eq!(fits.report.tally.verified, 1);
    let refused = run(&dir.path().join("b"), &hinted, one, without);
    assert_eq!(
        (refused.report.tally.verified, refused.report.tally.not_run),
        (0, 1)
    );
    assert_eq!(refused.client.requests().len(), 0);
}

#[test]
fn a_dry_run_shows_the_hint_of_the_first_card_or_says_it_has_none() {
    let hinted = context_of(&["--forge-hint", FIXTURE], &cards()).unwrap();
    let cards = cards();
    let first = dry_run_in(&hinted, &cards, &[0, 2], &options(50_000_000)).unwrap();
    assert!(
        first.contains("forge hint: the script lines of the first card are in its prompt"),
        "{first}"
    );
    assert!(
        first.contains(HEADING) && first.contains("K:Flying"),
        "{first}"
    );
    let second = dry_run_in(&hinted, &cards, &[2, 0], &options(50_000_000)).unwrap();
    assert!(
        second.contains("forge hint: the first card has no script"),
        "{second}"
    );
    assert!(!second.contains(HEADING), "{second}");
    let plain = dry_run_in(
        &context_of(&[], &cards).unwrap(),
        &cards,
        &[0],
        &options(50_000_000),
    )
    .unwrap();
    assert!(!plain.contains("forge hint"), "{plain}");
}

#[test]
fn a_folder_without_scripts_ends_the_run_before_any_request() {
    let dir = tempfile::tempdir().unwrap();
    let empty = dir.path().display().to_string();
    let missing = dir.path().join("nowhere").display().to_string();
    for folder in [empty, missing] {
        let error = context_of(&["--forge-hint", &folder], &cards())
            .err()
            .unwrap_or_else(|| panic!("{folder} is no scripts folder"));
        assert!(error.to_string().contains(&folder), "{error}");
    }
}

#[test]
fn the_flag_is_optional_and_takes_a_folder_beside_zero_or_more_solved_examples() {
    assert_eq!(args(&["--cards", "X"]).forge_hint, None);
    for solved in ["0", "5"] {
        let parsed = args(&[
            "--cards",
            "X",
            "--solved-examples",
            solved,
            "--forge-hint",
            FIXTURE,
        ]);
        assert_eq!(parsed.forge_hint.as_deref(), Some(Path::new(FIXTURE)));
        assert_eq!(parsed.solved_examples.to_string(), solved);
    }
}
