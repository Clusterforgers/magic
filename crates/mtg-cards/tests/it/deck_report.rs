//! Goal: the deck check reports every support state with its reasons and spans per zone, counts the sideboard and every other zone toward the state, applies the rules of Commander, the constructed formats and freeform exactly, derives hashes that move with the deck and with coverage and with nothing else, and keeps its wire shape.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::deck::{
    CardFacts, DeckLine, DeckReport, DeckState, DeckZone, IssueCode, Legality, deck_report,
};
use mtg_cards::load::{Library, Source, slug};
use mtg_cards::support::{Caps, PrintedCard, PrintedFace, Support, oracle_fingerprint};
use mtg_core::color::{Color, ColorSet};
use rstest::rstest;
use serde_json::json;

use DeckZone::{Commander, Companion, Main, Sideboard};

/// The oracle id of the nth test card.
fn id(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012}")
}

/// What a test card is: its name, type line, rules text, keywords and color identity.
struct Spec {
    n: u32,
    name: &'static str,
    type_line: &'static str,
    text: &'static str,
    keywords: &'static [&'static str],
    identity: &'static str,
    /// Formats where the card is legal; any other format has it not legal, or banned where listed.
    legal: &'static [&'static str],
    banned: &'static [&'static str],
    restricted: &'static [&'static str],
}

const ALL: &[&str] = &["commander", "modern", "vintage", "legacy"];

const fn spec(n: u32, name: &'static str, type_line: &'static str, identity: &'static str) -> Spec {
    Spec {
        n,
        name,
        type_line,
        text: "",
        keywords: &[],
        identity,
        legal: ALL,
        banned: &[],
        restricted: &[],
    }
}

fn specs() -> Vec<Spec> {
    let mut all = base_specs();
    all.extend(pairing_specs());
    all
}

fn base_specs() -> Vec<Spec> {
    vec![
        spec(1, "Forest", "Basic Land \u{2014} Forest", "G"),
        spec(2, "Island", "Basic Snow Land \u{2014} Island", "U"),
        spec(3, "Lightning Bolt", "Instant", "R"),
        spec(4, "Sol Ring", "Artifact", ""),
        spec(
            5,
            "Atraxa, Praetors' Voice",
            "Legendary Creature \u{2014} Phyrexian Angel Horror",
            "WUBG",
        ),
        Spec {
            keywords: &["Partner"],
            ..spec(
                6,
                "Tymna the Weaver",
                "Legendary Creature \u{2014} Human Cleric",
                "WB",
            )
        },
        Spec {
            keywords: &["Partner"],
            ..spec(
                7,
                "Thrasios, Triton Hero",
                "Legendary Creature \u{2014} Merfolk Wizard",
                "GU",
            )
        },
        Spec {
            text: "A deck can have any number of cards named Relentless Rats.",
            ..spec(8, "Relentless Rats", "Creature \u{2014} Rat", "B")
        },
        Spec {
            text: "A deck can have up to nine cards named Nazg\u{fb}l.",
            ..spec(9, "Nazg\u{fb}l", "Creature \u{2014} Wraith", "B")
        },
        spec(10, "Llanowar Elves", "Creature \u{2014} Elf Druid", "G"),
        Spec {
            text: "Jace can be your commander.",
            ..spec(
                11,
                "Jace, Commander",
                "Legendary Planeswalker \u{2014} Jace",
                "U",
            )
        },
        Spec {
            banned: &["modern", "commander"],
            ..spec(12, "Tolarian Academy", "Legendary Land", "U")
        },
        Spec {
            legal: &[],
            ..spec(13, "Un-Card", "Creature \u{2014} Goblin", "R")
        },
        Spec {
            restricted: &["vintage"],
            ..spec(14, "Ancestral Recall", "Instant", "U")
        },
        Spec {
            keywords: &["Choose a Background"],
            ..spec(
                15,
                "Wilson, Dungeon Master",
                "Legendary Creature \u{2014} Human",
                "R",
            )
        },
        spec(
            16,
            "Raised by Giants",
            "Legendary Enchantment \u{2014} Background",
            "G",
        ),
        Spec {
            keywords: &["Partner with"],
            text: "Partner with Pir, Imaginative Rascal",
            ..spec(
                17,
                "Toothy, Imaginary Friend",
                "Legendary Creature \u{2014} Illusion",
                "U",
            )
        },
        Spec {
            keywords: &["Partner with"],
            text: "Partner with Toothy, Imaginary Friend",
            ..spec(
                18,
                "Pir, Imaginative Rascal",
                "Legendary Creature \u{2014} Human",
                "G",
            )
        },
    ]
}

