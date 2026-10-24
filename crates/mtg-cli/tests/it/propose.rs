//! Goal: `propose` over a scripted model: a good proposal is kept, a rejected one gets exactly one repair round with the checker's diagnosis, rejected proposals are saved apart, a budget stops the run before the request that would pass it, `--resume` skips saved cards, a failing API ends or skips as it should, and a dry run shows the prompt and sends nothing.

use std::fs;
use std::path::Path;

use mtg_cards::{PrintedCard, PrintedFace};
use mtg_cli::progress::{Captured, Progress};
use mtg_cli::propose::budget::Stop;
use mtg_cli::propose::fake::ScriptedClient;
use mtg_cli::propose::model::Role;
use mtg_cli::propose::outcome::ProposeReport;
use mtg_cli::propose::session::Options;
use mtg_cli::propose::store::Store;
use mtg_cli::propose::{dry_run, propose_cards, select};

const BOLT: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}]}]}"#;
const FLYING: &str = r#"{"faces":[{"abilities":[{"Keyword":"Flying"}]}]}"#;
const FLYING_AND_HASTE: &str =
    r#"{"faces":[{"abilities":[{"Keyword":"Flying"},{"Keyword":"Haste"}]}]}"#;

fn card(name: &str, type_line: &str, text: &str, keywords: &[&str]) -> PrintedCard {
    PrintedCard {
        oracle_id: format!(
            "00000000-0000-0000-0000-{:012}",
            name.bytes()
                .fold(7u64, |sum, byte| (sum * 31 + u64::from(byte))
                    % 1_000_000_000_000)
        ),
        name: name.to_owned(),
        keywords: keywords.iter().map(|k| (*k).to_owned()).collect(),
        layout: "normal".into(),
        faces: vec![PrintedFace {
            name: name.to_owned(),
            mana_cost: "{2}{U}".into(),
            type_line: type_line.to_owned(),
            oracle_text: text.to_owned(),
            power: Some("2".into()),
            toughness: Some("2".into()),
            ..PrintedFace::default()
        }],
    }
}

fn drake() -> PrintedCard {
    card("Wind Drake", "Creature — Drake", "Flying", &["Flying"])
}

fn options() -> Options {
    Options {
        model: "claude-sonnet-5-5".into(),
        examples: 4,
        resume: false,
        max_requests: 100,
        max_input_tokens: 50_000_000,
    }
}

struct Outcome {
    report: ProposeReport,
    progress: Captured,
}

fn run(
    dir: &Path,
    cards: &[PrintedCard],
    names: &[&str],
    client: &ScriptedClient,
    options: &Options,
) -> Result<Outcome, mtg_cli::error::CliError> {
    let names: Vec<String> = names.iter().map(|name| (*name).to_owned()).collect();
    let targets = select::by_names(cards, &names)?;
    let (progress, captured) = Progress::buffer();
    let report = propose_cards(
        cards,
        &targets,
        client,
        options,
        &Store::new(dir.join("out")),
        &progress,
    )?;
    Ok(Outcome {
        report,
        progress: captured,
    })
}

fn files(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in fs::read_dir(&next).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(relative) = path.strip_prefix(dir) {
                found.push(relative.display().to_string());
            }
        }
    }
    found.sort();
    found
}

#[test]
fn a_good_proposal_is_kept_as_the_completed_rules() {
    let dir = tempfile::tempdir().unwrap();
    let client = ScriptedClient::new().answer("Wind Drake", FLYING);
    let outcome = run(dir.path(), &[drake()], &["Wind Drake"], &client, &options()).unwrap();

    assert_eq!(outcome.report.tally.verified, 1);
    assert_eq!(outcome.report.budget.requests(), 1);
    assert_eq!(files(&dir.path().join("out")), ["wind_drake.json"]);
    let saved = fs::read_to_string(dir.path().join("out/wind_drake.json")).unwrap();
    assert!(saved.contains("\"impl_status\": \"Full\""), "{saved}");
    assert!(saved.contains("\"Keyword\": \"Flying\""), "{saved}");
    assert!(saved.contains("oracle_fingerprint"), "{saved}");
    assert!(
        outcome
            .progress
            .text()
            .contains("Wind Drake: accepted Verified (1 requests)")
    );
    let text = outcome.report.render();
    assert!(
        text.contains("Accepted Verified                1"),
        "{text}"
    );
}

