//! Goal: a deck's report counts the distinct cards that wait for a read (`summary.to_read`), says `read` when only approved reads stand beside Verified cards, stays `manual_play` while a card is neither run whole nor read, gives each card the state of the draft that decides it, and changes nothing for a deck without drafts.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use mtg_cards::ability::Ability;
use mtg_cards::card::Card;
use mtg_cards::deck::{
    CardFacts, DeckLine, DeckReport, DeckState, DeckZone, Legality, deck_report,
};
use mtg_cards::effect::Effect;
use mtg_cards::load::{Library, Source, canonical, slug};
use mtg_cards::support::{Caps, PrintedCard, PrintedFace, Support, read_hash};
use mtg_cards::value::Amount;
use mtg_cards::{DraftRules, ReadState};
use mtg_core::color::ColorSet;
use proptest::prelude::*;
use rstest::rstest;

use crate::common::{Corpus, printed};

use DeckZone::{Commander, Main, Sideboard};

/// What a card of a test deck is, by the rules and the draft it has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A basic land: no rules text, so `Verified` without a file.
    Land,
    /// A rules file covering the text, which outranks the approved draft it also has.
    File,
    /// A draft a player approved: `Read`.
    Approved,
    /// A draft nobody judged: played by hand until read.
    Pending,
    /// A draft a player found wrong: played by hand.
    Wrong,
    /// A draft approved before an edit: the verdict is on another hash, so it waits for a read again.
    ReadBeforeAnEdit,
    /// Rules text with no rules and no draft: played by hand.
    Plain,
    /// A card the database does not hold.
    Missing,
}

/// One line of a test deck: the card (`id` tells cards of one kind apart), its zone and copies.
type Entry = (Kind, u32, DeckZone, u32);

fn oracle_id(id: u32) -> String {
    format!("00000000-0000-4000-8000-{id:012}")
}

static BOLT: LazyLock<(Card, PrintedCard)> = LazyLock::new(|| {
    let card = Corpus::repo()
        .library()
        .card_by_name("Lightning Bolt")
        .expect("the corpus has Lightning Bolt")
        .clone();
    let shown = printed()
        .into_iter()
        .find(|card| card.name == "Lightning Bolt")
        .expect("the corpus has Lightning Bolt");
    (card, shown)
});

/// The corpus bolt as card number `id`: a spell the engine runs whole.
fn bolt(id: u32) -> (Card, PrintedCard) {
    let (mut card, mut shown) = BOLT.clone();
    let name = format!("Bolt {id}");
    card.oracle_id = oracle_id(id);
    card.name.clone_from(&name);
    card.faces[0].name.clone_from(&name);
    shown.oracle_id = oracle_id(id);
    shown.name.clone_from(&name);
    shown.faces[0].name = name;
    (card, shown)
}

/// The bolt with another amount of damage: the draft as it was before an edit.
fn edited(mut card: Card) -> Card {
    let Ability::Spell { effects, .. } = &mut card.faces[0].abilities[0] else {
        panic!("the corpus bolt is a spell");
    };
    let Effect::DealDamage { amount, .. } = &mut effects[0] else {
        panic!("the corpus bolt deals damage");
    };
    *amount = Amount::Fixed(4);
    card
}

fn land(id: u32) -> PrintedCard {
    PrintedCard {
        oracle_id: oracle_id(id),
        name: format!("Forest {id}"),
        keywords: vec![],
        layout: "normal".into(),
        faces: vec![PrintedFace {
            name: format!("Forest {id}"),
            type_line: "Basic Land \u{2014} Forest".into(),
            oracle_text: "({T}: Add {G}.)".into(),
            ..PrintedFace::default()
        }],
    }
}

fn plain(id: u32) -> PrintedCard {
    let mut shown = land(id);
    shown.name = format!("Plain {id}");
    shown.faces[0].name = format!("Plain {id}");
    shown.faces[0].type_line = "Sorcery".into();
    shown.faces[0].oracle_text = "Draw two cards, then discard a card.".into();
    shown
}

