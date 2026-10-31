//! Goal: the ledger groups equal gaps by a reason without names, numbers and ids, frees a card only through its sole blocker, ranks by the popularity freed and writes a stable TSV and table; each kind of blocker comes from the right part of a report.

use super::*;

use crate::check_rules::Subject;
use mtg_cards::card::{Card, ImplStatus, Span};
use mtg_cards::compile::ClauseKind;
use mtg_cards::load::LoadError;
use mtg_cards::proposal::{ClauseReport, Repair, RepairClass, Stray};
use mtg_cards::support::SupportRecord;
use proptest::prelude::*;
use rstest::rstest;

fn blocker(kind: Kind, reason: &str) -> Blocker {
    Blocker {
        kind,
        reason: reason.to_owned(),
    }
}

fn held(name: &str, weight: f64, blockers: &[(Kind, &str)]) -> Held {
    Held {
        name: name.to_owned(),
        weight,
        marks: 0,
        blockers: blockers
            .iter()
            .map(|(kind, reason)| blocker(*kind, reason))
            .collect(),
    }
}

fn names() -> Vec<String> {
    vec!["Fire // Ice".to_owned(), "Fire".to_owned()]
}

#[rstest]
#[case("Fire deals 3 damage", "~ deals N damage")]
#[case("FIRE // ICE deals 3 damage", "~ deals N damage")]
#[case("+1/+1 counter, {2}{R} and 10", "+N/+N counter, {N}{R} and N")]
#[case("id 0a1b2c3d-0000-4abc-8def-0123456789ab here", "id ID here")]
#[case("two  words\n\tand\tmore", "two words and more")]
#[case("faces[0].abilities[12]", "faces[N].abilities[N]")]
#[case("no names or numbers", "no names or numbers")]
#[case("Éire 4", "Éire N")]
#[case("Fireball and Backfire, Fire's 2", "Fireball and Backfire, ~'s N")]
fn the_names_numbers_and_ids_are_taken_out_of_a_reason(#[case] text: &str, #[case] expected: &str) {
    assert_eq!(normalise(text, &names()), expected);
}

#[rstest]
#[case("Spell (effect A; effect B)", &["Spell: effect A", "Spell: effect B"])]
#[case("Static (modification X)", &["Static: modification X"])]
#[case("Activated (cost, timing, condition or limit)", &["Activated: cost, timing, condition or limit"])]
#[case("Keyword", &["Keyword"])]
#[case("Spell (a (nested) part)", &["Spell (a (nested) part)"])]
fn a_construct_is_split_into_the_parts_that_have_no_writer(
    #[case] construct: &str,
    #[case] expected: &[&str],
) {
    assert_eq!(construct_parts(construct), expected);
}

fn valid(
    clauses: Vec<(ClauseState, &str)>,
    strays: &[&str],
    support: Option<(Support, Vec<(&str, u32)>)>,
) -> Report {
    let span = |at: u32| Span {
        start: at,
        end: at + 1,
    };
    Report {
        card: "Fire".to_owned(),
        problems: Vec::new(),
        repairs: Vec::new(),
        clauses: clauses
            .into_iter()
            .zip(0..)
            .map(|((state, text), at)| ClauseReport {
                face: 0,
                span: span(at),
                text: text.to_owned(),
                kind: ClauseKind::Spell,
                state,
            })
            .collect(),
        strays: strays
            .iter()
            .map(|construct| Stray {
                face: 0,
                ability: 3,
                construct: (*construct).to_owned(),
            })
            .collect(),
        support: support.map(|(support, clauses)| SupportRecord {
            oracle_id: "x".to_owned(),
            support,
            clauses: clauses
                .into_iter()
                .map(|(reason, at)| Clause {
                    face: 0,
                    span: span(at),
                    reason: reason.to_owned(),
                })
                .collect(),
            fingerprint: String::new(),
        }),
        rules: Some(Card {
            oracle_id: "x".to_owned(),
            name: "Fire".to_owned(),
            impl_status: ImplStatus::Full,
            faces: Vec::new(),
            notes: None,
        }),
    }
}

