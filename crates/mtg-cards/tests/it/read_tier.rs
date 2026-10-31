//! Goal: a draft counts as `Read` only when a player approved the exact rules and text, by the precedence of the Read tier (a rules file, a Verified compiled row, an approved draft, any other compiled row, nothing), the hash of a read moves with the text and the draft, `Read` is counted apart in a deck and on the wire, and no Verified number of the corpus changes.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::LazyLock;

use crate::common;

use mtg_cards::ability::Ability;
use mtg_cards::card::{Card, ImplStatus, Span};
use mtg_cards::deck::{
    CardFacts, DeckLine, DeckReport, DeckState, DeckZone, Legality, deck_report,
};
use mtg_cards::effect::Effect;
use mtg_cards::generated::{Generation, generate};
use mtg_cards::load::{GeneratedRules, Library, Source, canonical, slug};
use mtg_cards::support::{
    Caps, PrintedCard, PrintedFace, Support, SupportRecord, oracle_fingerprint, read_hash,
};
use mtg_cards::value::Amount;
use mtg_cards::{CardDefinition, DraftRules, ReadState};
use mtg_core::color::ColorSet;
use rstest::rstest;
use serde_json::json;

use common::{Corpus, printed};

const BOLT: &str = "Lightning Bolt";

static BOLT_CARD: LazyLock<(Card, PrintedCard)> = LazyLock::new(|| {
    let card = Corpus::repo()
        .library()
        .card_by_name(BOLT)
        .expect("the corpus has Lightning Bolt")
        .clone();
    let shown = printed()
        .into_iter()
        .find(|card| card.name == BOLT)
        .expect("the corpus has Lightning Bolt");
    (card, shown)
});

fn bolt() -> (Card, PrintedCard) {
    BOLT_CARD.clone()
}

/// How a rules file or a compiled row stands against the imported text.
#[derive(Debug, Clone, Copy)]
enum Shape {
    /// Covers the text and is current.
    Whole,
    /// Runs the damage and leaves a clause to the table.
    Partial,
    /// Declared as no rules at all.
    Manual,
    /// Written for another text.
    Stale,
}

fn shaped(shape: Shape) -> Card {
    let (mut card, _) = bolt();
    match shape {
        Shape::Whole => {}
        Shape::Partial => {
            card.impl_status = ImplStatus::Partial;
            card.faces[0].abilities.push(Ability::Manual {
                reason: "left to the table".into(),
                span: Span { start: 0, end: 5 },
            });
        }
        Shape::Manual => {
            card.impl_status = ImplStatus::Manual;
            card.faces[0].abilities.clear();
        }
        Shape::Stale => {
            card.faces[0].oracle_fingerprint =
                oracle_fingerprint("Lightning Bolt deals 2 damage to any target.");
        }
    }
    card
}

/// Where the rules of the card come from before any draft is added.
#[derive(Debug, Clone, Copy)]
enum Rules {
    Nothing,
    File(Shape),
    Compiled(Shape),
}

fn library_of(rules: Rules) -> Library {
    let (card, _) = bolt();
    match rules {
        Rules::Nothing => Library::default(),
        Rules::File(shape) => {
            let card = shaped(shape);
            let source = Source {
                path: format!("cards/{}.json", slug(&card.name)),
                text: canonical(&card),
            };
            Library::from_sources(&[source]).unwrap()
        }
        Rules::Compiled(shape) => {
            let row = GeneratedRules {
                oracle_id: card.oracle_id.clone(),
                rules: serde_json::to_value(shaped(shape)).unwrap(),
            };
            let (library, left_out) = Library::default().with_generated(&[row]);
            assert!(left_out.is_empty(), "{left_out:?}");
            library
        }
    }
}

/// The bolt with a different amount of damage: the draft as it was before an edit.
fn edited() -> Card {
    let (mut card, _) = bolt();
    let Ability::Spell { effects, .. } = &mut card.faces[0].abilities[0] else {
        panic!("the corpus bolt is a spell");
    };
    let Effect::DealDamage { amount, .. } = &mut effects[0] else {
        panic!("the corpus bolt deals damage");
    };
    *amount = Amount::Fixed(4);
    card
}