/// The imported card of each distinct card of a deck; a missing card has none.
fn printed_of(entries: &[Entry]) -> BTreeMap<u32, PrintedCard> {
    let mut cards = BTreeMap::new();
    for &(kind, id, _, _) in entries {
        let shown = match kind {
            Kind::Land => land(id),
            Kind::Plain => plain(id),
            Kind::Missing => continue,
            Kind::File | Kind::Approved | Kind::Pending | Kind::Wrong | Kind::ReadBeforeAnEdit => {
                bolt(id).1
            }
        };
        cards.insert(id, shown);
    }
    cards
}

/// The library a deck's cards stand in: a file for each `File`, a draft row for each card with one.
fn library_of(entries: &[Entry]) -> Library {
    let mut sources = Vec::new();
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    for &(kind, id, _, _) in entries {
        if !seen.insert(id) {
            continue;
        }
        let (card, shown) = bolt(id);
        let (read, hash) = match kind {
            Kind::File | Kind::Approved => (ReadState::Approved, read_hash(&shown, &card)),
            Kind::Pending => (ReadState::Pending, read_hash(&shown, &card)),
            Kind::Wrong => (ReadState::Wrong, read_hash(&shown, &card)),
            Kind::ReadBeforeAnEdit => (
                ReadState::Approved,
                read_hash(&shown, &edited(card.clone())),
            ),
            Kind::Land | Kind::Plain | Kind::Missing => continue,
        };
        if kind == Kind::File {
            sources.push(Source {
                path: format!("cards/{}.json", slug(&card.name)),
                text: canonical(&card),
            });
        }
        rows.push(DraftRules {
            oracle_id: card.oracle_id.clone(),
            read_hash: hash,
            rules: serde_json::to_value(&card).unwrap(),
            read,
        });
    }
    let files = Library::from_sources(&sources).unwrap();
    let (library, left_out) = files.with_drafts(&rows);
    assert!(left_out.is_empty(), "{left_out:?}");
    library
}

fn facts_of(shown: &PrintedCard) -> CardFacts {
    CardFacts {
        printed: shown.clone(),
        color_identity: ColorSet::default(),
        legalities: BTreeMap::from([("freeform".to_owned(), Legality::Legal)]),
    }
}

/// The report of a freeform deck of `entries` in `library`.
fn report_in(library: &Library, entries: &[Entry], caps: &Caps) -> DeckReport {
    let facts: BTreeMap<String, CardFacts> = printed_of(entries)
        .values()
        .map(|shown| (shown.oracle_id.clone(), facts_of(shown)))
        .collect();
    let lines: Vec<DeckLine> = entries
        .iter()
        .map(|&(_, id, zone, count)| DeckLine {
            zone,
            oracle_id: oracle_id(id),
            count,
        })
        .collect();
    deck_report("freeform", &lines, &facts, library, caps)
}

fn report_of(entries: &[Entry]) -> DeckReport {
    report_in(&library_of(entries), entries, &Caps::everything())
}

/// The state, the cards to read and the copies run on a read and played by hand.
fn judged(report: &DeckReport) -> (DeckState, u32, u32, u32) {
    let summary = &report.summary;
    (
        summary.state,
        summary.to_read,
        summary.counts.read,
        summary.counts.manual,
    )
}