/// Cards for the pairing rules beyond Partner and the identity of hybrid and colorless commanders.
fn pairing_specs() -> Vec<Spec> {
    vec![
        // Its {G/W} hybrid symbol puts both colors in the identity.
        spec(
            19,
            "Rhys the Redeemed",
            "Legendary Creature \u{2014} Elf Warrior",
            "WG",
        ),
        Spec {
            keywords: &["Friends forever"],
            ..spec(20, "Friend One", "Legendary Creature \u{2014} Human", "G")
        },
        Spec {
            keywords: &["Friends forever"],
            ..spec(21, "Friend Two", "Legendary Creature \u{2014} Human", "U")
        },
        Spec {
            keywords: &["Doctor's companion"],
            ..spec(
                22,
                "Companion Example",
                "Legendary Creature \u{2014} Human",
                "G",
            )
        },
        spec(
            23,
            "Doctor Example",
            "Legendary Creature \u{2014} Time Lord Doctor",
            "U",
        ),
        spec(
            24,
            "Colorless Legend",
            "Legendary Artifact Creature \u{2014} Golem",
            "",
        ),
    ]
}

fn facts(spec: &Spec) -> (String, CardFacts) {
    let identity: ColorSet = spec
        .identity
        .chars()
        .filter_map(Color::from_symbol)
        .collect();
    let mut legalities = BTreeMap::new();
    for format in spec.legal {
        legalities.insert((*format).to_owned(), Legality::Legal);
    }
    for format in spec.banned {
        legalities.insert((*format).to_owned(), Legality::Banned);
    }
    for format in spec.restricted {
        legalities.insert((*format).to_owned(), Legality::Restricted);
    }
    let card = CardFacts {
        printed: PrintedCard {
            layout: String::new(),
            oracle_id: id(spec.n),
            name: spec.name.to_owned(),
            keywords: spec.keywords.iter().map(|k| (*k).to_owned()).collect(),
            faces: vec![PrintedFace {
                name: spec.name.to_owned(),
                type_line: spec.type_line.to_owned(),
                oracle_text: spec.text.to_owned(),
                ..PrintedFace::default()
            }],
        },
        color_identity: identity,
        legalities,
    };
    (id(spec.n), card)
}

fn cards() -> BTreeMap<String, CardFacts> {
    specs().iter().map(facts).collect()
}

fn n_of(name: &str) -> u32 {
    specs().iter().find(|s| s.name == name).unwrap().n
}

fn line(zone: DeckZone, name: &str, count: u32) -> DeckLine {
    DeckLine {
        zone,
        oracle_id: id(n_of(name)),
        count,
    }
}

fn empty_library() -> Library {
    Library::from_sources(&[]).unwrap()
}

fn report(format: &str, lines: &[DeckLine]) -> DeckReport {
    deck_report(format, lines, &cards(), &empty_library(), &Caps::none())
}

fn codes(report: &DeckReport) -> Vec<IssueCode> {
    report.legality.iter().map(|issue| issue.code).collect()
}

/// An Atraxa deck of `forests` Forests and `islands` Islands, plus extra lines.
fn atraxa(forests: u32, islands: u32, extra: &[DeckLine]) -> Vec<DeckLine> {
    let mut lines = vec![
        line(Commander, "Atraxa, Praetors' Voice", 1),
        line(Main, "Forest", forests),
        line(Main, "Island", islands),
    ];
    lines.extend_from_slice(extra);
    lines
}

fn sixty(extra: &[DeckLine]) -> Vec<DeckLine> {
    let mut lines = vec![line(Main, "Forest", 30), line(Main, "Island", 30)];
    lines.extend_from_slice(extra);
    lines
}

