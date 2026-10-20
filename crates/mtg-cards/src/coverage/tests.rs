//! Goal: every card lands in the state its rules and the engine's capabilities earn, with the blockers behind it; hand-written rules win over the compiler's; and the report's counts, shares and rankings follow the cards, their copies and their blockers, ties broken the same way every time.
use super::*;
use crate::card::{Face, Span};
use crate::compile::compile;
use crate::keyword::Keyword;
use crate::support::{PrintedFace, oracle_fingerprint};
use rstest::rstest;

fn id(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012}")
}

fn printed(n: u32, name: &str, type_line: &str, text: &str) -> PrintedCard {
    PrintedCard {
        oracle_id: id(n),
        name: name.into(),
        keywords: vec![],
        faces: vec![PrintedFace {
            name: name.into(),
            type_line: type_line.into(),
            oracle_text: text.into(),
            ..PrintedFace::default()
        }],
    }
}

fn in_decks(printed: PrintedCard, copies: &[(&str, u32)]) -> PreconCard {
    PreconCard {
        printed,
        copies: copies
            .iter()
            .map(|&(deck, n)| (deck.to_owned(), n))
            .collect(),
    }
}

/// What the engine of these tests runs: spells that draw, and flying.
fn small_caps() -> Caps {
    Caps::from_names(["Ability.Spell", "Effect.Draw", "Keyword.Flying"])
}

fn analyzed_alone(card: PrintedCard, caps: &Caps) -> CardResult {
    let mut report = analyze_with(&[in_decks(card, &[("deck", 1)])], &Library::default(), caps);
    assert_eq!(report.cards.len(), 1);
    report.cards.remove(0)
}

fn labels(result: &CardResult) -> Vec<String> {
    result.blockers.iter().map(Blocker::label).collect()
}

fn library_of(card: &Card) -> Library {
    Library::from_sources(&[Source {
        path: format!("cards/{}.json", slug(&card.name)),
        text: canonical(card),
    }])
    .unwrap_or_else(|errors| panic!("{errors}"))
}

/// A rules file for card `n`: the compiler's reading of `from`, or no abilities at all.
fn filed(n: u32, name: &str, status: ImplStatus, from: Option<&PrintedCard>) -> Card {
    let compiled = from.and_then(|printed| compile(printed).card);
    Card {
        oracle_id: id(n),
        name: name.into(),
        impl_status: status,
        faces: compiled.map_or_else(
            || {
                vec![Face {
                    name: name.into(),
                    oracle_fingerprint: String::new(),
                    abilities: vec![],
                }]
            },
            |card| card.faces,
        ),
        notes: None,
    }
}

#[rstest]
#[case::vanilla("Creature \u{2014} Bear", "", &[], Support::Verified, RulesSource::Absent, &[])]
#[case::reminder_text_only("Basic Land \u{2014} Forest", "({T}: Add {G}.)", &[], Support::Verified, RulesSource::Absent, &[])]
#[case::read_whole("Sorcery", "Draw a card.", &[], Support::Verified, RulesSource::Compiled, &[])]
#[case::keyword_and_unread_trigger(
    "Creature \u{2014} Angel",
    "Flying\nWhen Test Card enters, tap target creature.",
    &["Flying"],
    Support::Partial,
    RulesSource::Compiled,
    &["triggered: When ~ enters, tap target creature."]
)]
#[case::nothing_read("Instant", "Tap target creature.", &[], Support::Manual, RulesSource::Compiled, &["spell: Tap target creature."])]
#[case::unread_spell_line_leaves_every_line(
    "Sorcery",
    "Draw a card.\nTap target creature.",
    &[],
    Support::Manual,
    RulesSource::Compiled,
    &["spell: Draw a card.", "spell: Tap target creature."]
)]
#[case::numbers_and_mana_are_abstracted(
    "Instant",
    "Tap up to three target creatures. Pay {2}{U}.",
    &[],
    Support::Manual,
    RulesSource::Compiled,
    &["spell: Tap up to N target creatures. Pay {M}."]
)]
#[case::unreadable_keyword("Creature \u{2014} Elf", "Undying", &[], Support::Manual, RulesSource::Compiled, &["keyword: Undying"])]
#[case::keyword_the_engine_lacks("Creature \u{2014} Elf", "Reach", &[], Support::Manual, RulesSource::Compiled, &["engine: Keyword.Reach"])]
#[case::effect_the_engine_lacks("Sorcery", "Scry 2.", &[], Support::Manual, RulesSource::Compiled, &["engine: Effect.Scry"])]
#[case::invalid_compiled_rules("Sorcery", "Draw a card.", &["Flying"], Support::Manual, RulesSource::Compiled, &["rules: invalid"])]
fn a_card_lands_in_the_state_its_rules_earn(
    #[case] type_line: &str,
    #[case] text: &str,
    #[case] keywords: &[&str],
    #[case] state: Support,
    #[case] source: RulesSource,
    #[case] blockers: &[&str],
) {
    let mut card = printed(7, "Test Card", type_line, text);
    card.keywords = keywords.iter().map(|&word| word.to_owned()).collect();
    let result = analyzed_alone(card, &small_caps());
    assert_eq!(result.support, state);
    assert_eq!(result.rules, source);
    assert_eq!(labels(&result), blockers);
    assert_eq!(result.blockers.is_empty(), state == Support::Verified);
}

