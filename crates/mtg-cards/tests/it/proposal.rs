//! Goal: a proposed rules file is judged against the printed text and nothing else: valid rules that every clause reads back from are accepted with the support state they give, a wording that differs, an ability with no clause, a construct without a writer and a clause left out each get their own verdict, the checker completes what the card gives (ids, names, status, fingerprints, open clauses as `Manual`) and never changes the state of a hand-written file.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::builtin;
use mtg_cards::card::ImplStatus;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{ClauseState, Report, Verdict, check};
use mtg_cards::support::{PrintedCard, Support};
use rstest::rstest;

use common::{cards_dir, printed, token_sources};

const BOLT: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}]}]}"#;
const BOLT_OF_FOUR: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":4}}]}}]}]}"#;
const BOLT_AND_FLYING: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}},{"Keyword":"Flying"}]}]}"#;
const ANGEL: &str = r#"{"faces":[{"abilities":[{"Keyword":"Flying"},{"Keyword":"Vigilance"}]}]}"#;
const ANGEL_REVERSED: &str =
    r#"{"faces":[{"abilities":[{"Keyword":"Vigilance"},{"Keyword":"Flying"}]}]}"#;
const FLYING_ONLY: &str = r#"{"faces":[{"abilities":[{"Keyword":"Flying"}]}]}"#;
const NOTHING: &str = r#"{"faces":[{"abilities":[]}]}"#;
const MISSPELT: &str = r#"{"faces":[{"abilities":[{"Keyword":"Flyng"}]}]}"#;
/// A draw whose renderers write it ("When ~ enters, draw a card.") on a card whose text says another thing.
const DRAW_ON_ENTERING: &str = r#"{"faces":[{"abilities":[{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Draw":{}}]}}]}]}"#;

fn names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn run_with(name: &str, rules: &str, caps: &Caps) -> Report {
    let cards = printed();
    let tokens = Library::from_sources(&token_sources()).unwrap();
    let card = cards
        .iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("the corpus has {name}"));
    check(rules, card, &tokens, &names(&cards), caps)
}

fn run(name: &str, rules: &str) -> Report {
    run_with(name, rules, &Caps::everything())
}

#[rstest]
#[case::a_spell_read_back("Lightning Bolt", BOLT, Verdict::Accepted, Some(Support::Verified))]
#[case::keywords_read_back("Serra Angel", ANGEL, Verdict::Accepted, Some(Support::Verified))]
#[case::keywords_in_any_order(
    "Serra Angel",
    ANGEL_REVERSED,
    Verdict::Accepted,
    Some(Support::Verified)
)]
#[case::a_clause_left_out_is_partial(
    "Serra Angel",
    FLYING_ONLY,
    Verdict::Accepted,
    Some(Support::Partial)
)]
#[case::nothing_modelled_is_manual(
    "Serra Angel",
    NOTHING,
    Verdict::Accepted,
    Some(Support::Manual)
)]
#[case::other_numbers_than_the_text(
    "Lightning Bolt",
    BOLT_OF_FOUR,
    Verdict::Mismatch,
    Some(Support::Verified)
)]
#[case::an_ability_the_text_lacks(
    "Lightning Bolt",
    BOLT_AND_FLYING,
    Verdict::Mismatch,
    Some(Support::Verified)
)]
#[case::an_event_the_text_lacks(
    "Lightning Bolt",
    DRAW_ON_ENTERING,
    Verdict::Mismatch,
    Some(Support::Partial)
)]
#[case::a_misspelt_variant("Serra Angel", MISSPELT, Verdict::Invalid, None)]
#[case::not_json("Serra Angel", "{", Verdict::Invalid, None)]
#[case::not_an_object("Serra Angel", "[]", Verdict::Invalid, None)]
fn a_proposal_gets_the_verdict_its_reading_earns(
    #[case] card: &str,
    #[case] rules: &str,
    #[case] verdict: Verdict,
    #[case] support: Option<Support>,
) {
    let report = run(card, rules);

    assert_eq!(report.verdict(), verdict, "{}", report.render(true));
    assert_eq!(report.support.map(|record| record.support), support);
}

#[test]
fn a_wording_that_differs_is_shown_beside_the_clause() {
    let report = run("Lightning Bolt", BOLT_OF_FOUR);

    let ClauseState::Differs { written, .. } = &report.clauses[0].state else {
        panic!("{}", report.render(true));
    };
    assert_eq!(written, "~ deals 4 damage to any target.");
    assert!(
        report.render(false).contains("differs"),
        "{}",
        report.render(false)
    );
}

#[test]
fn an_ability_without_a_clause_is_a_stray() {
    let report = run("Lightning Bolt", BOLT_AND_FLYING);

    assert_eq!(report.strays.len(), 1);
    assert_eq!(report.strays[0].construct, "Keyword");
}