#[rstest]
#[case::lands_only(vec![(Kind::Land, 1, Main, 40)], (DeckState::Verified, 0, 0, 0))]
#[case::a_rules_file_outranks_its_approved_draft(
    vec![(Kind::Land, 1, Main, 36), (Kind::File, 2, Main, 4)],
    (DeckState::Verified, 0, 0, 0),
)]
#[case::approved_reads_beside_verified_cards(
    vec![(Kind::Land, 1, Main, 36), (Kind::Approved, 2, Main, 4)],
    (DeckState::Read, 0, 4, 0),
)]
#[case::an_approved_read_in_the_sideboard_only(
    vec![(Kind::Land, 1, Main, 40), (Kind::Approved, 2, Sideboard, 1)],
    (DeckState::Read, 0, 1, 0),
)]
#[case::an_approved_read_in_the_command_zone(
    vec![(Kind::Land, 1, Main, 40), (Kind::Approved, 2, Commander, 1)],
    (DeckState::Read, 0, 1, 0),
)]
#[case::a_deck_of_reads_alone(vec![(Kind::Approved, 1, Main, 60)], (DeckState::Read, 0, 60, 0))]
#[case::a_pending_draft_is_played_by_hand_and_to_read(
    vec![(Kind::Land, 1, Main, 36), (Kind::Pending, 2, Main, 4)],
    (DeckState::ManualPlay, 1, 0, 4),
)]
#[case::a_pending_draft_beside_an_approved_one(
    vec![(Kind::Land, 1, Main, 30), (Kind::Approved, 2, Main, 4), (Kind::Pending, 3, Main, 2)],
    (DeckState::ManualPlay, 1, 4, 2),
)]
#[case::each_waiting_card_counts_once(
    vec![
        (Kind::Pending, 2, Main, 3),
        (Kind::Pending, 2, Sideboard, 2),
        (Kind::Pending, 3, Main, 1),
        (Kind::ReadBeforeAnEdit, 4, Commander, 1),
    ],
    (DeckState::ManualPlay, 3, 0, 7),
)]
#[case::a_read_on_an_older_hash_waits_for_a_new_one(
    vec![(Kind::Land, 1, Main, 39), (Kind::ReadBeforeAnEdit, 2, Main, 1)],
    (DeckState::ManualPlay, 1, 0, 1),
)]
#[case::a_wrong_draft_is_not_to_read(
    vec![(Kind::Land, 1, Main, 39), (Kind::Wrong, 2, Main, 1)],
    (DeckState::ManualPlay, 0, 0, 1),
)]
#[case::a_card_without_a_draft_is_not_to_read(
    vec![(Kind::Land, 1, Main, 39), (Kind::Plain, 2, Main, 1)],
    (DeckState::ManualPlay, 0, 0, 1),
)]
#[case::a_card_by_hand_keeps_a_deck_of_reads_manual(
    vec![(Kind::Approved, 1, Main, 59), (Kind::Plain, 2, Sideboard, 1)],
    (DeckState::ManualPlay, 0, 59, 1),
)]
#[case::an_unknown_card_blocks_a_deck_of_reads(
    vec![(Kind::Approved, 1, Main, 59), (Kind::Missing, 2, Main, 1)],
    (DeckState::Blocked, 0, 59, 0),
)]
#[case::an_unknown_card_blocks_a_deck_with_cards_to_read(
    vec![(Kind::Pending, 1, Main, 59), (Kind::Missing, 2, Main, 1)],
    (DeckState::Blocked, 1, 0, 59),
)]
fn the_state_and_the_cards_to_read_follow_the_cards_of_a_deck(
    #[case] entries: Vec<Entry>,
    #[case] expected: (DeckState, u32, u32, u32),
) {
    let report = report_of(&entries);
    assert_eq!(judged(&report), expected);
    assert_eq!(report.is_clean(), expected.0 == DeckState::Verified);
}

#[rstest]
#[case::a_land(Kind::Land, Support::Verified, None)]
#[case::a_file_with_an_approved_draft(Kind::File, Support::Verified, None)]
#[case::an_approved_draft(Kind::Approved, Support::Read, Some(ReadState::Approved))]
#[case::a_pending_draft(Kind::Pending, Support::Manual, Some(ReadState::Pending))]
#[case::a_wrong_draft(Kind::Wrong, Support::Manual, Some(ReadState::Wrong))]
#[case::a_read_before_an_edit(Kind::ReadBeforeAnEdit, Support::Manual, Some(ReadState::Pending))]
#[case::a_card_with_no_draft(Kind::Plain, Support::Manual, None)]
#[case::an_unknown_card(Kind::Missing, Support::Unplayable, None)]
fn each_card_of_the_report_carries_the_state_of_the_draft_that_decides_it(
    #[case] kind: Kind,
    #[case] support: Support,
    #[case] draft: Option<ReadState>,
) {
    let report = report_of(&[(kind, 7, Main, 2), (kind, 7, Sideboard, 1)]);
    let found: Vec<_> = report
        .cards
        .iter()
        .map(|card| (card.zone, card.count, card.support, card.draft))
        .collect();
    assert_eq!(
        found,
        vec![(Main, 2, support, draft), (Sideboard, 1, support, draft)]
    );
}