fn two_faces(n: u32) -> PrintedCard {
    let mut card = printed(n, "Test Front", "Creature \u{2014} Bear", "");
    card.faces.push(PrintedFace {
        name: "Test Back".into(),
        type_line: "Creature \u{2014} Bear".into(),
        ..PrintedFace::default()
    });
    card.name = "Test Front // Test Back".into();
    card
}

#[rstest]
#[case::the_engine_plays_the_front_face_only(small_caps(), Support::Manual, &["printed: the engine plays only the front face of a card"])]
#[case::the_engine_plays_every_face(Caps::from_names(["Card.Faces"]), Support::Verified, &[])]
fn a_back_face_is_a_blocker_until_the_engine_plays_it(
    #[case] caps: Caps,
    #[case] state: Support,
    #[case] blockers: &[&str],
) {
    let result = analyzed_alone(two_faces(2), &caps);
    assert_eq!(result.support, state);
    assert_eq!(labels(&result), blockers);
}

#[rstest]
#[case::x_cost_without_the_capability(small_caps(), Support::Manual, &["printed: the engine cannot pay {X} in a mana cost yet"])]
#[case::x_cost_with_it(Caps::from_names(["Ability.Spell", "Effect.Draw", "Card.XCost"]), Support::Verified, &[])]
fn an_x_cost_is_a_blocker_until_the_engine_pays_it(
    #[case] caps: Caps,
    #[case] state: Support,
    #[case] blockers: &[&str],
) {
    let mut card = printed(2, "Test Draw", "Sorcery", "Draw a card.");
    card.faces[0].mana_cost = "{X}{U}".into();
    let result = analyzed_alone(card, &caps);
    assert_eq!(result.support, state);
    assert_eq!(labels(&result), blockers);
}

#[test]
fn a_card_with_an_unread_clause_and_a_missing_primitive_has_both_blockers() {
    let text = "Flying\nReach\nWhen Test Card enters, tap target creature.";
    let result = analyzed_alone(
        printed(5, "Test Card", "Creature \u{2014} Spider", text),
        &small_caps(),
    );
    assert_eq!(result.support, Support::Partial);
    assert_eq!(
        labels(&result),
        [
            "triggered: When ~ enters, tap target creature.",
            "engine: Keyword.Reach"
        ]
    );
}

#[test]
fn hand_written_rules_win_over_the_compilers() {
    let card = printed(3, "Test Draw", "Sorcery", "Draw a card.");
    let entries = [in_decks(card, &[("deck", 1)])];
    let by_the_compiler = analyze_with(&entries, &Library::default(), &small_caps());
    assert_eq!(
        (
            by_the_compiler.cards[0].support,
            by_the_compiler.cards[0].rules
        ),
        (Support::Verified, RulesSource::Compiled)
    );

    let hand = library_of(&filed(3, "Test Draw", ImplStatus::Manual, None));
    let by_the_file = analyze_with(&entries, &hand, &small_caps());
    let result = &by_the_file.cards[0];
    assert_eq!(
        (result.support, result.rules),
        (Support::Manual, RulesSource::HandWritten)
    );
    assert_eq!(labels(result), ["spell: Draw a card."]);
}