/// What a player's verdicts say of the draft, and on which version they were given.
#[derive(Debug, Clone, Copy)]
enum Draft {
    None,
    Pending,
    Approved,
    Wrong,
    /// Approved before the draft was edited: the hash is that of the earlier rules.
    ApprovedBeforeAnEdit,
}

fn draft_rules(draft: Draft, shown: &PrintedCard) -> Option<DraftRules> {
    let (card, _) = bolt();
    let (read, hash) = match draft {
        Draft::None => return None,
        Draft::Pending => (ReadState::Pending, read_hash(shown, &card)),
        Draft::Approved => (ReadState::Approved, read_hash(shown, &card)),
        Draft::Wrong => (ReadState::Wrong, read_hash(shown, &card)),
        Draft::ApprovedBeforeAnEdit => (ReadState::Approved, read_hash(shown, &edited())),
    };
    Some(DraftRules {
        oracle_id: card.oracle_id.clone(),
        read_hash: hash,
        rules: serde_json::to_value(&card).unwrap(),
        read,
    })
}

fn with_draft(library: &Library, draft: Draft, shown: &PrintedCard) -> Library {
    let rows: Vec<DraftRules> = draft_rules(draft, shown).into_iter().collect();
    let (library, left_out) = library.with_drafts(&rows);
    assert!(left_out.is_empty(), "{left_out:?}");
    library
}

#[rstest]
#[case::nothing_and_no_draft(Rules::Nothing, Draft::None, Support::Manual)]
#[case::nothing_and_a_pending_draft(Rules::Nothing, Draft::Pending, Support::Manual)]
#[case::nothing_and_an_approved_draft(Rules::Nothing, Draft::Approved, Support::Read)]
#[case::nothing_and_a_wrong_draft(Rules::Nothing, Draft::Wrong, Support::Manual)]
#[case::nothing_and_a_read_before_an_edit(
    Rules::Nothing,
    Draft::ApprovedBeforeAnEdit,
    Support::Manual
)]
#[case::a_whole_file_outranks_a_draft(
    Rules::File(Shape::Whole),
    Draft::Approved,
    Support::Verified
)]
#[case::a_partial_file_outranks_a_draft(
    Rules::File(Shape::Partial),
    Draft::Approved,
    Support::Partial
)]
#[case::a_manual_file_outranks_a_draft(
    Rules::File(Shape::Manual),
    Draft::Approved,
    Support::Manual
)]
#[case::a_stale_file_outranks_a_draft(Rules::File(Shape::Stale), Draft::Approved, Support::Stale)]
#[case::a_verified_row_outranks_a_draft(
    Rules::Compiled(Shape::Whole),
    Draft::Approved,
    Support::Verified
)]
#[case::an_approved_draft_outranks_a_partial_row(
    Rules::Compiled(Shape::Partial),
    Draft::Approved,
    Support::Read
)]
#[case::an_approved_draft_outranks_a_manual_row(
    Rules::Compiled(Shape::Manual),
    Draft::Approved,
    Support::Read
)]
#[case::an_approved_draft_outranks_a_stale_row(
    Rules::Compiled(Shape::Stale),
    Draft::Approved,
    Support::Read
)]
#[case::a_pending_draft_leaves_a_partial_row(
    Rules::Compiled(Shape::Partial),
    Draft::Pending,
    Support::Partial
)]
#[case::a_wrong_draft_leaves_a_partial_row(
    Rules::Compiled(Shape::Partial),
    Draft::Wrong,
    Support::Partial
)]
#[case::an_old_read_leaves_a_partial_row(
    Rules::Compiled(Shape::Partial),
    Draft::ApprovedBeforeAnEdit,
    Support::Partial
)]
#[case::a_pending_draft_leaves_a_manual_row(
    Rules::Compiled(Shape::Manual),
    Draft::Pending,
    Support::Manual
)]
#[case::a_pending_draft_leaves_a_stale_row(
    Rules::Compiled(Shape::Stale),
    Draft::Pending,
    Support::Stale
)]
fn the_precedence_decides_the_support_of_a_card(
    #[case] rules: Rules,
    #[case] draft: Draft,
    #[case] expected: Support,
) {
    let (_, shown) = bolt();
    let plain = library_of(rules);
    let library = with_draft(&plain, draft, &shown);
    let caps = Caps::everything();

    let record = library.support(&shown, &caps);
    let assembled = library.assemble(&shown, &caps);

    assert_eq!(record.support, expected);
    assert_eq!(assembled.support, record);
    assert_eq!(assembled.definition.read, expected == Support::Read);
    if expected == Support::Read {
        assert_eq!(record.clauses, vec![]);
        assert_eq!(assembled.definition.manual, vec![]);
        assert_eq!(
            assembled.definition.rules.len(),
            1,
            "the draft's spell runs"
        );
    } else {
        // A draft that does not count changes nothing about the card, not even its fingerprint.
        assert_eq!(record, plain.support(&shown, &caps));
    }
}