#[test]
fn the_first_prompt_holds_the_reference_the_card_and_asks_for_json_only() {
    let dir = tempfile::tempdir().unwrap();
    let client = ScriptedClient::new().answer("Wind Drake", FLYING);
    run(dir.path(), &[drake()], &["Wind Drake"], &client, &options()).unwrap();

    let requests = client.requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.model, "claude-sonnet-5-5");
    assert!(
        request.system.contains("enum Effect"),
        "the DSL reference is in the system prompt"
    );
    assert!(request.system.contains("Reply with one JSON object"));
    let user = &request.messages[0].text;
    assert!(user.starts_with("Card: Wind Drake\n"));
    assert!(user.contains("Type line: Creature — Drake"));
    assert!(user.contains("Mana cost: {2}{U}"));
    assert!(user.ends_with("Reply with the JSON object only."));
}

#[test]
fn an_invalid_answer_gets_one_repair_round_and_is_kept_when_the_repair_is_good() {
    let dir = tempfile::tempdir().unwrap();
    let client = ScriptedClient::new()
        .answer(
            "Wind Drake",
            r#"{"faces":[{"abilities":[{"Keyword":"Soaring"}]}]}"#,
        )
        .answer("Wind Drake", &format!("```json\n{FLYING}\n```"));
    let outcome = run(dir.path(), &[drake()], &["Wind Drake"], &client, &options()).unwrap();

    assert_eq!(outcome.report.tally.verified, 1);
    assert_eq!(
        (
            outcome.report.budget.requests(),
            outcome.report.budget.repairs()
        ),
        (2, 1)
    );
    assert_eq!(files(&dir.path().join("out")), ["wind_drake.json"]);
    let requests = client.requests();
    let repair = &requests[1];
    let roles: Vec<Role> = repair.messages.iter().map(|m| m.role).collect();
    assert_eq!(roles, [Role::User, Role::Assistant, Role::User]);
    assert!(
        repair.messages[1].text.contains("Soaring"),
        "the model's own answer comes back"
    );
    let diagnosis = &repair.messages[2].text;
    assert!(diagnosis.contains("INVALID"), "{diagnosis}");
    assert!(
        diagnosis.contains("Soaring"),
        "the checker's complaint names the variant: {diagnosis}"
    );
}

#[test]
fn an_answer_that_is_not_json_is_repaired_too() {
    let dir = tempfile::tempdir().unwrap();
    let client = ScriptedClient::new()
        .answer("Wind Drake", "I cannot write rules for this card.")
        .answer("Wind Drake", FLYING);
    let outcome = run(dir.path(), &[drake()], &["Wind Drake"], &client, &options()).unwrap();
    assert_eq!(outcome.report.tally.verified, 1);
    assert!(client.requests()[1].messages[2].text.contains("not JSON"));
}

#[test]
fn a_proposal_that_reads_back_wrong_is_repaired_with_the_clause_that_differs() {
    let dir = tempfile::tempdir().unwrap();
    let client = ScriptedClient::new()
        .answer("Wind Drake", FLYING_AND_HASTE)
        .answer("Wind Drake", FLYING);
    let outcome = run(dir.path(), &[drake()], &["Wind Drake"], &client, &options()).unwrap();

    assert_eq!(outcome.report.tally.verified, 1);
    let diagnosis = &client.requests()[1].messages[2].text;
    assert!(diagnosis.contains("MISMATCH"), "{diagnosis}");
    assert!(diagnosis.contains("stands for no sentence"), "{diagnosis}");
}

#[test]
fn a_second_rejection_ends_the_card_and_is_saved_apart_with_the_report() {
    let dir = tempfile::tempdir().unwrap();
    let client = ScriptedClient::new()
        .answer("Wind Drake", "nonsense")
        .answer("Wind Drake", FLYING_AND_HASTE);
    let outcome = run(dir.path(), &[drake()], &["Wind Drake"], &client, &options()).unwrap();

    assert_eq!(outcome.report.tally.mismatch, 1);
    assert_eq!(client.requests().len(), 2, "one repair round, never more");
    assert_eq!(
        files(&dir.path().join("out")),
        ["rejected/wind_drake.json", "rejected/wind_drake.txt"]
    );
    let json = fs::read_to_string(dir.path().join("out/rejected/wind_drake.json")).unwrap();
    assert!(json.contains("Haste"), "the last answer is kept: {json}");
    let report = fs::read_to_string(dir.path().join("out/rejected/wind_drake.txt")).unwrap();
    assert!(report.starts_with("Wind Drake: MISMATCH"), "{report}");
}