#[test]
fn a_hand_written_clause_is_named_by_the_shape_of_the_text_it_covers_or_by_its_reason() {
    let text = "Flying\nWhen Test Card enters, tap target creature.";
    let card = printed(6, "Test Card", "Creature \u{2014} Angel", text);
    let by_shape = analyze_with(
        &[in_decks(card.clone(), &[("deck", 1)])],
        &library_of(&filed(6, "Test Card", ImplStatus::Partial, Some(&card))),
        &small_caps(),
    );
    assert_eq!(by_shape.cards[0].support, Support::Partial);
    assert_eq!(
        labels(&by_shape.cards[0]),
        ["triggered: When ~ enters, tap target creature."]
    );

    // A clause over text the compiler lists no clause for (a reminder) has only its reason.
    let reminder = "(Remember this.)";
    let odd = printed(7, "Test Odd", "Creature \u{2014} Angel", reminder);
    let mut file = filed(7, "Test Odd", ImplStatus::Partial, None);
    file.faces[0].oracle_fingerprint = oracle_fingerprint(reminder);
    file.faces[0].abilities = vec![
        Ability::Keyword(Keyword::Flying),
        Ability::Manual {
            reason: "the table decides".into(),
            span: Span { start: 1, end: 9 },
        },
    ];
    let by_reason = analyze_with(
        &[in_decks(odd, &[("deck", 1)])],
        &library_of(&file),
        &small_caps(),
    );
    assert_eq!(by_reason.cards[0].support, Support::Partial);
    assert_eq!(labels(&by_reason.cards[0]), ["manual: the table decides"]);
}

#[test]
fn rules_written_for_older_text_are_stale_and_unplayable_cards_say_so() {
    let old = printed(4, "Test Draw", "Sorcery", "Draw a card.");
    let now = printed(4, "Test Draw", "Sorcery", "Draw two cards.");
    let mirror = printed(5, "Test Mirror", "Creature \u{2014} Illusion", "Flying");
    let hand = Library::from_sources(&[
        Source {
            path: "cards/test_draw.json".into(),
            text: canonical(&filed(4, "Test Draw", ImplStatus::Full, Some(&old))),
        },
        Source {
            path: "cards/test_mirror.json".into(),
            text: canonical(&filed(5, "Test Mirror", ImplStatus::Unplayable, None)),
        },
    ])
    .unwrap_or_else(|errors| panic!("{errors}"));
    let entries = [
        in_decks(now, &[("deck", 1)]),
        in_decks(mirror, &[("deck", 2)]),
    ];

    let report = analyze_with(&entries, &hand, &small_caps());

    let seen: Vec<_> = report
        .cards
        .iter()
        .map(|card| (card.support, labels(card), card.rules))
        .collect();
    assert_eq!(
        seen,
        [
            (
                Support::Stale,
                vec!["rules: stale".to_owned()],
                RulesSource::HandWritten
            ),
            (
                Support::Unplayable,
                vec!["rules: unplayable".to_owned()],
                RulesSource::HandWritten
            ),
        ]
    );
}

#[test]
fn the_embedded_library_is_the_hand_written_rules() {
    let bolt = PrintedCard {
        oracle_id: "4457ed35-7c10-48c8-9776-456485fdf070".into(),
        name: "Lightning Bolt".into(),
        keywords: vec![],
        faces: vec![PrintedFace {
            name: "Lightning Bolt".into(),
            mana_cost: "{R}".into(),
            type_line: "Instant".into(),
            oracle_text: "Lightning Bolt deals 3 damage to any target.".into(),
            ..PrintedFace::default()
        }],
    };
    let entries = [in_decks(bolt, &[("deck", 4)])];
    let runs_all = analyze(&entries, &Caps::everything());
    let runs_none = analyze(&entries, &Caps::none());
    let seen = |report: &Report| (report.cards[0].support, report.cards[0].rules);
    assert_eq!(
        seen(&runs_all),
        (Support::Verified, RulesSource::HandWritten)
    );
    assert_eq!(
        seen(&runs_none),
        (Support::Manual, RulesSource::HandWritten)
    );
    assert_eq!(
        labels(&runs_none.cards[0]),
        ["engine: Ability.Spell", "engine: Effect.DealDamage"]
    );
}