fn reads() -> ClauseState {
    ClauseState::Reads { ability: 0 }
}

fn unwritten(construct: &str) -> ClauseState {
    ClauseState::Unwritten {
        ability: 1,
        construct: construct.to_owned(),
    }
}

fn differs(written: &str) -> ClauseState {
    ClauseState::Differs {
        ability: 2,
        written: written.to_owned(),
    }
}

fn found(report: &Report) -> Vec<(&'static str, String)> {
    blockers(report, &names())
        .into_iter()
        .map(|blocker| (blocker.kind.word(), blocker.reason))
        .collect()
}

fn words(pairs: &[(&'static str, &str)]) -> Vec<(&'static str, String)> {
    pairs
        .iter()
        .map(|(kind, reason)| (*kind, (*reason).to_owned()))
        .collect()
}

#[test]
fn an_invalid_proposal_is_held_back_by_the_messages_of_its_problems() {
    let problem = |path: &str, message: &str| LoadError {
        file: "cards/fire.json".to_owned(),
        path: path.to_owned(),
        message: message.to_owned(),
    };
    let mut report = valid(Vec::new(), &[], None);
    report.problems = vec![
        problem("faces[0].abilities[1]", "target 2 does not exist"),
        problem("faces[1].abilities[0]", "target 0 does not exist"),
        problem("", "not JSON: expected value at line 3 column 9"),
    ];
    report.rules = None;

    assert_eq!(
        found(&report),
        words(&[
            ("invalid", "not JSON: expected value at line N column N"),
            ("invalid", "target N does not exist"),
        ])
    );
    let mut unparsed = valid(Vec::new(), &[], None);
    unparsed.rules = None;
    assert_eq!(
        found(&unparsed),
        words(&[("invalid", "the file is not rules")])
    );
}

#[test]
fn an_unchecked_proposal_is_held_back_by_each_part_that_has_no_writer() {
    let report = valid(
        vec![
            (reads(), "Draw a card."),
            (unwritten("Spell (effect Connive; effect Fire)"), "x"),
            (unwritten("Spell (effect Connive)"), "y"),
        ],
        &[],
        Some((Support::Verified, Vec::new())),
    );

    assert_eq!(
        found(&report),
        words(&[
            ("missing-writer", "Spell: effect Connive"),
            ("missing-writer", "Spell: effect Fire"),
        ])
    );
}

#[test]
fn a_card_named_like_a_construct_does_not_change_the_name_of_the_construct() {
    let report = valid(
        vec![
            (unwritten("Spell (effect Connive)"), "x"),
            (
                differs("Connive deals 2 damage."),
                "Connive deals 3 damage.",
            ),
        ],
        &[],
        Some((
            Support::Partial,
            vec![("the engine does not run Effect.Connive yet", 0)],
        )),
    );

    let found = blockers(&report, &["Connive".to_owned()])
        .into_iter()
        .map(|blocker| (blocker.kind.word(), blocker.reason))
        .collect::<Vec<_>>();

    assert_eq!(
        found,
        words(&[
            ("missing-writer", "Spell: effect Connive"),
            (
                "mismatch",
                "wrote \"~ deals N damage.\" for \"~ deals N damage.\""
            ),
            (
                "missing-primitive",
                "the engine does not run Effect.Connive yet"
            ),
        ])
    );
}

#[test]
fn a_mismatch_names_the_wording_written_and_the_clause_and_a_stray_ability() {
    let report = valid(
        vec![(
            differs("Fire deals 4 damage to any target."),
            "Fire deals 3 damage to any target.",
        )],
        &["Static"],
        None,
    );

    assert_eq!(
        found(&report),
        words(&[
            ("mismatch", "Static has no clause in the text"),
            (
                "mismatch",
                "wrote \"~ deals N damage to any target.\" for \"~ deals N damage to any target.\""
            ),
        ])
    );
}

#[test]
fn a_clause_the_game_does_not_run_is_a_missing_primitive_and_an_open_clause_names_its_text() {
    let report = valid(
        vec![
            (reads(), "Draw a card."),
            (ClauseState::Open, "Fire becomes a 4/4 creature."),
            (
                ClauseState::Manual {
                    reason: "you apply this clause by hand".to_owned(),
                },
                "Roll a d20.",
            ),
        ],
        &[],
        Some((
            Support::Partial,
            vec![
                ("the engine does not run Cost.Untap yet", 0),
                ("you apply this clause by hand", 1),
                ("you apply this clause by hand", 2),
            ],
        )),
    );

    assert_eq!(
        found(&report),
        words(&[
            ("missing-primitive", "no rule for: Roll a dN."),
            (
                "missing-primitive",
                "no rule for: ~ becomes a N/N creature."
            ),
            (
                "missing-primitive",
                "the engine does not run Cost.Untap yet"
            ),
        ])
    );
}

#[test]
fn a_verified_proposal_is_held_back_by_nothing() {
    let report = valid(
        vec![(reads(), "Draw a card.")],
        &[],
        Some((Support::Verified, Vec::new())),
    );

    assert!(blockers(&report, &names()).is_empty());
}

const A: (Kind, &str) = (Kind::MissingWriter, "Spell: effect A");
const B: (Kind, &str) = (Kind::MissingWriter, "Spell: effect B");
const C: (Kind, &str) = (Kind::Invalid, "target N does not exist");
const D: (Kind, &str) = (Kind::MissingPrimitive, "the engine does not run X yet");

fn row(gap: &Gap) -> (&str, usize, usize) {
    (gap.blocker.reason.as_str(), gap.blocked, gap.freed)
}

#[test]
fn equal_gaps_group_and_a_card_with_two_blockers_is_not_freed_by_one() {
    let cards = [
        held("One", 0.5, &[A]),
        held("Two", 0.25, &[A, B]),
        held("Three", 0.125, &[B]),
        held("Four", 0.0625, &[A, B, C]),
    ];

    let gaps = rank(&cards);

    // A blocks One, Two and Four but frees only One; B blocks Two, Three and Four and frees Three; C frees none.
    assert_eq!(
        gaps.iter().map(row).collect::<Vec<_>>(),
        [
            ("Spell: effect A", 3, 1),
            ("Spell: effect B", 3, 1),
            ("target N does not exist", 1, 0),
        ]
    );
    assert_eq!(
        gaps.iter().map(|gap| gap.weight).collect::<Vec<_>>(),
        [0.5, 0.125, 0.0]
    );
}

type Cards<'a> = &'a [(&'a str, f64, &'a [(Kind, &'a str)])];

#[rstest]
#[case::weight_before_count(
    &[("a", 0.5, &[A][..]), ("b", 0.25, &[B][..]), ("c", 0.25, &[B][..]), ("d", 0.0, &[C][..])],
    &["Spell: effect B", "Spell: effect A", "target N does not exist"]
)]
#[case::freed_before_blocked_on_equal_weight(
    &[("a", 0.0, &[A][..]), ("b", 0.0, &[B][..]), ("c", 0.0, &[B][..]), ("d", 0.0, &[B, C][..])],
    &["Spell: effect B", "Spell: effect A", "target N does not exist"]
)]
#[case::blocked_before_the_reason(
    &[("a", 0.0, &[A, D][..]), ("b", 0.0, &[A, D][..]), ("c", 0.0, &[B, D][..])],
    &["the engine does not run X yet", "Spell: effect A", "Spell: effect B"]
)]
#[case::kind_then_reason_last(
    &[("a", 0.0, &[D][..]), ("b", 0.0, &[C][..]), ("c", 0.0, &[B][..]), ("d", 0.0, &[A][..])],
    &["Spell: effect A", "Spell: effect B", "target N does not exist", "the engine does not run X yet"]
)]
fn the_gaps_that_free_the_most_popularity_come_first(
    #[case] cards: Cards<'_>,
    #[case] order: &[&str],
) {
    let cards: Vec<Held> = cards
        .iter()
        .map(|(name, weight, blockers)| held(name, *weight, blockers))
        .collect();

    let gaps = rank(&cards);

    assert_eq!(
        gaps.iter()
            .map(|gap| gap.blocker.reason.as_str())
            .collect::<Vec<_>>(),
        order
    );
}