#[rstest]
#[case::legal(atraxa(50, 49, &[]), vec![])]
#[case::one_card_short(atraxa(50, 48, &[]), vec![IssueCode::DeckSize])]
#[case::one_card_over(atraxa(50, 50, &[]), vec![IssueCode::DeckSize])]
#[case::a_sideboard_is_no_part_of_the_hundred(atraxa(50, 49, &[line(Sideboard, "Lightning Bolt", 3)]), vec![])]
#[case::no_commander(
    vec![line(Main, "Forest", 50), line(Main, "Island", 50)],
    vec![IssueCode::CommanderCount]
)]
#[case::three_commanders(
    vec![
        line(Commander, "Tymna the Weaver", 1),
        line(Commander, "Thrasios, Triton Hero", 1),
        line(Commander, "Atraxa, Praetors' Voice", 1),
        line(Main, "Forest", 49),
        line(Main, "Island", 48),
    ],
    vec![IssueCode::CommanderCount]
)]
#[case::partners(
    vec![
        line(Commander, "Tymna the Weaver", 1),
        line(Commander, "Thrasios, Triton Hero", 1),
        line(Main, "Forest", 49),
        line(Main, "Island", 49),
    ],
    vec![]
)]
#[case::named_partners(
    vec![
        line(Commander, "Toothy, Imaginary Friend", 1),
        line(Commander, "Pir, Imaginative Rascal", 1),
        line(Main, "Forest", 49),
        line(Main, "Island", 49),
    ],
    vec![]
)]
#[case::a_creature_and_a_background(
    vec![
        line(Commander, "Wilson, Dungeon Master", 1),
        line(Commander, "Raised by Giants", 1),
        line(Main, "Forest", 98),
    ],
    vec![]
)]
#[case::friends_forever(
    vec![
        line(Commander, "Friend One", 1),
        line(Commander, "Friend Two", 1),
        line(Main, "Forest", 49),
        line(Main, "Island", 49),
    ],
    vec![]
)]
#[case::friends_forever_and_partner_do_not_mix(
    vec![
        line(Commander, "Friend One", 1),
        line(Commander, "Tymna the Weaver", 1),
        line(Main, "Forest", 98),
    ],
    vec![IssueCode::CommanderPair]
)]
#[case::a_doctors_companion_and_a_time_lord_doctor(
    vec![
        line(Commander, "Companion Example", 1),
        line(Commander, "Doctor Example", 1),
        line(Main, "Forest", 49),
        line(Main, "Island", 49),
    ],
    vec![]
)]
#[case::a_time_lord_doctor_needs_the_companion(
    vec![
        line(Commander, "Doctor Example", 1),
        line(Commander, "Friend Two", 1),
        line(Main, "Island", 98),
    ],
    vec![IssueCode::CommanderPair]
)]
#[case::a_hybrid_commander_allows_both_its_colors(
    vec![
        line(Commander, "Rhys the Redeemed", 1),
        line(Main, "Forest", 98),
        line(Main, "Sol Ring", 1),
    ],
    vec![]
)]
#[case::a_hybrid_commander_refuses_a_third_color(
    vec![
        line(Commander, "Rhys the Redeemed", 1),
        line(Main, "Forest", 97),
        line(Main, "Sol Ring", 1),
        line(Main, "Island", 1),
    ],
    vec![IssueCode::ColorIdentity]
)]
#[case::a_colorless_commander_refuses_every_color(
    vec![
        line(Commander, "Colorless Legend", 1),
        line(Main, "Forest", 98),
        line(Main, "Sol Ring", 1),
    ],
    vec![IssueCode::ColorIdentity]
)]
#[case::two_commanders_without_partner(
    vec![
        line(Commander, "Atraxa, Praetors' Voice", 1),
        line(Commander, "Thrasios, Triton Hero", 1),
        line(Main, "Forest", 49),
        line(Main, "Island", 49),
    ],
    vec![IssueCode::CommanderPair]
)]
#[case::one_commander_twice(
    vec![
        line(Commander, "Tymna the Weaver", 2),
        line(Main, "Relentless Rats", 98),
    ],
    vec![IssueCode::CommanderPair, IssueCode::Copies]
)]
#[case::a_commander_that_is_no_legend(
    vec![line(Commander, "Llanowar Elves", 1), line(Main, "Forest", 99)],
    vec![IssueCode::CommanderEligibility]
)]
#[case::a_planeswalker_that_says_so(
    vec![line(Commander, "Jace, Commander", 1), line(Main, "Island", 99)],
    vec![]
)]
#[case::a_background_alone(
    vec![line(Commander, "Raised by Giants", 1), line(Main, "Forest", 99)],
    vec![IssueCode::CommanderEligibility]
)]
#[case::singleton(
    atraxa(50, 47, &[line(Main, "Sol Ring", 2)]),
    vec![IssueCode::Copies]
)]
#[case::basic_lands_and_any_number_cards_are_free(
    atraxa(40, 30, &[line(Main, "Relentless Rats", 29)]),
    vec![]
)]
#[case::up_to_nine(
    atraxa(45, 44, &[line(Main, "Sol Ring", 1), line(Main, "Nazg\u{fb}l", 9)]),
    vec![]
)]
#[case::more_than_nine(
    atraxa(44, 44, &[line(Main, "Sol Ring", 1), line(Main, "Nazg\u{fb}l", 10)]),
    vec![IssueCode::Copies]
)]
#[case::outside_the_identity(
    atraxa(50, 48, &[line(Main, "Lightning Bolt", 1)]),
    vec![IssueCode::ColorIdentity]
)]
#[case::banned(
    atraxa(50, 48, &[line(Main, "Tolarian Academy", 1)]),
    vec![IssueCode::Banned]
)]
#[case::banned_in_the_sideboard_too(
    atraxa(50, 49, &[line(Sideboard, "Tolarian Academy", 1)]),
    vec![IssueCode::Banned]
)]
#[case::not_in_the_pool(
    atraxa(50, 48, &[line(Main, "Un-Card", 1)]),
    vec![IssueCode::ColorIdentity, IssueCode::NotLegal]
)]
#[case::two_companions(
    atraxa(50, 49, &[line(Companion, "Forest", 2)]),
    vec![IssueCode::CompanionCount]
)]
#[case::a_companion_outside_the_identity(
    atraxa(50, 49, &[line(Companion, "Lightning Bolt", 1)]),
    vec![IssueCode::ColorIdentity]
)]
fn commander_decks(#[case] lines: Vec<DeckLine>, #[case] expected: Vec<IssueCode>) {
    let found = report("commander", &lines);
    let mut got = codes(&found);
    got.sort_by_key(|code| format!("{code:?}"));
    let mut want = expected;
    want.sort_by_key(|code| format!("{code:?}"));
    assert_eq!(got, want, "{:#?}", found.legality);
}

