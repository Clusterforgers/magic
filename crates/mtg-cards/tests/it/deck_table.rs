//! Goal: the deck rules of a table lift exactly the issues of the format's own check that they replace (size, copies, colour identity, the pair of commanders), nothing else, and a report whose legality is replaced follows with its issue count and state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::deck::{Counts, DeckReport, DeckState, DeckZone, Issue, IssueCode, Summary};
use mtg_core::house::{DeckRules, HouseRules};
use rstest::rstest;

const ALL: [IssueCode; 14] = [
    IssueCode::Empty,
    IssueCode::UnknownFormat,
    IssueCode::UnknownCard,
    IssueCode::DeckSize,
    IssueCode::SideboardSize,
    IssueCode::CompanionCount,
    IssueCode::NoCommanderZone,
    IssueCode::CommanderCount,
    IssueCode::CommanderEligibility,
    IssueCode::CommanderPair,
    IssueCode::Copies,
    IssueCode::ColorIdentity,
    IssueCode::Banned,
    IssueCode::NotLegal,
];

fn deck(set: impl FnOnce(&mut DeckRules)) -> HouseRules {
    let mut rules = DeckRules::default();
    set(&mut rules);
    HouseRules {
        deck: rules,
        ..HouseRules::default()
    }
}

#[rstest]
#[case::no_rules(HouseRules::default(), &[])]
#[case::a_deck_size(deck(|d| d.deck_size = Some(60)), &[IssueCode::DeckSize])]
#[case::the_formats_own_deck_size(deck(|d| d.deck_size = Some(100)), &[IssueCode::DeckSize])]
#[case::singleton_off(deck(|d| d.singleton = Some(false)), &[IssueCode::Copies])]
#[case::a_copy_limit(deck(|d| { d.singleton = Some(false); d.max_copies = Some(3); }), &[IssueCode::Copies])]
#[case::singleton_kept(deck(|d| d.singleton = Some(true)), &[])]
#[case::colour_identity_off(deck(|d| d.colour_identity = Some(false)), &[IssueCode::ColorIdentity])]
#[case::colour_identity_kept(deck(|d| d.colour_identity = Some(true)), &[])]
#[case::two_commanders(HouseRules { commanders_per_seat: Some(2), ..HouseRules::default() }, &[IssueCode::CommanderPair])]
#[case::one_commander(HouseRules { commanders_per_seat: Some(1), ..HouseRules::default() }, &[])]
#[case::other_rules_lift_nothing(
    HouseRules { starting_life: Some(30), commander_tax: Some(0), ..HouseRules::default() },
    &[]
)]
fn a_table_lifts_only_the_rules_it_replaces(
    #[case] house: HouseRules,
    #[case] lifted: &[IssueCode],
) {
    for code in ALL {
        assert_eq!(code.lifted_by(&house), lifted.contains(&code), "{code:?}");
        // What a table can lift is a rule a table can change.
        assert!(code.liftable() || !code.lifted_by(&house), "{code:?}");
    }
}

#[test]
fn only_the_deck_rules_of_a_table_can_be_lifted() {
    let liftable: Vec<IssueCode> = ALL.into_iter().filter(|code| code.liftable()).collect();
    assert_eq!(
        liftable,
        [
            IssueCode::DeckSize,
            IssueCode::CommanderPair,
            IssueCode::Copies,
            IssueCode::ColorIdentity
        ]
    );
}

fn issue(code: IssueCode) -> Issue {
    Issue {
        code,
        message: "x".into(),
        zone: Some(DeckZone::Main),
        oracle_id: None,
        name: None,
    }
}

fn report(state: DeckState, counts: Counts, legality: Vec<Issue>) -> DeckReport {
    DeckReport {
        deck_hash: String::new(),
        coverage_hash: String::new(),
        format: "commander".into(),
        summary: Summary {
            format: "commander".into(),
            state,
            counts,
            zones: Vec::new(),
            issues: u32::try_from(legality.len()).unwrap(),
        },
        legality,
        cards: Vec::new(),
    }
}

fn counts(verified: u32, manual: u32, unplayable: u32) -> Counts {
    Counts {
        total: verified + manual + unplayable,
        verified,
        manual,
        unplayable,
        ..Counts::default()
    }
}

#[rstest]
#[case::cleared_and_verified(counts(100, 0, 0), vec![], DeckState::Verified, 0)]
#[case::cleared_with_manual_cards(counts(97, 3, 0), vec![], DeckState::ManualPlay, 0)]
#[case::cleared_with_an_unplayable_card(counts(99, 0, 1), vec![], DeckState::Blocked, 0)]
#[case::one_issue_left(counts(100, 0, 0), vec![IssueCode::Banned], DeckState::Blocked, 1)]
#[case::two_issues_left(counts(97, 3, 0), vec![IssueCode::Banned, IssueCode::DeckSize], DeckState::Blocked, 2)]
fn replacing_the_legality_updates_the_issue_count_and_the_state(
    #[case] counts: Counts,
    #[case] codes: Vec<IssueCode>,
    #[case] state: DeckState,
    #[case] issues: u32,
) {
    // The check of the deck's own format refused it for one issue.
    let mut report = report(DeckState::Blocked, counts, vec![issue(IssueCode::Copies)]);
    let legality: Vec<Issue> = codes.into_iter().map(issue).collect();
    report.set_legality(legality.clone());
    assert_eq!(report.legality, legality);
    assert_eq!(
        (report.summary.state, report.summary.issues),
        (state, issues)
    );
}