#[test]
fn compiled_rules_may_create_the_predefined_tokens_of_the_hand_written_library() {
    let text = "When Test Card enters, create a Treasure token.";
    let card = printed(8, "Test Card", "Creature \u{2014} Pirate", text);
    let entries = [in_decks(card, &[("deck", 1)])];
    let seen = |report: &Report| (report.cards[0].support, labels(&report.cards[0]));

    let with_tokens = analyze(&entries, &Caps::everything());
    let without = analyze_with(&entries, &Library::default(), &Caps::everything());

    assert_eq!(seen(&with_tokens), (Support::Verified, vec![]));
    assert_eq!(
        seen(&without),
        (Support::Manual, vec!["rules: invalid".to_owned()])
    );
}

#[test]
fn compiled_rules_may_name_another_analyzed_card() {
    let partner = |n: u32, name: &str, other: &str| {
        let text = format!("Partner with {other}");
        printed(n, name, "Legendary Creature \u{2014} Elf", &text)
    };
    let (silvar, trynn) = (
        partner(1, "Test Silvar", "Test Trynn"),
        partner(2, "Test Trynn", "Test Silvar"),
    );
    let both = analyze_with(
        &[
            in_decks(silvar.clone(), &[("deck", 1)]),
            in_decks(trynn, &[("deck", 1)]),
        ],
        &Library::default(),
        &Caps::everything(),
    );
    let alone = analyze_with(
        &[in_decks(silvar, &[("deck", 1)])],
        &Library::default(),
        &Caps::everything(),
    );

    assert_eq!(
        both.cards
            .iter()
            .map(|card| card.support)
            .collect::<Vec<_>>(),
        [Support::Verified, Support::Verified]
    );
    assert_eq!(labels(&alone.cards[0]), ["rules: invalid"]);
}

#[test]
fn copies_add_up_by_deck_and_one_oracle_id_is_one_card() {
    let card = printed(1, "Test Draw", "Sorcery", "Draw a card.");
    let entries = [
        in_decks(card.clone(), &[("x", 2), ("y", 1)]),
        in_decks(card, &[("y", 3)]),
    ];
    let report = analyze_with(&entries, &Library::default(), &small_caps());
    assert_eq!(report.cards.len(), 1);
    assert_eq!(
        report.cards[0].copies,
        BTreeMap::from([("x".to_owned(), 2), ("y".to_owned(), 4)])
    );
    assert_eq!(report.cards[0].total_copies(), 6);
}

/// Cards in three decks: two verified, one partial, one manual, one unplayable.
fn sample_entries() -> Vec<PreconCard> {
    let angel = "Flying\nWhen Test Angel enters, tap target creature.";
    vec![
        in_decks(
            printed(1, "Test Bears", "Creature \u{2014} Bear", ""),
            &[("C1 Alpha", 10), ("C2 Beta", 5), ("C3 Gamma", 1)],
        ),
        in_decks(
            printed(2, "Test Draw", "Sorcery", "Draw a card."),
            &[("C1 Alpha", 1)],
        ),
        in_decks(
            printed(3, "Test Freeze", "Instant", "Tap target creature."),
            &[("C1 Alpha", 2), ("C2 Beta", 1), ("C3 Gamma", 3)],
        ),
        in_decks(
            printed(4, "Test Angel", "Creature \u{2014} Angel", angel),
            &[("C2 Beta", 1)],
        ),
        in_decks(
            printed(9, "Test Mirror", "Creature \u{2014} Illusion", "Flying"),
            &[("C2 Beta", 2)],
        ),
    ]
}

fn sample_report() -> Report {
    let hand = library_of(&filed(9, "Test Mirror", ImplStatus::Unplayable, None));
    analyze_with(&sample_entries(), &hand, &small_caps())
}

#[test]
fn the_tally_counts_cards_and_copies_by_state() {
    let tally: Vec<_> = sample_report()
        .tally()
        .into_iter()
        .map(|row| (row.bucket, row.cards, row.copies))
        .collect();
    assert_eq!(
        tally,
        [
            (Bucket::Verified, 2, 17),
            (Bucket::Partial, 1, 1),
            (Bucket::Manual, 1, 6),
            (Bucket::Other, 1, 2),
        ]
    );
}