#[test]
fn rules_that_leave_a_clause_to_the_table_are_kept_as_partial() {
    let dir = tempfile::tempdir().unwrap();
    let card = card(
        "Quux Drake",
        "Creature — Drake",
        "Flying\nFrobnicate the quux whenever a wibble gloops.",
        &["Flying"],
    );
    let client = ScriptedClient::new().answer("Quux Drake", FLYING);
    let outcome = run(dir.path(), &[card], &["Quux Drake"], &client, &options()).unwrap();

    assert_eq!(
        (outcome.report.tally.partial, outcome.report.tally.verified),
        (1, 0)
    );
    assert_eq!(files(&dir.path().join("out")), ["quux_drake.json"]);
    let saved = fs::read_to_string(dir.path().join("out/quux_drake.json")).unwrap();
    assert!(
        saved.contains("Manual"),
        "the open clause is left to the table: {saved}"
    );
    assert_eq!(
        client.requests().len(),
        1,
        "an accepted proposal is not repaired"
    );
}

#[test]
fn an_empty_proposal_is_accepted_by_the_checker_but_not_kept() {
    let dir = tempfile::tempdir().unwrap();
    let client = ScriptedClient::new().answer("Wind Drake", r#"{"faces":[{"abilities":[]}]}"#);
    let outcome = run(dir.path(), &[drake()], &["Wind Drake"], &client, &options()).unwrap();
    assert_eq!(outcome.report.tally.empty, 1);
    assert_eq!(
        files(&dir.path().join("out")),
        ["rejected/wind_drake.json", "rejected/wind_drake.txt"]
    );
    assert_eq!(client.requests().len(), 1, "nothing to repair");
}

fn three_cards() -> Vec<PrintedCard> {
    vec![
        card("Alpha Drake", "Creature — Drake", "Flying", &["Flying"]),
        card("Beta Drake", "Creature — Drake", "Flying", &["Flying"]),
        card("Gamma Drake", "Creature — Drake", "Flying", &["Flying"]),
    ]
}

#[test]
fn the_request_budget_stops_the_run_and_the_cards_after_it_are_not_run() {
    let dir = tempfile::tempdir().unwrap();
    let client = ScriptedClient::new()
        .answer("Alpha Drake", "bad")
        .answer("Alpha Drake", FLYING)
        .answer("Beta Drake", "bad")
        .answer("Beta Drake", FLYING)
        .answer("Gamma Drake", FLYING);
    let options = Options {
        max_requests: 3,
        ..options()
    };
    let names = ["Alpha Drake", "Beta Drake", "Gamma Drake"];
    let outcome = run(dir.path(), &three_cards(), &names, &client, &options).unwrap();

    let tally = &outcome.report.tally;
    assert_eq!((tally.verified, tally.invalid, tally.not_run), (1, 1, 1));
    assert_eq!(client.requests().len(), 3, "the budget is never passed");
    assert_eq!(outcome.report.stopped, Some(Stop::Requests(3)));
    let text = outcome.report.render();
    assert!(
        text.contains("stopped: the budget of 3 requests is spent"),
        "{text}"
    );
    assert!(text.contains("requests 3 of 3 (1 repair rounds)"), "{text}");
}

#[test]
fn the_input_token_budget_refuses_a_request_before_it_is_sent() {
    let dir = tempfile::tempdir().unwrap();
    let client = ScriptedClient::new().answer("Wind Drake", FLYING);
    let options = Options {
        max_input_tokens: 1000,
        ..options()
    };
    let outcome = run(dir.path(), &[drake()], &["Wind Drake"], &client, &options).unwrap();

    assert!(
        client.requests().is_empty(),
        "the prompt is about 20 thousand tokens"
    );
    assert_eq!(outcome.report.tally.not_run, 1);
    assert_eq!(outcome.report.stopped, Some(Stop::InputTokens(1000)));
    assert!(!dir.path().join("out").exists());
}

#[test]
fn the_input_token_budget_holds_over_a_run_by_the_tokens_the_answers_cost() {
    let dir = tempfile::tempdir().unwrap();
    let client = ScriptedClient::new()
        .answer("Alpha Drake", FLYING)
        .answer("Beta Drake", FLYING)
        .answer("Gamma Drake", FLYING);
    let first = client_prompt_estimate(&three_cards()[0]);
    // Room for two requests by their estimates, not for a third.
    let options = Options {
        max_input_tokens: first * 2 + first / 2,
        ..options()
    };
    let names = ["Alpha Drake", "Beta Drake", "Gamma Drake"];
    let outcome = run(dir.path(), &three_cards(), &names, &client, &options).unwrap();
    let usage = outcome.report.budget.usage();
    assert!(usage.input_tokens <= options.max_input_tokens, "{usage:?}");
    assert_eq!(outcome.report.tally.verified, 2);
    assert_eq!(outcome.report.tally.not_run, 1);
}

/// The estimate of the first request about a card, from a dry run of the same options.
fn client_prompt_estimate(card: &PrintedCard) -> u64 {
    let text = dry_run(std::slice::from_ref(card), &[0], &options())
        .unwrap_or_else(|e| panic!("the dry run works: {e}"));
    let number = text
        .lines()
        .find_map(|line| line.strip_prefix("estimated input tokens of the first request: "))
        .and_then(|rest| rest.split(' ').next())
        .and_then(|digits| digits.parse().ok());
    number.unwrap_or_else(|| panic!("the dry run states the estimate:\n{text}"))
}

#[test]
fn resume_skips_the_cards_that_have_a_saved_proposal_accepted_or_not() {
    let dir = tempfile::tempdir().unwrap();
    let first = ScriptedClient::new()
        .answer("Alpha Drake", FLYING)
        .answer("Beta Drake", "bad")
        .answer("Beta Drake", "still bad");
    let names = ["Alpha Drake", "Beta Drake", "Gamma Drake"];
    let stopped = Options {
        max_requests: 3,
        ..options()
    };
    run(dir.path(), &three_cards(), &names, &first, &stopped).unwrap();

    let second = ScriptedClient::new().answer("Gamma Drake", FLYING);
    let resumed = Options {
        resume: true,
        ..options()
    };
    let outcome = run(dir.path(), &three_cards(), &names, &second, &resumed).unwrap();

    assert_eq!(outcome.report.tally.skipped, 2);
    assert_eq!(outcome.report.tally.verified, 1);
    let asked: Vec<_> = second
        .requests()
        .iter()
        .filter_map(|r| r.messages[0].text.lines().next().map(str::to_owned))
        .collect();
    assert_eq!(asked, ["Card: Gamma Drake"]);
}

#[test]
fn without_resume_a_saved_card_is_asked_again() {
    let dir = tempfile::tempdir().unwrap();
    let again = ScriptedClient::new()
        .answer("Wind Drake", FLYING)
        .answer("Wind Drake", FLYING);
    run(dir.path(), &[drake()], &["Wind Drake"], &again, &options()).unwrap();
    run(dir.path(), &[drake()], &["Wind Drake"], &again, &options()).unwrap();
    assert_eq!(again.requests().len(), 2);
}

#[test]
fn a_card_whose_request_fails_gets_no_file_and_the_run_goes_on() {
    let dir = tempfile::tempdir().unwrap();
    let client = ScriptedClient::new()
        .failure("Alpha Drake", 500)
        .answer("Beta Drake", FLYING);
    let outcome = run(
        dir.path(),
        &three_cards(),
        &["Alpha Drake", "Beta Drake"],
        &client,
        &options(),
    )
    .unwrap();
    assert_eq!(
        (outcome.report.tally.failed, outcome.report.tally.verified),
        (1, 1)
    );
    assert_eq!(files(&dir.path().join("out")), ["beta_drake.json"]);
}

#[test]
fn a_refused_key_ends_the_run_with_the_error() {
    let dir = tempfile::tempdir().unwrap();
    let client = ScriptedClient::new().failure("Alpha Drake", 401);
    let error = run(
        dir.path(),
        &three_cards(),
        &["Alpha Drake", "Beta Drake"],
        &client,
        &options(),
    )
    .err()
    .unwrap();
    assert_eq!(
        client.requests().len(),
        1,
        "no second request after a refused key"
    );
    assert!(error.to_string().contains("401"), "{error}");
}

#[test]
fn five_cards_in_a_row_without_an_answer_end_the_run() {
    let dir = tempfile::tempdir().unwrap();
    let cards: Vec<PrintedCard> = (0..6)
        .map(|n| {
            card(
                &format!("Drake {n}"),
                "Creature — Drake",
                "Flying",
                &["Flying"],
            )
        })
        .collect();
    let mut client = ScriptedClient::new();
    for card in &cards {
        client = client.failure(&card.name, 503);
    }
    let names: Vec<&str> = cards.iter().map(|c| c.name.as_str()).collect();
    let error = run(dir.path(), &cards, &names, &client, &options())
        .err()
        .unwrap();
    assert_eq!(client.requests().len(), 5);
    assert!(error.to_string().contains("5 cards in a row"), "{error}");
}

#[test]
fn a_dry_run_shows_the_first_prompt_under_a_size_budget_and_has_no_client_to_send_with() {
    let cards = three_cards();
    let text = dry_run(&cards, &[1, 0], &options()).unwrap();

    assert!(text.starts_with("propose --dry-run: nothing is sent\n"));
    assert!(text.contains("cards: 2 (first: Beta Drake)"));
    assert!(
        text.contains("--- card prompt ---\nCard: Beta Drake\n"),
        "{text}"
    );
    assert!(text.contains("[the DSL reference follows:"));
    assert!(
        text.len() < 12_000,
        "the printed prompt is {} bytes",
        text.len()
    );
    // The estimate of the first request, and of the run, are numbers a person can compare with the budget.
    let estimate = client_prompt_estimate(&cards[1]);
    assert!(estimate < 30_000, "a prompt is about {estimate} tokens");
    assert!(
        text.contains("the budget is 50000000 tokens and 100 requests"),
        "{text}"
    );
}

/// A hand-written card as the stored card it is: the Scryfall record of the corpus.
fn corpus_card(slug: &str) -> PrintedCard {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../mtg-cards/tests/fixtures/corpus")
        .join(format!("{slug}.json"));
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{slug} reads: {e}"));
    let record: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{slug} parses: {e}"));
    let text = |key: &str| record[key].as_str().unwrap_or_default().to_owned();
    PrintedCard {
        oracle_id: text("oracle_id"),
        name: text("name"),
        keywords: record["keywords"]
            .as_array()
            .map(|all| {
                all.iter()
                    .filter_map(|k| k.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
        layout: text("layout"),
        faces: vec![PrintedFace {
            name: text("name"),
            mana_cost: text("mana_cost"),
            type_line: text("type_line"),
            oracle_text: text("oracle_text"),
            ..PrintedFace::default()
        }],
    }
}

#[test]
fn the_examples_of_a_burn_spell_are_hand_written_burn_spells_with_their_rules() {
    let slugs = [
        "lightning_bolt",
        "forked_bolt",
        "rift_bolt",
        "demon_bolt",
        "fireball",
        "abbot_of_keral_keep",
        "abundant_growth",
    ];
    let cards: Vec<PrintedCard> = slugs.iter().map(|slug| corpus_card(slug)).collect();
    let client = ScriptedClient::new().answer("Lightning Bolt", BOLT);
    let dir = tempfile::tempdir().unwrap();
    let outcome = run(dir.path(), &cards, &["Lightning Bolt"], &client, &options()).unwrap();
    assert_eq!(outcome.report.tally.verified, 1);

    let user = client.requests()[0].messages[0].text.clone();
    assert_eq!(user.matches("\nExample: ").count(), 4, "{user}");
    assert!(
        !user.contains("Example: Lightning Bolt"),
        "never the card itself"
    );
    for burn in ["Forked Bolt", "Rift Bolt", "Demon Bolt"] {
        assert!(
            user.contains(&format!("Example: {burn} (")),
            "{burn} is near: {user}"
        );
    }
    assert!(!user.contains("Example: Abbot of Keral Keep"), "{user}");
    // Each example brings its rules, as the checker's own JSON.
    assert!(
        user.contains(r#"Rules: {"faces":[{"abilities":["#),
        "{user}"
    );
}

#[test]
fn a_dry_run_with_no_card_is_an_error() {
    assert!(dry_run(&three_cards(), &[], &options()).is_err());
}