#[rstest]
#[case::legal(sixty(&[]), vec![])]
#[case::four_copies(sixty(&[line(Main, "Lightning Bolt", 4)]), vec![])]
#[case::five_copies(sixty(&[line(Main, "Lightning Bolt", 5)]), vec![IssueCode::Copies])]
#[case::copies_count_across_main_and_sideboard(
    sixty(&[line(Main, "Lightning Bolt", 3), line(Sideboard, "Lightning Bolt", 2)]),
    vec![IssueCode::Copies]
)]
#[case::basic_lands_are_free(vec![line(Main, "Forest", 60), line(Sideboard, "Island", 15)], vec![])]
#[case::fifty_nine(vec![line(Main, "Forest", 59)], vec![IssueCode::DeckSize])]
#[case::sixteen_in_the_sideboard(
    sixty(&[line(Sideboard, "Forest", 16)]),
    vec![IssueCode::SideboardSize]
)]
#[case::fifteen_in_the_sideboard(sixty(&[line(Sideboard, "Forest", 15)]), vec![])]
#[case::no_command_zone(
    sixty(&[line(Commander, "Atraxa, Praetors' Voice", 1)]),
    vec![IssueCode::NoCommanderZone]
)]
#[case::one_companion(sixty(&[line(Companion, "Lightning Bolt", 1)]), vec![])]
#[case::two_companions(sixty(&[line(Companion, "Forest", 2)]), vec![IssueCode::CompanionCount])]
#[case::banned(sixty(&[line(Main, "Tolarian Academy", 1)]), vec![IssueCode::Banned])]
#[case::not_in_the_pool(sixty(&[line(Sideboard, "Un-Card", 1)]), vec![IssueCode::NotLegal])]
#[case::any_number_cards(sixty(&[line(Main, "Relentless Rats", 20)]), vec![])]
fn modern_decks(#[case] lines: Vec<DeckLine>, #[case] expected: Vec<IssueCode>) {
    let found = report("modern", &lines);
    assert_eq!(codes(&found), expected, "{:#?}", found.legality);
}

#[rstest]
#[case::one_restricted_card(sixty(&[line(Main, "Ancestral Recall", 1)]), vec![])]
#[case::two_restricted_cards(sixty(&[line(Main, "Ancestral Recall", 2)]), vec![IssueCode::Copies])]
#[case::one_in_each_part(
    sixty(&[line(Main, "Ancestral Recall", 1), line(Sideboard, "Ancestral Recall", 1)]),
    vec![IssueCode::Copies]
)]
fn vintage_restricts_to_one_copy(#[case] lines: Vec<DeckLine>, #[case] expected: Vec<IssueCode>) {
    assert_eq!(codes(&report("vintage", &lines)), expected);
}