#[test]
fn a_clause_with_no_ability_is_left_to_the_table_and_listed_with_the_blocker() {
    let report = run("Serra Angel", FLYING_ONLY);

    assert!(matches!(report.clauses[1].state, ClauseState::Open));
    assert_eq!(report.left().count(), 1);
    let card = report.rules.as_ref().unwrap();
    assert_eq!(card.impl_status, ImplStatus::Partial);
    assert!(matches!(
        card.faces[0].abilities.last(),
        Some(Ability::Manual { .. })
    ));
}

#[test]
fn what_the_engine_lacks_is_a_blocker_of_an_accepted_proposal() {
    let report = run_with("Lightning Bolt", BOLT, &Caps::none());

    assert_eq!(report.verdict(), Verdict::Accepted);
    let record = report.support.as_ref().unwrap();
    assert_eq!(record.support, Support::Manual);
    assert!(
        report
            .render(false)
            .contains("blocker   face 0: the engine does not run"),
        "{}",
        report.render(false)
    );
}

#[test]
fn the_checker_completes_ids_names_status_fingerprints_and_the_printed_order() {
    let report = run("Serra Angel", ANGEL_REVERSED);

    let card = report.rules.as_ref().unwrap();
    assert_eq!(card.name, "Serra Angel");
    assert_eq!(card.impl_status, ImplStatus::Full);
    assert_eq!(card.faces[0].oracle_fingerprint.len(), 32);
    let kinds: Vec<String> = card.faces[0]
        .abilities
        .iter()
        .map(|ability| format!("{ability:?}"))
        .collect();
    assert_eq!(kinds, ["Keyword(Flying)", "Keyword(Vigilance)"]);
}

#[test]
fn the_status_a_file_declares_does_not_count() {
    let lying = r#"{"impl_status":"Full","faces":[{"abilities":[{"Keyword":"Flying"}]}]}"#;

    let report = run("Serra Angel", lying);

    assert_eq!(report.rules.unwrap().impl_status, ImplStatus::Partial);
}

#[test]
fn another_cards_oracle_id_is_invalid_where_it_is_written() {
    let wrong = r#"{"oracle_id":"00000000-0000-4000-8000-000000000000","faces":[{"abilities":[{"Keyword":"Flying"},{"Keyword":"Vigilance"}]}]}"#;

    let report = run("Serra Angel", wrong);

    assert_eq!(report.verdict(), Verdict::Invalid);
    assert!(
        report
            .problems
            .iter()
            .any(|problem| problem.path == "oracle_id"),
        "{:?}",
        report.problems
    );
}

#[test]
fn a_misspelt_variant_is_reported_with_its_path() {
    let report = run("Serra Angel", MISSPELT);

    assert_eq!(report.problems.len(), 1);
    assert!(
        report.problems[0].path.contains("Keyword"),
        "{:?}",
        report.problems
    );
    assert!(report.problems[0].message.contains("unknown variant"));
    assert!(report.rules.is_none());
}

fn hand_written_text(file: &str) -> String {
    std::fs::read_to_string(cards_dir().join(file)).unwrap()
}

#[rstest]
#[case::a_loyalty_ability("Jace Beleren", "jace_beleren.json", "Loyalty")]
#[case::a_modification("Control Magic", "control_magic.json", "SetController")]
#[case::a_rule("Mindlock Orb", "mindlock_orb.json", "Rule")]
fn a_construct_without_a_writer_cannot_be_read_back_and_is_named(
    #[case] card: &str,
    #[case] file: &str,
    #[case] named: &str,
) {
    let report = run(card, &hand_written_text(file));

    assert_eq!(
        report.verdict(),
        Verdict::Unchecked,
        "{}",
        report.render(true)
    );
    let constructs: Vec<&str> = report.unwritten().collect();
    assert!(
        constructs.iter().any(|construct| construct.contains(named)),
        "{constructs:?}"
    );
    assert!(report.support.is_some());
}

/// Every hand-written file, read as a proposal, is valid and never contradicts the text of its card; the checker
/// reports the state the library computes for the file.
#[test]
fn no_hand_written_file_is_invalid_or_mismatches_and_the_state_is_the_librarys() {
    let library = builtin::library().unwrap();
    let cards = printed();
    let all = names(&cards);
    let tokens = Library::from_sources(&token_sources()).unwrap();
    let caps = Caps::everything();
    let mut accepted = 0;
    for card in &cards {
        if library.card(&card.oracle_id).is_none() {
            continue;
        }
        let file = format!("{}.json", slug(&card.name));
        let text = std::fs::read_to_string(cards_dir().join(&file))
            .unwrap_or_else(|e| panic!("{file}: {e}"));
        let report = check(&text, card, &tokens, &all, &caps);
        let verdict = report.verdict();
        assert!(
            matches!(verdict, Verdict::Accepted | Verdict::Unchecked),
            "{}",
            report.render(false)
        );
        assert_eq!(
            report.support.as_ref().map(|record| record.support),
            Some(library.support(card, &caps).support),
            "{}",
            card.name
        );
        accepted += 1;
    }
    assert!(accepted >= 100, "only {accepted} files were checked");
}