#[rstest]
#[case::the_engine_runs_everything(
    Caps::everything(),
    Support::Manual,
    Some(ReadState::Pending),
    1
)]
#[case::the_engine_lacks_the_damage(Caps::from_names(["Ability.Spell"]), Support::Manual, None, 0)]
#[case::the_engine_runs_nothing(Caps::none(), Support::Manual, None, 0)]
fn a_draft_the_engine_cannot_run_waits_for_no_read(
    #[case] caps: Caps,
    #[case] support: Support,
    #[case] draft: Option<ReadState>,
    #[case] to_read: u32,
) {
    let entries = [(Kind::Pending, 1, Main, 4)];
    let report = report_in(&library_of(&entries), &entries, &caps);
    assert_eq!(
        (report.cards[0].support, report.cards[0].draft),
        (support, draft)
    );
    assert_eq!(report.summary.to_read, to_read);
}

#[test]
fn an_approved_draft_the_engine_cannot_run_is_played_by_hand_and_asks_for_nothing() {
    let entries = [(Kind::Land, 1, Main, 36), (Kind::Approved, 2, Main, 4)];
    let caps = Caps::from_names(["Ability.Spell"]);
    let report = report_in(&library_of(&entries), &entries, &caps);
    assert_eq!(judged(&report), (DeckState::ManualPlay, 0, 0, 4));
    let bolt = report
        .cards
        .iter()
        .find(|card| card.oracle_id == oracle_id(2))
        .unwrap();
    assert_eq!((bolt.support, bolt.draft), (Support::Manual, None));
}

#[rstest]
#[case::lands(vec![(Kind::Land, 1, Main, 40)])]
#[case::a_file(vec![(Kind::Land, 1, Main, 36), (Kind::File, 2, Main, 4)])]
#[case::a_card_by_hand(vec![(Kind::Land, 1, Main, 36), (Kind::Plain, 2, Main, 4)])]
#[case::an_unknown_card(vec![(Kind::Land, 1, Main, 36), (Kind::Missing, 2, Sideboard, 1)])]
fn a_deck_without_drafts_reports_as_it_did_and_unrelated_drafts_change_nothing(
    #[case] entries: Vec<Entry>,
) {
    let alone = report_of(&entries);
    let strangers = [
        (Kind::Pending, 90, Main, 1),
        (Kind::Approved, 91, Main, 1),
        (Kind::Wrong, 92, Main, 1),
    ];
    let combined: Vec<Entry> = entries.iter().chain(&strangers).copied().collect();
    let crowded = report_in(&library_of(&combined), &entries, &Caps::everything());

    assert_eq!(crowded, alone);
    assert_eq!(alone.summary.to_read, 0);
    assert_eq!(alone.summary.counts.read, 0);
    assert!(alone.cards.iter().all(|card| card.draft.is_none()));
    assert!(!alone.cards.iter().any(|card| card.support == Support::Read));
    assert_ne!(alone.summary.state, DeckState::Read);
}