#[rstest]
#[case::anything_goes(vec![line(Main, "Lightning Bolt", 40), line(Commander, "Forest", 5), line(Companion, "Forest", 5), line(Sideboard, "Un-Card", 99)], vec![])]
#[case::empty(vec![], vec![IssueCode::Empty])]
#[case::only_empty_lines(vec![line(Main, "Forest", 0)], vec![IssueCode::Empty])]
fn freeform_asks_for_a_card_and_nothing_else(
    #[case] lines: Vec<DeckLine>,
    #[case] expected: Vec<IssueCode>,
) {
    assert_eq!(codes(&report("freeform", &lines)), expected);
}

#[rstest]
#[case("commander")]
#[case("modern")]
#[case("freeform")]
fn an_empty_deck_is_blocked_in_every_format(#[case] format: &str) {
    let found = report(format, &[]);
    assert_eq!(codes(&found), [IssueCode::Empty]);
    assert_eq!(found.summary.state, DeckState::Blocked);
    assert!(found.cards.is_empty() && found.summary.zones.is_empty());
}

#[test]
fn a_format_without_rules_and_a_card_the_database_lacks_are_issues() {
    let ghost = DeckLine {
        zone: Main,
        oracle_id: id(999),
        count: 2,
    };
    let found = report("brawl", &[line(Main, "Forest", 59), ghost.clone()]);
    assert_eq!(
        codes(&found),
        [IssueCode::UnknownCard, IssueCode::UnknownFormat]
    );
    let unknown = found
        .legality
        .iter()
        .find(|i| i.code == IssueCode::UnknownCard)
        .unwrap();
    assert_eq!(
        (unknown.zone, unknown.oracle_id.as_deref()),
        (Some(Main), Some(id(999).as_str()))
    );
    let card = found.cards.iter().find(|c| c.oracle_id == id(999)).unwrap();
    assert_eq!((card.support, card.count), (Support::Unplayable, 2));
    assert_eq!(card.name, id(999));
    assert_eq!(found.summary.state, DeckState::Blocked);

    let freeform = report("freeform", &[ghost]);
    assert_eq!(codes(&freeform), [IssueCode::UnknownCard]);
}

#[test]
fn issues_name_their_card_and_zone() {
    let found = report("modern", &sixty(&[line(Sideboard, "Tolarian Academy", 1)]));
    assert_eq!(found.legality.len(), 1);
    let issue = &found.legality[0];
    assert_eq!(issue.code, IssueCode::Banned);
    assert_eq!(issue.zone, Some(Sideboard));
    assert_eq!(issue.oracle_id.as_deref(), Some(id(12).as_str()));
    assert_eq!(issue.name.as_deref(), Some("Tolarian Academy"));
    assert_eq!(issue.message, "Tolarian Academy is banned in modern");
}

#[rstest]
#[case::a_card_outside_the_identity(
    vec![
        line(Commander, "Colorless Legend", 1),
        line(Main, "Forest", 98),
        line(Main, "Sol Ring", 1),
    ],
    "Forest has G in its color identity, outside the commander's (colorless)"
)]
#[case::a_third_color_of_a_hybrid_commander(
    vec![
        line(Commander, "Rhys the Redeemed", 1),
        line(Main, "Forest", 97),
        line(Main, "Sol Ring", 1),
        line(Main, "Island", 1),
    ],
    "Island has U in its color identity, outside the commander's (WG)"
)]
fn the_identity_issue_says_color_in_the_spelling_of_the_rest_of_the_app(
    #[case] lines: Vec<DeckLine>,
    #[case] message: &str,
) {
    let found = report("commander", &lines);
    let said: Vec<&str> = found
        .legality
        .iter()
        .filter(|issue| issue.code == IssueCode::ColorIdentity)
        .map(|issue| issue.message.as_str())
        .collect();
    assert!(said.contains(&message), "{said:?}");
    assert!(
        found
            .legality
            .iter()
            .all(|issue| !issue.message.to_lowercase().contains("colour")),
        "{:?}",
        found.legality
    );
}

// ---- support ----

const TEXT_PARTIAL: &str = "Flying\nDraw a card.";