#[test]
fn an_approved_draft_gives_a_read_record_that_differs_from_a_verified_one_of_the_same_rules() {
    let (card, shown) = bolt();
    let caps = Caps::everything();
    let read = with_draft(&Library::default(), Draft::Approved, &shown).support(&shown, &caps);
    let verified = library_of(Rules::File(Shape::Whole)).support(&shown, &caps);
    assert_eq!(read.support, Support::Read);
    assert_eq!(verified.support, Support::Verified);
    assert_ne!(read.fingerprint, verified.fingerprint);
    assert_eq!(read.oracle_id, card.oracle_id);
}

#[rstest]
#[case::the_engine_runs_everything(Caps::everything(), Support::Read)]
#[case::the_engine_runs_what_the_draft_uses(Caps::from_names(["Ability.Spell", "Effect.DealDamage"]), Support::Read)]
#[case::the_engine_lacks_the_damage(Caps::from_names(["Ability.Spell"]), Support::Manual)]
#[case::the_engine_runs_nothing(Caps::none(), Support::Manual)]
fn an_approved_draft_counts_only_where_the_engine_runs_every_primitive_it_uses(
    #[case] caps: Caps,
    #[case] expected: Support,
) {
    let (_, shown) = bolt();
    let library = with_draft(&Library::default(), Draft::Approved, &shown);
    assert_eq!(library.support(&shown, &caps).support, expected);
}

/// The text of the imported card after an erratum.
fn after_an_erratum(shown: &PrintedCard) -> PrintedCard {
    let mut changed = shown.clone();
    changed.faces[0].oracle_text = "Lightning Bolt deals 4 damage to any target.".into();
    changed
}

#[rstest]
#[case::nobody_read_it(ReadState::Pending, false, ReadState::Pending, Support::Manual)]
#[case::a_player_approved_it(ReadState::Approved, false, ReadState::Approved, Support::Read)]
#[case::a_player_found_it_wrong(ReadState::Wrong, false, ReadState::Wrong, Support::Manual)]
#[case::the_text_changed_after_the_approval(
    ReadState::Approved,
    true,
    ReadState::Pending,
    Support::Manual
)]
#[case::the_text_changed_after_the_wrong(
    ReadState::Wrong,
    true,
    ReadState::Pending,
    Support::Manual
)]
fn a_verdict_counts_only_on_the_hash_of_the_rules_and_the_text_it_was_given_on(
    #[case] stored: ReadState,
    #[case] erratum: bool,
    #[case] counts_as: ReadState,
    #[case] expected: Support,
) {
    let (card, shown) = bolt();
    let row = DraftRules {
        oracle_id: card.oracle_id.clone(),
        read_hash: read_hash(&shown, &card),
        rules: serde_json::to_value(&card).unwrap(),
        read: stored,
    };
    let (library, left_out) = Library::default().with_drafts(&[row]);
    assert!(left_out.is_empty(), "{left_out:?}");
    let now = if erratum {
        after_an_erratum(&shown)
    } else {
        shown
    };

    let draft = library.draft(&card.oracle_id).unwrap();

    assert_eq!(draft.state_for(&now), counts_as);
    assert_eq!(library.support(&now, &Caps::everything()).support, expected);
}

#[test]
fn a_draft_edited_after_its_approval_is_read_again() {
    let (card, shown) = bolt();
    let edited = edited();
    let row = DraftRules {
        oracle_id: card.oracle_id.clone(),
        read_hash: read_hash(&shown, &card),
        rules: serde_json::to_value(&edited).unwrap(),
        read: ReadState::Approved,
    };
    let (library, _) = Library::default().with_drafts(&[row]);

    let draft = library.draft(&card.oracle_id).unwrap();

    assert_eq!(draft.state_for(&shown), ReadState::Pending);
    assert_ne!(read_hash(&shown, &card), read_hash(&shown, &edited));
    assert_eq!(
        library.support(&shown, &Caps::everything()).support,
        Support::Manual
    );
}