fn deck(label: &str, verified: u32, copies: u32) -> DeckShare {
    DeckShare {
        deck: label.to_owned(),
        copies,
        verified,
    }
}

fn held(name: &str, support: Support, copies: &[(&str, u32)]) -> CardResult {
    CardResult {
        support,
        copies: copies
            .iter()
            .map(|&(label, n)| (label.to_owned(), n))
            .collect(),
        ..blocked(name, &[])
    }
}

#[test]
fn decks_rank_by_the_share_of_their_copies_that_are_verified() {
    assert_eq!(
        sample_report().decks(),
        [
            deck("C1 Alpha", 11, 13),
            deck("C2 Beta", 5, 9),
            deck("C3 Gamma", 1, 4),
        ]
    );
}

#[test]
fn equal_shares_tie_by_label_whatever_the_deck_sizes() {
    let report = Report {
        cards: vec![
            held(
                "Bears",
                Support::Verified,
                &[("alpha", 2), ("beta", 1), ("delta", 1)],
            ),
            held("Draw", Support::Verified, &[("beta", 1)]),
            held(
                "Freeze",
                Support::Manual,
                &[("alpha", 2), ("gamma", 1), ("delta", 1)],
            ),
        ],
    };
    assert_eq!(
        report.decks(),
        [
            deck("beta", 2, 2),
            deck("alpha", 2, 4),
            deck("delta", 1, 2),
            deck("gamma", 0, 1),
        ]
    );
}

fn blocked(name: &str, blockers: &[&str]) -> CardResult {
    CardResult {
        oracle_id: name.into(),
        name: name.into(),
        rules: RulesSource::Compiled,
        support: Support::Manual,
        blockers: blockers
            .iter()
            .map(|&label| Blocker::Clause(label.to_owned()))
            .collect(),
        copies: BTreeMap::from([("deck".to_owned(), 1)]),
    }
}

fn counted(counts: &[BlockerCount]) -> Vec<(String, u32, u32)> {
    counts
        .iter()
        .map(|count| (count.blocker.label(), count.alone, count.in_all))
        .collect()
}

fn ranking_report() -> Report {
    let cards = [
        ("q1", &["q"][..]),
        ("q2", &["q"]),
        ("q3", &["q"]),
        ("p1", &["p"]),
        ("pr1", &["p", "r"]),
        ("pr2", &["p", "r"]),
        ("pr3", &["p", "r"]),
        ("pr4", &["p", "r"]),
        ("rq", &["r", "q"]),
        ("t1", &["m"]),
        ("t2", &["n"]),
    ];
    Report {
        cards: cards
            .iter()
            .map(|(name, blockers)| blocked(name, blockers))
            .collect(),
    }
}

#[test]
fn blockers_rank_by_the_cards_they_hold_back_in_all_then_alone_then_name() {
    let report = ranking_report();
    let want = |rows: &[(&str, u32, u32)]| -> Vec<(String, u32, u32)> {
        rows.iter()
            .map(|&(label, alone, in_all)| (label.to_owned(), alone, in_all))
            .collect()
    };
    assert_eq!(
        counted(&report.blocker_counts()),
        want(&[
            ("p", 1, 5),
            ("r", 0, 5),
            ("q", 3, 4),
            ("m", 1, 1),
            ("n", 1, 1)
        ])
    );
    assert_eq!(
        counted(&report.top_all(2)),
        want(&[("p", 1, 5), ("r", 0, 5)])
    );
}

#[test]
fn blockers_rank_alone_by_the_cards_only_they_hold_back_and_skip_those_that_never_do() {
    let report = ranking_report();
    let want = |rows: &[(&str, u32, u32)]| -> Vec<(String, u32, u32)> {
        rows.iter()
            .map(|&(label, alone, in_all)| (label.to_owned(), alone, in_all))
            .collect()
    };
    assert_eq!(
        counted(&report.top_alone(10)),
        want(&[("q", 3, 4), ("p", 1, 5), ("m", 1, 1), ("n", 1, 1)])
    );
    assert_eq!(counted(&report.top_alone(1)), want(&[("q", 3, 4)]));
    assert_eq!(report.top_alone(0).len(), 0);
}