/// A rules file for one test card.
fn source(n: u32, name: &str, status: &str, text: &str, abilities: &str) -> Source {
    let fingerprint = if abilities.is_empty() && status != "Full" {
        String::new()
    } else {
        format!(r#","oracle_fingerprint":"{}""#, oracle_fingerprint(text))
    };
    Source {
        path: format!("cards/{}.json", slug(name)),
        text: format!(
            r#"{{"oracle_id":"{}","name":"{name}","impl_status":"{status}","faces":[{{"name":"{name}"{fingerprint},"abilities":[{abilities}]}}]}}"#,
            id(n)
        ),
    }
}

/// Five cards, one in each support state: 21 verified, 22 partial (a clause over `Draw a card.`),
/// 23 manual (declared), 24 stale (its text changed), 25 unplayable.
fn states() -> (Library, BTreeMap<String, CardFacts>) {
    let library = Library::from_sources(&[
        source(21, "Verified Vanilla", "Full", "", ""),
        source(
            22,
            "Partial Flyer",
            "Partial",
            TEXT_PARTIAL,
            r#"{"Keyword":"Flying"},{"Manual":{"reason":"no draw yet","span":{"start":7,"end":19}}}"#,
        ),
        source(23, "Declared Manual", "Manual", "Whatever it says.", ""),
        source(24, "Errata Victim", "Full", "The old text.", ""),
        source(25, "Digital Only", "Unplayable", "Conjure a card.", ""),
    ])
    .unwrap_or_else(|errors| panic!("{errors}"));
    let make = |n: u32, name: &str, text: &str| {
        let (key, mut card) = facts(&Spec {
            n,
            name: "x",
            type_line: "Creature \u{2014} Test",
            text: "",
            keywords: &[],
            identity: "",
            legal: &[],
            banned: &[],
            restricted: &[],
        });
        name.clone_into(&mut card.printed.name);
        name.clone_into(&mut card.printed.faces[0].name);
        text.clone_into(&mut card.printed.faces[0].oracle_text);
        (key, card)
    };
    let cards = [
        make(21, "Verified Vanilla", ""),
        make(22, "Partial Flyer", TEXT_PARTIAL),
        make(23, "Declared Manual", "Whatever it says."),
        make(24, "Errata Victim", "The new text."),
        make(25, "Digital Only", "Conjure a card."),
        make(26, "No Rules File", "Some text."),
    ]
    .into_iter()
    .collect();
    (library, cards)
}

fn free(lines: &[(DeckZone, u32, u32)]) -> Vec<DeckLine> {
    lines
        .iter()
        .map(|(zone, n, count)| DeckLine {
            zone: *zone,
            oracle_id: id(*n),
            count: *count,
        })
        .collect()
}

fn states_report(lines: &[(DeckZone, u32, u32)], caps: &Caps) -> DeckReport {
    let (library, cards) = states();
    deck_report("freeform", &free(lines), &cards, &library, caps)
}

#[test]
fn every_support_state_is_reported_with_its_reasons_and_spans() {
    let found = states_report(
        &[
            (Main, 21, 4),
            (Main, 22, 3),
            (Main, 23, 2),
            (Main, 24, 1),
            (Main, 25, 1),
            (Main, 26, 5),
        ],
        &Caps::everything(),
    );
    let by_name: BTreeMap<&str, _> = found.cards.iter().map(|c| (c.name.as_str(), c)).collect();
    assert_eq!(by_name["Verified Vanilla"].support, Support::Verified);
    assert_eq!(by_name["Verified Vanilla"].reasons.len(), 0);

    let partial = by_name["Partial Flyer"];
    assert_eq!((partial.support, partial.count), (Support::Partial, 3));
    assert_eq!(partial.reasons.len(), 1);
    assert_eq!(partial.reasons[0].text, "no draw yet");
    assert_eq!(
        (
            partial.reasons[0].face,
            partial.reasons[0].span.start,
            partial.reasons[0].span.end
        ),
        (0, 7, 19)
    );

    assert_eq!(by_name["Declared Manual"].support, Support::Manual);
    assert_eq!(by_name["Errata Victim"].support, Support::Stale);
    assert_eq!(by_name["Digital Only"].support, Support::Unplayable);
    assert_eq!(by_name["No Rules File"].support, Support::Manual);

    let counts = found.summary.counts;
    assert_eq!(
        (
            counts.total,
            counts.verified,
            counts.partial,
            counts.manual,
            counts.stale,
            counts.unplayable
        ),
        (16, 4, 3, 7, 1, 1)
    );
    assert_eq!(
        found.summary.state,
        DeckState::Blocked,
        "an unplayable card blocks the deck"
    );
}

#[rstest]
#[case::all_verified(vec![(Main, 21, 40)], DeckState::Verified)]
#[case::a_partial_card(vec![(Main, 21, 40), (Main, 22, 1)], DeckState::ManualPlay)]
#[case::a_manual_card(vec![(Main, 21, 40), (Main, 23, 1)], DeckState::ManualPlay)]
#[case::a_stale_card_counts_as_manual(vec![(Main, 21, 40), (Main, 24, 1)], DeckState::ManualPlay)]
#[case::an_unplayable_card(vec![(Main, 21, 40), (Main, 25, 1)], DeckState::Blocked)]
#[case::one_manual_card_in_the_sideboard(vec![(Main, 21, 40), (Sideboard, 23, 1)], DeckState::ManualPlay)]
#[case::one_manual_card_in_the_command_zone(vec![(Main, 21, 40), (Commander, 22, 1)], DeckState::ManualPlay)]
#[case::one_manual_card_as_companion(vec![(Main, 21, 40), (Companion, 26, 1)], DeckState::ManualPlay)]
#[case::an_unplayable_card_in_the_sideboard(vec![(Main, 21, 40), (Sideboard, 25, 1)], DeckState::Blocked)]
#[case::verified_in_every_zone(vec![(Main, 21, 40), (Sideboard, 21, 3), (Commander, 21, 1), (Companion, 21, 1)], DeckState::Verified)]
fn the_state_counts_every_zone(#[case] lines: Vec<(DeckZone, u32, u32)>, #[case] state: DeckState) {
    let found = states_report(&lines, &Caps::everything());
    assert_eq!(found.summary.state, state);
    assert_eq!(found.is_clean(), state == DeckState::Verified);
}

#[test]
fn cards_are_grouped_by_zone_and_the_counts_follow_the_zones() {
    let found = states_report(
        &[
            (Companion, 26, 1),
            (Sideboard, 23, 2),
            (Main, 22, 3),
            (Main, 21, 4),
            (Commander, 21, 1),
            (Sideboard, 21, 1),
        ],
        &Caps::everything(),
    );
    let order: Vec<(DeckZone, &str)> = found
        .cards
        .iter()
        .map(|c| (c.zone, c.name.as_str()))
        .collect();
    assert_eq!(
        order,
        [
            (Main, "Partial Flyer"),
            (Main, "Verified Vanilla"),
            (Sideboard, "Declared Manual"),
            (Sideboard, "Verified Vanilla"),
            (Commander, "Verified Vanilla"),
            (Companion, "No Rules File"),
        ]
    );
    let zones: Vec<(DeckZone, u32, u32)> = found
        .summary
        .zones
        .iter()
        .map(|z| (z.zone, z.counts.total, z.counts.verified))
        .collect();
    assert_eq!(
        zones,
        [
            (Main, 7, 4),
            (Sideboard, 3, 1),
            (Commander, 1, 1),
            (Companion, 1, 0)
        ]
    );
    assert_eq!(found.summary.counts.total, 12);
}

#[test]
fn the_engine_lifts_a_card_from_manual_to_verified_and_the_coverage_hash_follows() {
    let lines = [(Main, 21, 4), (Main, 22, 1)];
    let everything = states_report(&lines, &Caps::everything());
    let none = states_report(&lines, &Caps::none());
    let flying_only = states_report(&lines, &Caps::from_names(["Keyword.Flying"]));
    let partial = |report: &DeckReport| {
        report
            .cards
            .iter()
            .find(|c| c.name == "Partial Flyer")
            .unwrap()
            .support
    };
    assert_eq!(partial(&everything), Support::Partial);
    assert_eq!(partial(&none), Support::Manual);
    assert_eq!(partial(&flying_only), Support::Partial);
    assert_eq!(
        everything.deck_hash, none.deck_hash,
        "the cards are the same"
    );
    assert_ne!(everything.coverage_hash, none.coverage_hash);
    assert_ne!(none.coverage_hash, flying_only.coverage_hash);
}

#[test]
fn the_hashes_move_with_what_they_are_made_of_and_with_nothing_else() {
    let base = [(Main, 21, 4), (Sideboard, 22, 1)];
    let found = states_report(&base, &Caps::everything());

    // The order of the lines and how the copies are split do not matter.
    let reshuffled = states_report(
        &[(Sideboard, 22, 1), (Main, 21, 1), (Main, 21, 3)],
        &Caps::everything(),
    );
    assert_eq!(reshuffled.deck_hash, found.deck_hash);
    assert_eq!(reshuffled.coverage_hash, found.coverage_hash);
    assert_eq!(reshuffled, found);

    let moved = states_report(&[(Main, 21, 4), (Main, 22, 1)], &Caps::everything());
    assert_ne!(moved.deck_hash, found.deck_hash, "a card in another zone");
    assert_eq!(moved.coverage_hash, found.coverage_hash, "the same cards");

    let more = states_report(&[(Main, 21, 5), (Sideboard, 22, 1)], &Caps::everything());
    assert_ne!(more.deck_hash, found.deck_hash, "another count");
    assert_eq!(more.coverage_hash, found.coverage_hash);

    let other = states_report(&[(Main, 21, 4), (Sideboard, 23, 1)], &Caps::everything());
    assert_ne!(other.deck_hash, found.deck_hash, "another card");
    assert_ne!(other.coverage_hash, found.coverage_hash);

    let (library, mut cards) = states();
    cards.get_mut(&id(21)).unwrap().printed.faces[0].oracle_text = "Now with text.".to_owned();
    let errata = deck_report(
        "freeform",
        &free(&base),
        &cards,
        &library,
        &Caps::everything(),
    );
    assert_eq!(errata.deck_hash, found.deck_hash, "the deck did not change");
    assert_ne!(
        errata.coverage_hash, found.coverage_hash,
        "its card's text did"
    );
}

#[test]
fn the_report_keeps_its_wire_shape() {
    let (library, cards) = states();
    let lines = free(&[(Main, 21, 2), (Sideboard, 22, 1)]);
    let found = deck_report("freeform", &lines, &cards, &library, &Caps::everything());
    let value = serde_json::to_value(&found).unwrap();
    assert_eq!(
        value["cards"],
        json!([
            {
                "oracle_id": id(21), "name": "Verified Vanilla", "zone": "main", "count": 2,
                "support": "verified", "reasons": []
            },
            {
                "oracle_id": id(22), "name": "Partial Flyer", "zone": "sideboard", "count": 1,
                "support": "partial",
                "reasons": [{"face": 0, "span": {"start": 7, "end": 19}, "text": "no draw yet"}]
            }
        ])
    );
    assert_eq!(value["format"], "freeform");
    assert_eq!(value["legality"], json!([]));
    assert_eq!(
        value["summary"],
        json!({
            "format": "freeform",
            "state": "manual_play",
            "counts": {"total": 3, "verified": 2, "read": 0, "partial": 1, "manual": 0, "stale": 0, "unplayable": 0},
            "zones": [
                {"zone": "main", "counts": {"total": 2, "verified": 2, "read": 0, "partial": 0, "manual": 0, "stale": 0, "unplayable": 0}},
                {"zone": "sideboard", "counts": {"total": 1, "verified": 0, "read": 0, "partial": 1, "manual": 0, "stale": 0, "unplayable": 0}}
            ],
            "issues": 0,
            "to_read": 0
        })
    );
    for key in ["deck_hash", "coverage_hash"] {
        let hash = value[key].as_str().unwrap();
        assert!(
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
            "{key}: {hash}"
        );
    }
    let round: DeckReport = serde_json::from_value(value).unwrap();
    assert_eq!(round, found);
}

#[test]
fn absurd_counts_saturate_instead_of_overflowing() {
    let lines = [
        line(Main, "Forest", u32::MAX),
        line(Main, "Forest", u32::MAX),
        line(Sideboard, "Forest", u32::MAX),
        line(Commander, "Atraxa, Praetors' Voice", u32::MAX),
    ];
    for format in ["freeform", "commander", "modern"] {
        let found = report(format, &lines);
        assert_eq!(found.summary.counts.total, u32::MAX, "{format}");
        assert_eq!(found.cards.iter().map(|c| c.count).max(), Some(u32::MAX));
    }
    let hash = |lines: &[DeckLine]| mtg_cards::deck::deck_hash(lines);
    assert_eq!(
        hash(&lines),
        hash(&lines[1..]),
        "copies of a card in a zone are summed, up to the largest count"
    );
}

#[test]
fn the_same_deck_gives_the_same_report_from_any_order_of_input() {
    let a = report(
        "commander",
        &atraxa(50, 49, &[line(Sideboard, "Lightning Bolt", 2)]),
    );
    let mut lines = atraxa(50, 49, &[line(Sideboard, "Lightning Bolt", 2)]);
    lines.reverse();
    assert_eq!(report("commander", &lines), a);
}