#[test]
fn a_draft_written_for_another_text_is_ignored_even_when_its_hash_is_current() {
    let (mut card, shown) = bolt();
    card.faces[0].oracle_fingerprint = oracle_fingerprint("Lightning Bolt deals 2 damage.");
    let row = DraftRules {
        oracle_id: card.oracle_id.clone(),
        read_hash: read_hash(&shown, &card),
        rules: serde_json::to_value(&card).unwrap(),
        read: ReadState::Approved,
    };
    let (library, left_out) = Library::default().with_drafts(&[row]);
    assert!(left_out.is_empty(), "{left_out:?}");
    assert_eq!(
        library.draft(&card.oracle_id).unwrap().state_for(&shown),
        ReadState::Approved
    );
    assert_eq!(
        library.support(&shown, &Caps::everything()).support,
        Support::Manual
    );
}

#[test]
fn the_newest_row_of_a_card_is_its_draft() {
    let (card, shown) = bolt();
    let row = |read, damage_edit: bool| DraftRules {
        oracle_id: card.oracle_id.clone(),
        read_hash: read_hash(&shown, &card),
        rules: serde_json::to_value(if damage_edit { edited() } else { card.clone() }).unwrap(),
        read,
    };
    let (library, left_out) = Library::default()
        .with_drafts(&[row(ReadState::Wrong, true), row(ReadState::Approved, false)]);
    assert!(left_out.is_empty(), "{left_out:?}");
    assert_eq!(
        library.draft(&card.oracle_id).unwrap().read,
        ReadState::Approved
    );
    assert_eq!(
        library.support(&shown, &Caps::everything()).support,
        Support::Read
    );
}

#[rstest]
#[case::a_partial_draft(Shape::Partial, "impl_status")]
#[case::a_manual_draft(Shape::Manual, "impl_status")]
fn a_draft_that_is_not_whole_is_left_out_and_reported(#[case] shape: Shape, #[case] path: &str) {
    let (card, shown) = bolt();
    let row = DraftRules {
        oracle_id: card.oracle_id.clone(),
        read_hash: read_hash(&shown, &card),
        rules: serde_json::to_value(shaped(shape)).unwrap(),
        read: ReadState::Approved,
    };

    let (library, left_out) = Library::default().with_drafts(&[row]);

    assert!(library.draft(&card.oracle_id).is_none());
    assert_eq!(left_out.len(), 1, "{left_out:?}");
    assert_eq!(left_out[0].path, path);
    assert_eq!(
        library.support(&shown, &Caps::everything()).support,
        Support::Manual
    );
}

#[test]
fn a_full_draft_with_a_manual_ability_is_left_out_and_reported() {
    let (card, shown) = bolt();
    let mut full = shaped(Shape::Partial);
    full.impl_status = ImplStatus::Full;
    let row = DraftRules {
        oracle_id: card.oracle_id.clone(),
        read_hash: read_hash(&shown, &full),
        rules: serde_json::to_value(&full).unwrap(),
        read: ReadState::Approved,
    };

    let (library, left_out) = Library::default().with_drafts(&[row]);

    assert!(library.draft(&card.oracle_id).is_none());
    assert_eq!(left_out.len(), 1, "{left_out:?}");
}

#[rstest]
#[case::not_a_card(json!({"nonsense": true}))]
#[case::another_card(json!({"oracle_id": "00000000-0000-4000-8000-000000000001", "name": "Other", "impl_status": "full", "faces": []}))]
fn a_row_that_does_not_load_is_left_out_and_the_others_still_load(
    #[case] rules: serde_json::Value,
) {
    let (card, shown) = bolt();
    let bad = DraftRules {
        oracle_id: card.oracle_id.clone(),
        read_hash: "0".repeat(32),
        rules,
        read: ReadState::Approved,
    };
    let good = DraftRules {
        oracle_id: "00000000-0000-4000-8000-000000000002".into(),
        read_hash: read_hash(&shown, &card),
        rules: serde_json::to_value(&card).unwrap(),
        read: ReadState::Approved,
    };

    let (library, left_out) = Library::default().with_drafts(&[bad, good]);

    assert!(!left_out.is_empty(), "the bad row is reported");
    assert!(library.draft(&card.oracle_id).is_none());
}