#[test]
fn the_examples_are_the_freed_cards_first_the_most_played_first_and_at_most_five() {
    let mut cards = vec![
        held("Shared", 9.0, &[A, B]),
        held("Rare", 0.5, &[A]),
        held("Common", 2.0, &[A]),
    ];
    cards.extend((0..6).map(|n| held(&format!("Filler {n}"), 0.0, &[A, B])));

    let gaps = rank(&cards);

    let a = &gaps[0];
    assert_eq!(a.blocker.reason, "Spell: effect A");
    assert_eq!(
        a.examples,
        ["Common", "Rare", "Shared", "Filler 0", "Filler 1"]
    );
    assert_eq!((a.blocked, a.freed), (9, 2));
    assert_eq!(a.weight, 2.5);
}

#[test]
fn the_tsv_has_a_header_and_a_tab_separated_row_for_each_gap() {
    let gaps = rank(&[
        held(
            "Sol Ring",
            0.5,
            &[(Kind::Mismatch, "wrote\t\"a\"\nfor \"b\"")],
        ),
        held("Arcane Signet", 0.25, &[A]),
        held("Island", 0.0, &[A]),
    ]);

    let text = tsv(&gaps);

    assert_eq!(
        text,
        "kind\treason\tblocked\tfreed\tweight\texamples\n\
         mismatch\twrote \"a\" for \"b\"\t1\t1\t0.500000\tSol Ring\n\
         missing-writer\tSpell: effect A\t2\t2\t0.250000\tArcane Signet; Island\n"
    );
    for line in text.lines() {
        assert_eq!(line.split('\t').count(), 6, "{line}");
    }
}