#[test]
fn a_report_of_no_cards_ranks_nothing() {
    let report = Report::default();
    assert_eq!(report.decks().len(), 0);
    assert!(report.top_alone(20).is_empty() && report.top_all(20).is_empty());
    assert!(report.tally().iter().all(|row| row.cards == 0));
}

#[test]
fn the_text_of_a_small_report_is_fixed() {
    let expected = "\
coverage: 3 precons, 5 distinct cards, 26 copies

state      cards  share   copies  share
verified       2  40.0%       17  65.4%
partial        1  20.0%        1   3.8%
manual         1  20.0%        6  23.1%
other          1  20.0%        2   7.7%

best precons by the share of their cards that are verified
   84.6%     11/13  C1 Alpha
   55.6%       5/9  C2 Beta

worst precons by the share of their cards that are verified
   25.0%       1/4  C3 Gamma

blockers that alone hold back the most cards (alone: cards with no other blocker)
   alone    all  blocker
       1      1  spell: Tap target creature.
       1      1  triggered: When ~ enters, tap target creature.
       1      1  rules: unplayable

blockers that hold back the most cards in all (all: with or without others)
   alone    all  blocker
       1      1  spell: Tap target creature.
       1      1  triggered: When ~ enters, tap target creature.
       1      1  rules: unplayable
";
    assert_eq!(sample_report().render(3), expected);
}

#[test]
fn the_two_rankings_differ_and_each_shows_the_top_n() {
    let expected = "\
coverage: 1 precon, 11 distinct cards, 11 copies

state      cards  share   copies  share
verified       0   0.0%        0   0.0%
partial        0   0.0%        0   0.0%
manual        11 100.0%       11 100.0%
other          0   0.0%        0   0.0%

best precons by the share of their cards that are verified
    0.0%      0/11  deck

blockers that alone hold back the most cards (alone: cards with no other blocker)
   alone    all  blocker
       3      4  q
       1      5  p

blockers that hold back the most cards in all (all: with or without others)
   alone    all  blocker
       1      5  p
       0      5  r
";
    assert_eq!(ranking_report().render(2), expected);
}

#[test]
fn long_shapes_are_cut_at_one_hundred_characters() {
    let long = "x".repeat(150);
    let report = Report {
        cards: vec![blocked("long", &[long.as_str()])],
    };
    let text = report.render(1);
    let shown = text
        .lines()
        .find(|line| line.contains("xxx"))
        .unwrap_or_default();
    let shape = shown.rsplit("  ").next().unwrap_or_default();
    assert_eq!(shape.chars().count(), 100);
    assert!(shape.ends_with("xx..."), "{shape}");
}

#[rstest]
#[case::none(0, 0, 0)]
#[case::one(1, 1, 0)]
#[case::three(3, 2, 1)]
#[case::ten(10, 5, 5)]
#[case::many(30, 5, 5)]
fn the_best_and_the_worst_precons_never_overlap(
    #[case] decks: u32,
    #[case] best: usize,
    #[case] worst: usize,
) {
    let cards = (0..decks)
        .map(|n| {
            let label = format!("deck{n:02}");
            held(&format!("v{n}"), Support::Verified, &[(&label, n)])
        })
        .chain((0..decks).map(|n| {
            let label = format!("deck{n:02}");
            held(&format!("m{n}"), Support::Manual, &[(&label, 5)])
        }))
        .collect();
    let text = Report { cards }.render(1);
    let section = |title: &str| {
        text.lines()
            .skip_while(|line| !line.starts_with(title))
            .skip(1)
            .take_while(|line| line.starts_with("  "))
            .count()
    };
    assert_eq!(section("best precons"), best);
    assert_eq!(section("worst precons"), worst);
}

#[test]
fn a_full_report_stays_within_about_eighty_lines() {
    let cards: Vec<CardResult> = (0..60)
        .map(|n| {
            let label = format!("deck{:02}", n % 12);
            let blocker = format!("blocker {}", n % 40);
            CardResult {
                copies: BTreeMap::from([(label, 1 + n % 3)]),
                ..blocked(&format!("card{n}"), &[blocker.as_str()])
            }
        })
        .collect();
    let text = Report { cards }.render(crate::coverage::DEFAULT_TOP);
    assert!(text.lines().count() <= 80, "{}", text.lines().count());
    assert!(text.lines().all(|line| line.chars().count() <= 130));
}