#[test]
fn drafts_are_visible_whatever_outranks_them() {
    let (card, shown) = bolt();
    let library = with_draft(
        &library_of(Rules::File(Shape::Whole)),
        Draft::Pending,
        &shown,
    );
    let draft = library.draft(&card.oracle_id).unwrap();
    assert_eq!(draft.read, ReadState::Pending);
    assert_eq!(draft.card, card);
    assert!(
        library
            .draft("00000000-0000-4000-8000-000000000003")
            .is_none()
    );
}

fn digest_of(parts: &[&str]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    hasher.finalize()[..16]
        .iter()
        .fold(String::new(), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

#[test]
fn the_read_hash_is_the_digest_of_the_oracle_id_the_fingerprints_and_the_implementation() {
    let (card, shown) = bolt();
    let fingerprint = oracle_fingerprint(&shown.faces[0].oracle_text);
    let implementation = mtg_cards::support::implementation_hash(&card);
    assert_eq!(
        read_hash(&shown, &card),
        digest_of(&[&shown.oracle_id, &fingerprint, &implementation])
    );
    assert_eq!(read_hash(&shown, &card).len(), 32);
}

#[rstest]
#[case::the_same_text(|p: &mut PrintedCard, _: &mut Card| p.faces[0].oracle_text.push('\n'), true)]
#[case::spacing_only(|p: &mut PrintedCard, _: &mut Card| p.faces[0].oracle_text = p.faces[0].oracle_text.replace(' ', "  "), true)]
#[case::the_name_of_the_card(|p: &mut PrintedCard, c: &mut Card| { p.name = "Other".into(); c.name = "Other".into(); }, true)]
#[case::an_erratum(|p: &mut PrintedCard, _: &mut Card| p.faces[0].oracle_text = "Lightning Bolt deals 4 damage to any target.".into(), false)]
#[case::another_oracle_id(|p: &mut PrintedCard, _: &mut Card| p.oracle_id = "00000000-0000-4000-8000-000000000009".into(), false)]
#[case::a_second_face(|p: &mut PrintedCard, _: &mut Card| p.faces.push(PrintedFace::default()), false)]
#[case::another_ability(|_: &mut PrintedCard, c: &mut Card| c.faces[0].abilities.clear(), false)]
#[case::another_status_only(|_: &mut PrintedCard, c: &mut Card| c.impl_status = ImplStatus::Partial, true)]
fn the_read_hash_moves_with_the_text_and_the_draft_and_with_nothing_else(
    #[case] change: fn(&mut PrintedCard, &mut Card),
    #[case] same: bool,
) {
    let (card, shown) = bolt();
    let (mut other_shown, mut other_card) = (shown.clone(), card.clone());
    change(&mut other_shown, &mut other_card);
    assert_eq!(
        read_hash(&shown, &card) == read_hash(&other_shown, &other_card),
        same
    );
}

fn facts_of(shown: &PrintedCard) -> CardFacts {
    CardFacts {
        printed: shown.clone(),
        color_identity: ColorSet::default(),
        legalities: BTreeMap::from([("freeform".to_owned(), Legality::Legal)]),
    }
}

fn forest() -> PrintedCard {
    PrintedCard {
        oracle_id: "00000000-0000-4000-8000-0000000000f0".into(),
        name: "Forest".into(),
        keywords: vec![],
        layout: "normal".into(),
        faces: vec![PrintedFace {
            name: "Forest".into(),
            type_line: "Basic Land \u{2014} Forest".into(),
            oracle_text: "({T}: Add {G}.)".into(),
            ..PrintedFace::default()
        }],
    }
}

fn report_of(library: &Library, caps: &Caps) -> DeckReport {
    let (_, shown) = bolt();
    let cards: BTreeMap<String, CardFacts> = [&shown, &forest()]
        .into_iter()
        .map(|card| (card.oracle_id.clone(), facts_of(card)))
        .collect();
    let lines = vec![
        DeckLine {
            zone: DeckZone::Main,
            oracle_id: shown.oracle_id.clone(),
            count: 4,
        },
        DeckLine {
            zone: DeckZone::Main,
            oracle_id: forest().oracle_id,
            count: 20,
        },
        DeckLine {
            zone: DeckZone::Sideboard,
            oracle_id: shown.oracle_id.clone(),
            count: 2,
        },
    ];
    deck_report("freeform", &lines, &cards, library, caps)
}

#[test]
fn a_deck_counts_the_copies_of_read_cards_apart_and_is_not_verified_by_them() {
    let (_, shown) = bolt();
    let caps = Caps::everything();
    let read = with_draft(&Library::default(), Draft::Approved, &shown);
    let pending = with_draft(&Library::default(), Draft::Pending, &shown);
    let verified = library_of(Rules::File(Shape::Whole));

    let counts = |library: &Library| report_of(library, &caps).summary.counts;

    let on_a_read = counts(&read);
    assert_eq!(
        (on_a_read.total, on_a_read.verified, on_a_read.read),
        (26, 20, 6)
    );
    assert_eq!((on_a_read.partial, on_a_read.manual), (0, 0));
    let on_a_pending = counts(&pending);
    assert_eq!(
        (
            on_a_pending.verified,
            on_a_pending.read,
            on_a_pending.manual
        ),
        (20, 0, 6)
    );
    let on_a_file = counts(&verified);
    assert_eq!((on_a_file.verified, on_a_file.read), (26, 0));

    let report = report_of(&read, &caps);
    assert_ne!(report.summary.state, DeckState::Verified);
    assert!(!report.is_clean());
    let bolts: Vec<_> = report
        .cards
        .iter()
        .filter(|card| card.oracle_id == shown.oracle_id)
        .map(|card| (card.zone, card.support, card.count))
        .collect();
    assert_eq!(
        bolts,
        vec![
            (DeckZone::Main, Support::Read, 4),
            (DeckZone::Sideboard, Support::Read, 2)
        ]
    );
    let sideboard = report
        .summary
        .zones
        .iter()
        .find(|zone| zone.zone == DeckZone::Sideboard)
        .unwrap();
    assert_eq!((sideboard.counts.read, sideboard.counts.total), (2, 2));
}

#[test]
fn a_read_card_moves_the_coverage_hash_of_a_deck() {
    let (_, shown) = bolt();
    let caps = Caps::everything();
    let plain = report_of(&Library::default(), &caps);
    let read = report_of(
        &with_draft(&Library::default(), Draft::Approved, &shown),
        &caps,
    );
    assert_ne!(plain.coverage_hash, read.coverage_hash);
    assert_eq!(plain.deck_hash, read.deck_hash);
}

#[test]
fn the_wire_words_of_read_round_trip() {
    assert_eq!(serde_json::to_value(Support::Read).unwrap(), json!("read"));
    assert_eq!(
        serde_json::from_value::<Support>(json!("read")).unwrap(),
        Support::Read
    );
    for (state, word) in [
        (ReadState::Pending, "pending"),
        (ReadState::Approved, "approved"),
        (ReadState::Wrong, "wrong"),
    ] {
        assert_eq!(serde_json::to_value(state).unwrap(), json!(word));
        assert_eq!(
            serde_json::from_value::<ReadState>(json!(word)).unwrap(),
            state
        );
    }

    let (_, shown) = bolt();
    let report = report_of(
        &with_draft(&Library::default(), Draft::Approved, &shown),
        &Caps::everything(),
    );
    let wire = serde_json::to_value(&report).unwrap();
    assert_eq!(wire["summary"]["counts"]["read"], json!(6));
    let bolts = wire["cards"].as_array().unwrap().iter();
    let supports: Vec<_> = bolts
        .filter(|card| card["name"] == json!(BOLT))
        .map(|card| card["support"].clone())
        .collect();
    assert_eq!(supports, vec![json!("read"), json!("read")]);
    assert_eq!(serde_json::from_value::<DeckReport>(wire).unwrap(), report);
}

#[test]
fn a_definition_names_its_read_on_the_wire_only_while_it_runs_on_one() {
    let (_, shown) = bolt();
    let caps = Caps::everything();
    let read = with_draft(&Library::default(), Draft::Approved, &shown)
        .assemble(&shown, &caps)
        .definition;
    let verified = library_of(Rules::File(Shape::Whole))
        .assemble(&shown, &caps)
        .definition;

    let (read_json, verified_json) = (
        serde_json::to_value(&read).unwrap(),
        serde_json::to_value(&verified).unwrap(),
    );

    assert_eq!(read_json["read"], json!(true));
    assert!(verified_json.get("read").is_none(), "{verified_json}");
    assert_eq!(
        serde_json::from_value::<CardDefinition>(read_json).unwrap(),
        read
    );
    assert_eq!(
        serde_json::from_value::<CardDefinition>(verified_json).unwrap(),
        verified
    );
    assert!(read.read && !verified.read);
}

fn names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn records(library: &Library, cards: &[PrintedCard], caps: &Caps) -> Vec<SupportRecord> {
    cards
        .iter()
        .map(|card| library.support(card, caps))
        .collect()
}

fn verified_count(records: &[SupportRecord]) -> usize {
    records
        .iter()
        .filter(|record| record.support == Support::Verified)
        .count()
}

/// Every approved draft of the rules a library already holds, as the server would hand them over.
fn approved_copies(cards: &[PrintedCard], library: &Library) -> Vec<DraftRules> {
    cards
        .iter()
        .filter_map(|shown| {
            let card = library.card(&shown.oracle_id)?;
            Some(DraftRules {
                oracle_id: shown.oracle_id.clone(),
                read_hash: read_hash(shown, card),
                rules: serde_json::to_value(card).unwrap(),
                read: ReadState::Approved,
            })
        })
        .collect()
}

#[rstest]
#[case::the_engine_runs_everything(Caps::everything(), 30)]
#[case::the_engine_runs_nothing(Caps::none(), 0)]
#[case::the_engine_runs_a_spell_that_deals_damage(Caps::from_names(["Ability.Spell", "Effect.DealDamage", "Keyword.Flying"]), 0)]
fn approved_drafts_of_the_corpus_change_no_record_of_a_rules_file(
    #[case] caps: Caps,
    #[case] verified_at_least: usize,
) {
    let cards = printed();
    let files = Corpus::repo().library();
    let (with_drafts, left_out) = files.with_drafts(&approved_copies(&cards, &files));
    assert!(left_out.len() < cards.len(), "most corpus files are whole");

    let before = records(&files, &cards, &caps);
    let after = records(&with_drafts, &cards, &caps);

    assert!(
        verified_count(&before) >= verified_at_least,
        "the corpus has Verified cards"
    );
    assert_eq!(before, after, "a rules file outranks every draft");
    assert_eq!(verified_count(&before), verified_count(&after));
    assert!(after.iter().all(|record| record.support != Support::Read));
}

#[test]
fn approved_drafts_never_turn_a_verified_compiled_row_into_anything_else() {
    let cards = printed();
    let (all, tokens) = (
        names(&cards),
        Library::from_sources(&common::token_sources()).unwrap(),
    );
    let rows: Vec<GeneratedRules> = cards
        .iter()
        .filter_map(|shown| match generate(shown, &tokens, &all) {
            Generation::Rules(rules) => Some(GeneratedRules {
                oracle_id: shown.oracle_id.clone(),
                rules: serde_json::to_value(&*rules).unwrap(),
            }),
            _ => None,
        })
        .collect();
    let (compiled, left_out) = tokens.with_generated(&rows);
    assert!(left_out.is_empty(), "{left_out:?}");
    let (drafted, _) = compiled.with_drafts(&approved_copies(&cards, &compiled));
    let caps = Caps::everything();

    let before = records(&compiled, &cards, &caps);
    let after = records(&drafted, &cards, &caps);

    assert!(
        verified_count(&before) > 30,
        "the compiler reads Verified cards"
    );
    assert_eq!(verified_count(&before), verified_count(&after));
    for ((was, now), shown) in before.iter().zip(&after).zip(&cards) {
        match was.support {
            Support::Verified => assert_eq!(was, now, "{}", shown.name),
            _ => assert!(
                now == was || now.support == Support::Read,
                "{}: {:?} became {:?}",
                shown.name,
                was.support,
                now.support
            ),
        }
    }
}