#[test]
fn the_table_lists_the_top_rows_and_cuts_a_long_reason() {
    let long = "x".repeat(200);
    let gaps = rank(&[
        held("First", 0.5, &[(Kind::Invalid, long.as_str())]),
        held("Second", 0.25, &[A]),
        held("Third", 0.125, &[B]),
    ]);

    let text = table(&gaps, 2);

    assert!(text.starts_with("gaps (top 2 of 3;"), "{text}");
    assert_eq!(text.lines().count(), 4, "{text}");
    assert!(
        text.contains(&format!("{}... (First)", "x".repeat(WIDTH - 3))),
        "{text}"
    );
    assert!(text.contains("Spell: effect A (Second)"), "{text}");
    assert!(!text.contains("Third"), "{text}");
}

#[test]
fn a_gap_that_frees_nothing_weighs_zero_and_ranks_below_the_gaps_that_free_cards() {
    let gaps = rank(&[held("Two", 0.5, &[A, B]), held("One", 0.0, &[C])]);

    assert!(gaps.iter().all(|gap| gap.weight.is_sign_positive()));
    assert_eq!(
        gaps.iter().map(row).collect::<Vec<_>>(),
        [
            ("target N does not exist", 1, 1),
            ("Spell: effect A", 1, 0),
            ("Spell: effect B", 1, 0),
        ]
    );
    assert!(tsv(&gaps).contains("\t0.000000\tTwo\n"));
}

fn marked(name: &str, weight: f64, marks: u32, blockers: &[(Kind, &str)]) -> Held {
    Held {
        marks,
        ..held(name, weight, blockers)
    }
}