#[test]
fn a_verified_deck_keeps_its_counts_and_a_read_card_changes_only_its_own_copies() {
    let verified = report_of(&[(Kind::Land, 1, Main, 36), (Kind::File, 2, Main, 4)]);
    assert_eq!(verified.summary.state, DeckState::Verified);
    assert_eq!(
        (
            verified.summary.counts.total,
            verified.summary.counts.verified
        ),
        (40, 40)
    );
    let on_reads = report_of(&[(Kind::Land, 1, Main, 36), (Kind::Approved, 2, Main, 4)]);
    let counts = on_reads.summary.counts;
    assert_eq!((counts.total, counts.verified, counts.read), (40, 36, 4));
    assert_ne!(on_reads.coverage_hash, verified.coverage_hash);
}

#[rstest]
#[case::reads_only(vec![(Kind::Approved, 1, Main, 60)], DeckState::Read)]
#[case::reads_beside_verified_cards(vec![(Kind::Land, 1, Main, 30), (Kind::Approved, 2, Main, 30)], DeckState::Read)]
#[case::a_card_to_read(vec![(Kind::Land, 1, Main, 30), (Kind::Pending, 2, Main, 30)], DeckState::ManualPlay)]
#[case::verified(vec![(Kind::Land, 1, Main, 60)], DeckState::Verified)]
fn a_table_that_lifts_every_issue_leaves_the_state_the_cards_give(
    #[case] entries: Vec<Entry>,
    #[case] state: DeckState,
) {
    let mut report = report_of(&entries);
    report.set_legality(vec![mtg_cards::deck::Issue {
        code: mtg_cards::deck::IssueCode::Copies,
        message: "too many copies".into(),
        zone: Some(Main),
        oracle_id: None,
        name: None,
    }]);
    assert_eq!(
        (report.summary.state, report.summary.issues),
        (DeckState::Blocked, 1)
    );
    report.set_legality(Vec::new());
    assert_eq!((report.summary.state, report.summary.issues), (state, 0));
}

const KINDS: [Kind; 8] = [
    Kind::Land,
    Kind::File,
    Kind::Approved,
    Kind::Pending,
    Kind::Wrong,
    Kind::ReadBeforeAnEdit,
    Kind::Plain,
    Kind::Missing,
];

proptest! {
    #[test]
    fn the_state_is_the_worst_card_and_to_read_counts_the_waiting_cards_once(
        kinds in proptest::collection::vec(proptest::sample::select(KINDS.to_vec()), 1..6),
        lines in proptest::collection::vec((0usize..6, 0usize..3, 1u32..5), 1..12),
    ) {
        let zones = [Main, Sideboard, Commander];
        let entries: Vec<Entry> = lines
            .iter()
            .map(|&(card, zone, count)| {
                let card = card % kinds.len();
                (kinds[card], u32::try_from(card).unwrap() + 1, zones[zone], count)
            })
            .collect();
        let used: Vec<Kind> = {
            let mut seen = BTreeMap::new();
            for &(kind, id, _, _) in &entries {
                seen.insert(id, kind);
            }
            seen.into_values().collect()
        };
        let waiting = used
            .iter()
            .filter(|kind| matches!(kind, Kind::Pending | Kind::ReadBeforeAnEdit))
            .count();
        let by_hand = |kind: &&Kind| {
            matches!(
                kind,
                Kind::Pending | Kind::Wrong | Kind::ReadBeforeAnEdit | Kind::Plain
            )
        };
        let expected = if used.contains(&Kind::Missing) {
            DeckState::Blocked
        } else if used.iter().any(|kind| by_hand(&kind)) {
            DeckState::ManualPlay
        } else if used.contains(&Kind::Approved) {
            DeckState::Read
        } else {
            DeckState::Verified
        };

        let report = report_of(&entries);
        prop_assert_eq!(report.summary.state, expected);
        prop_assert_eq!(report.summary.to_read as usize, waiting);
        let counts = report.summary.counts;
        let runs_whole = counts.verified + counts.read;
        prop_assert_eq!(counts.total, entries.iter().map(|entry| entry.3).sum::<u32>());
        prop_assert_eq!(
            counts.total,
            runs_whole + counts.partial + counts.manual + counts.stale + counts.unplayable
        );
    }
}