#[test]
fn the_marks_of_the_freed_cards_rank_a_gap_before_its_popularity_and_a_shared_card_adds_none() {
    let gaps = rank(&[
        marked("Popular", 0.5, 0, &[A]),
        marked("Wanted", 0.01, 3, &[B]),
        marked("Wanted too", 0.0, 2, &[B]),
        // Marked, but held by two gaps: freeing neither alone frees it.
        marked("Shared", 0.0, 9, &[C, D]),
    ]);

    assert_eq!(
        gaps.iter()
            .map(|gap| (gap.blocker.reason.as_str(), gap.marks))
            .collect::<Vec<_>>(),
        [
            ("Spell: effect B", 5),
            ("Spell: effect A", 0),
            ("target N does not exist", 0),
            ("the engine does not run X yet", 0),
        ]
    );
}

#[test]
fn marks_add_a_column_to_the_tsv_and_the_table_and_the_plain_output_has_none() {
    let gaps = rank(&[marked("Wanted", 0.25, 3, &[A])]);

    assert_eq!(
        tsv_marked(&gaps),
        "kind\treason\tblocked\tfreed\tmarks\tweight\texamples\n\
         missing-writer\tSpell: effect A\t1\t1\t3\t0.250000\tWanted\n"
    );
    assert_eq!(
        tsv(&gaps),
        "kind\treason\tblocked\tfreed\tweight\texamples\n\
         missing-writer\tSpell: effect A\t1\t1\t0.250000\tWanted\n"
    );
    let with = table_marked(&gaps, TOP);
    let without = table(&gaps, TOP);
    assert!(with.contains("marks: fix marks"), "{with}");
    assert!(!without.contains("marks"), "{without}");
    let header = with.lines().nth(1).unwrap();
    assert!(header.contains("blocked marks   weight"), "{header}");
    assert!(with.lines().nth(2).unwrap().contains("    3 "), "{with}");
    assert_eq!(without.lines().count(), with.lines().count());
}

#[test]
fn without_marks_the_ledger_ranks_by_popularity_as_it_always_did() {
    let cards = [
        held("One", 0.5, &[A]),
        held("Two", 0.25, &[B]),
        held("Three", 0.125, &[C]),
    ];

    let gaps = rank(&cards);

    assert_eq!(
        gaps.iter().map(row).collect::<Vec<_>>(),
        [
            ("Spell: effect A", 1, 1),
            ("Spell: effect B", 1, 1),
            ("target N does not exist", 1, 1),
        ]
    );
    assert!(gaps.iter().all(|gap| gap.marks == 0));
}

#[test]
fn an_empty_ledger_is_a_header_alone() {
    assert_eq!(tsv(&[]), "kind\treason\tblocked\tfreed\tweight\texamples\n");
    assert!(table(&[], TOP).starts_with("gaps (top 0 of 0;"));
}

fn blocker_strategy() -> impl Strategy<Value = (Kind, String)> {
    (
        prop::sample::select(vec![
            Kind::MissingWriter,
            Kind::Invalid,
            Kind::Mismatch,
            Kind::MissingPrimitive,
        ]),
        "[a-c]",
    )
}

proptest! {
    #[test]
    fn the_counts_of_the_ledger_add_up_to_the_cards(
        cards in prop::collection::vec(
            (0u32..40, prop::collection::vec(blocker_strategy(), 1..4)),
            0..12,
        )
    ) {
        let cards: Vec<Held> = cards
            .iter()
            .enumerate()
            .map(|(i, (rank, blockers))| Held {
                name: format!("card {i}"),
                weight: weight_of(*rank),
                marks: 0,
                blockers: blockers
                    .iter()
                    .map(|(kind, reason)| Blocker { kind: *kind, reason: reason.clone() })
                    .collect(),
            })
            .collect();

        let gaps = rank(&cards);

        let sole = cards.iter().filter(|card| card.blockers.len() == 1).count();
        prop_assert_eq!(gaps.iter().map(|gap| gap.freed).sum::<usize>(), sole);
        prop_assert_eq!(
            gaps.iter().map(|gap| gap.blocked).sum::<usize>(),
            cards.iter().map(|card| card.blockers.len()).sum::<usize>()
        );
        for gap in &gaps {
            prop_assert!(gap.freed <= gap.blocked);
            prop_assert!(gap.examples.len() <= EXAMPLES);
            prop_assert!(gap.examples.len() <= gap.blocked);
        }
        for pair in gaps.windows(2) {
            prop_assert!(pair[0].weight >= pair[1].weight);
        }
    }
}

fn repaired(mut report: Report, name: &str, classes: &[RepairClass]) -> Report {
    name.clone_into(&mut report.card);
    report.repairs = classes
        .iter()
        .map(|class| Repair {
            class: *class,
            at: "faces[0]".to_owned(),
            note: "fixed".to_owned(),
        })
        .collect();
    report
}

fn checked_of(reports: Vec<Report>) -> Checked {
    let subjects = (0..reports.len())
        .map(|n| Subject {
            oracle_id: format!("id{n}"),
            names: Vec::new(),
            plain: false,
        })
        .collect();
    Checked {
        reports,
        subjects,
        written: Vec::new(),
    }
}

#[test]
fn repairs_get_a_row_each_with_the_cards_repaired_and_those_nothing_holds_back_now() {
    let free = valid(vec![(reads(), "Fire.")], &[], None);
    let held_back = valid(vec![(differs("Ice."), "Fire.")], &[], None);
    let checked = checked_of(vec![
        repaired(
            free,
            "Free",
            &[RepairClass::ManualSpan, RepairClass::TokenSize],
        ),
        repaired(held_back, "Held", &[RepairClass::ManualSpan]),
        valid(vec![(reads(), "Fire.")], &[], None),
    ]);
    let ranks = Popularity::new(BTreeMap::from([
        ("id0".to_owned(), 1),
        ("id1".to_owned(), 2),
    ]))
    .expect("ranks");
    let rows = repair_rows(&checked, &ranks);
    let shown: Vec<_> = rows
        .iter()
        .map(|row| {
            (
                row.blocker.kind,
                row.blocker.reason.as_str(),
                row.blocked,
                row.freed,
            )
        })
        .collect();
    assert_eq!(
        shown,
        [
            (Kind::Repaired, "unusable Manual span", 2, 1),
            (Kind::Repaired, "token size from the text", 1, 1),
        ]
    );
    assert_eq!(rows[0].examples, ["Free", "Held"]);
    assert_eq!(
        repaired_line(&rows).as_deref(),
        Some("repaired before judging: 2 unusable Manual span, 1 token size from the text")
    );
}

#[rstest]
#[case::a_stray_keyword(&["Keyword"], vec![])]
#[case::an_ability_written_as_other_text(&[], vec![(differs("Fire."), "Fire.")])]
fn a_card_verified_with_no_rules_is_never_held_back_by_a_proposal(
    #[case] strays: &[&str],
    #[case] clauses: Vec<(ClauseState, &str)>,
) {
    let proposal = valid(clauses, strays, None);
    let mut checked = checked_of(vec![proposal.clone(), proposal]);
    checked.subjects[0].plain = true;
    let ranks = Popularity::new(BTreeMap::from([
        ("id0".to_owned(), 1),
        ("id1".to_owned(), 2),
    ]))
    .expect("ranks");
    let held = held_cards(&checked, &ranks, &Marks::new());
    assert_eq!(held.len(), 1, "only the card that needs rules is held");
    assert!(held[0].weight < weight_of(1), "it is the second card");
}

#[test]
fn no_repairs_no_rows_and_no_line() {
    let checked = checked_of(vec![valid(vec![(reads(), "Fire.")], &[], None)]);
    let ranks = Popularity::new(BTreeMap::from([("id0".to_owned(), 1)])).expect("ranks");
    let rows = repair_rows(&checked, &ranks);
    assert_eq!(rows.len(), 0);
    assert_eq!(repaired_line(&rows), None);
}
