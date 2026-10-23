//! Goal: the report counts the items by how they stand, ranks the verbs Forge names that our rules lack by cards, groups the unread tail by Forge API with its keys and the cards no other group holds, prices a number of readers by the cards they free, and leaves out the costs, the read cards and the keys every card of a group carries.
use rstest::rstest;

use super::*;

fn finding(kind: Kind, name: &str, status: Status, keys: &[&str]) -> Finding {
    Finding {
        kind,
        name: name.to_owned(),
        key: None,
        status,
        expect: match status {
            Status::Read | Status::Missing => vec![format!("Effect.{name}")],
            Status::NoVerb | Status::Unmapped => Vec::new(),
        },
        keys: keys.iter().map(|key| (*key).to_owned()).collect(),
    }
}

fn effect(name: &str, status: Status, keys: &[&str]) -> Finding {
    finding(Kind::Effect, name, status, keys)
}

/// A finding about the key of an item whose verb is read.
fn exception(name: &str, key: &str) -> Finding {
    Finding {
        key: Some(key.to_owned()),
        expect: vec![format!("Mod.{key}")],
        ..effect(name, Status::Missing, &[])
    }
}

fn card(name: &str, unread: bool, findings: Vec<Finding>) -> CardCheck {
    CardCheck {
        name: name.to_owned(),
        source: RulesSource::Compiled,
        support: if unread {
            Support::Partial
        } else {
            Support::Verified
        },
        unread,
        findings,
        ours: vec!["effect.draw".to_owned()],
    }
}

fn survey(checks: Vec<CardCheck>) -> Survey {
    Survey {
        stored: checks.len() + 1,
        absent: vec!["Lost Card".to_owned()],
        checks,
    }
}

#[test]
fn items_are_counted_by_how_they_stand_and_a_key_is_not_an_item() {
    let survey = survey(vec![
        card(
            "A",
            true,
            vec![
                effect("Zap", Status::Read, &[]),
                exception("Zap", "NonLegendary"),
                effect("Mend", Status::NoVerb, &[]),
            ],
        ),
        card(
            "B",
            false,
            vec![
                effect("Zap", Status::Missing, &[]),
                effect("Odd", Status::Unmapped, &[]),
            ],
        ),
    ]);
    assert_eq!(
        survey.items(),
        Items {
            all: 4,
            read: 1,
            missing: 1,
            no_verb: 1,
            unmapped: 1
        }
    );
}

#[test]
fn the_cards_are_counted_by_state() {
    let mut manual = card("M", true, vec![]);
    manual.support = Support::Manual;
    let survey = survey(vec![
        card("A", false, vec![]),
        card("B", true, vec![]),
        manual,
    ]);
    let states: Vec<(&str, usize)> = survey
        .states()
        .into_iter()
        .map(|(bucket, cards)| (bucket.name(), cards))
        .collect();
    assert_eq!(
        states,
        [("verified", 1), ("partial", 1), ("manual", 1), ("other", 0)]
    );
}

#[test]
fn missing_verbs_are_ranked_by_cards_with_the_cards_that_are_all_read() {
    let mut odd_trigger = finding(Kind::Trigger, "Bell", Status::Missing, &[]);
    odd_trigger.expect = vec!["Trigger.Ring".to_owned(), "Trigger.Chime".to_owned()];
    let survey = survey(vec![
        card(
            "A",
            true,
            vec![effect("Zap", Status::Missing, &[]), odd_trigger],
        ),
        card(
            "B",
            false,
            vec![
                effect("Zap", Status::Missing, &[]),
                effect("Zap", Status::Missing, &[]),
            ],
        ),
        card("C", true, vec![exception("Zap", "NonLegendary")]),
        card("D", true, vec![effect("Mend", Status::NoVerb, &[])]),
    ]);
    let found = survey.missing_verbs();
    let labels: Vec<(&str, usize, usize)> = found
        .iter()
        .map(|verb| (verb.label.as_str(), verb.cards, verb.read_cards))
        .collect();
    assert_eq!(
        labels,
        [
            ("Effect.Zap (Zap)", 2, 1),
            ("Mod.NonLegendary (Zap+NonLegendary)", 1, 0),
            ("Trigger.Ring | Trigger.Chime (trigger Bell)", 1, 0),
        ]
    );
    assert_eq!(found[0].examples, ["A", "B"]);
    assert_eq!(found[0].read_examples, ["B"]);
    let in_read = survey.missing_in_read_cards();
    assert_eq!(in_read.len(), 1);
    assert_eq!(in_read[0].label, "Effect.Zap (Zap)");
}

const ROWS: Rows = Rows { top: 2, keys: 6 };

fn tail_survey() -> Survey {
    survey(vec![
        card(
            "A",
            true,
            vec![
                effect(
                    "ChangeZone",
                    Status::Missing,
                    &["Destination=Exile", "Optional"],
                ),
                finding(Kind::Trigger, "ChangesZone", Status::Missing, &[]),
                finding(Kind::Cost, "Sac", Status::Missing, &[]),
            ],
        ),
        card(
            "B",
            true,
            vec![
                effect("ChangeZone", Status::Missing, &["Destination=Exile"]),
                effect("CopyPermanent", Status::Missing, &["Count"]),
            ],
        ),
        card(
            "C",
            true,
            vec![
                effect("CopyPermanent", Status::Read, &[]),
                exception("CopyPermanent", "NonLegendary"),
            ],
        ),
        card("D", false, vec![effect("ChangeZone", Status::Missing, &[])]),
        card("E", true, vec![effect("ChangeZone", Status::Read, &[])]),
    ])
}

#[test]
fn the_unread_tail_is_grouped_by_api_with_its_keys() {
    let tail = tail_survey().tail(Scope::All);
    assert_eq!((tail.cards, tail.grouped), (4, 3));
    let groups: Vec<(String, usize, usize, Status)> = tail
        .groups
        .iter()
        .map(|group| (group.label(), group.cards, group.alone, group.status))
        .collect();
    assert_eq!(
        groups,
        [
            ("ChangeZone".to_owned(), 2, 0, Status::Missing),
            ("CopyPermanent".to_owned(), 2, 1, Status::Missing),
            ("trigger ChangesZone".to_owned(), 1, 0, Status::Missing),
        ]
    );
    assert_eq!(
        tail.groups[0].keys,
        [
            ("Destination=Exile".to_owned(), 2),
            ("Optional".to_owned(), 1)
        ]
    );
    assert_eq!(
        tail.groups[1].keys,
        [("Count".to_owned(), 1), ("NonLegendary".to_owned(), 1)]
    );
}

#[test]
fn the_tail_is_cut_by_whether_the_rules_read_anything_of_the_card() {
    let mut empty = card(
        "Empty",
        true,
        vec![
            effect("Draw", Status::Missing, &[]),
            effect("Zap", Status::Missing, &[]),
        ],
    );
    empty.ours.clear();
    empty.source = RulesSource::Absent;
    let survey = survey(vec![
        empty,
        card("Some", true, vec![effect("Draw", Status::Missing, &[])]),
        card("Read", false, vec![effect("Draw", Status::Missing, &[])]),
    ]);
    let cut = |scope| {
        let tail = survey.tail(scope);
        let groups: Vec<(String, usize)> = tail
            .groups
            .iter()
            .map(|group| (group.label(), group.cards))
            .collect();
        (tail.cards, tail.grouped, groups)
    };
    let draw_and_zap = vec![("Draw".to_owned(), 2), ("Zap".to_owned(), 1)];
    assert_eq!(cut(Scope::All), (2, 2, draw_and_zap));
    assert_eq!(cut(Scope::Partly), (1, 1, vec![("Draw".to_owned(), 1)]));
    assert_eq!(
        cut(Scope::Nothing),
        (1, 1, vec![("Draw".to_owned(), 1), ("Zap".to_owned(), 1)])
    );
}

#[test]
fn a_reader_per_group_frees_the_cards_whose_groups_it_covers() {
    let tail = tail_survey().tail(Scope::All);
    // Cards A, B and C hold two, two and one groups: the top group alone frees none of them.
    assert_eq!(tail.covered, [(1, 0), (3, 3)]);
}

#[test]
fn readers_are_priced_from_the_top_of_the_ranking() {
    let mut checks = Vec::new();
    for (name, groups) in [("A", 1), ("B", 1), ("C", 2), ("D", 3)] {
        let findings = (0..groups)
            .map(|n| effect(&format!("Api{n}"), Status::Missing, &[]))
            .collect();
        checks.push(card(name, true, findings));
    }
    let tail = survey(checks).tail(Scope::All);
    let ranking: Vec<(&str, usize)> = tail
        .groups
        .iter()
        .map(|group| (group.name.as_str(), group.cards))
        .collect();
    assert_eq!(ranking, [("Api0", 4), ("Api1", 2), ("Api2", 1)]);
    // One reader frees A and B; all three free C and D.
    assert_eq!(tail.covered, [(1, 2), (3, 4)]);
}

#[test]
fn a_key_every_card_of_a_group_carries_is_a_parameter_and_is_left_out() {
    let checks = (0..10)
        .map(|n| {
            let keys: &[&str] = if n < 3 {
                &["NumCards", "Rare"]
            } else {
                &["NumCards"]
            };
            card(
                &format!("C{n}"),
                true,
                vec![effect("Draw", Status::Missing, keys)],
            )
        })
        .collect();
    let tail = survey(checks).tail(Scope::All);
    assert_eq!(tail.groups[0].keys, [("Rare".to_owned(), 3)]);
}

#[test]
fn a_small_group_lists_every_key() {
    let checks = (0..4)
        .map(|n| {
            card(
                &format!("C{n}"),
                true,
                vec![effect("Draw", Status::Missing, &["NumCards"])],
            )
        })
        .collect();
    let tail = survey(checks).tail(Scope::All);
    assert_eq!(tail.groups[0].keys, [("NumCards".to_owned(), 4)]);
}

#[test]
fn the_report_prints_the_numbers_the_rankings_and_with_detail_every_card() {
    let survey = tail_survey();
    let summary = survey.render(ROWS, false);
    for wanted in [
        "forge-check: 6 stored cards, 5 with a Forge script (83.3%), 1 without",
        "state of those cards: verified 1, partial 4, manual 0, other 0",
        "verbs Forge names that our rules lack",
        "unread tail, all cards with a clause left to the table: 4 cards; 3 have items we do not read, 1 have none",
        "unread tail, those whose rules read nothing: 0 cards; 0 have items we do not read, 0 have none",
        "rules of those cards: hand-written 0, compiled 5, none 0",
        "ChangeZone [verb]  Destination=Exile 2, Optional 1",
        "top   1 groups: 0 of 3 cards (0.0%)",
        "top   3 groups: 3 of 3 cards (100.0%)",
    ] {
        assert!(summary.contains(wanted), "{wanted} missing from\n{summary}");
    }
    assert!(!summary.contains("trigger ChangesZone [verb]"), "{summary}");
    assert!(!summary.contains("our verbs"), "{summary}");

    let full = survey.render(ROWS, true);
    assert!(full.contains("  A [partial, compiled, unread]:"), "{full}");
    assert!(full.contains("      our verbs: effect.draw"), "{full}");
    assert!(
        full.contains("stored cards with no Forge script:\n  Lost Card"),
        "{full}"
    );
}

#[rstest]
#[case(0, 0, "0%")]
#[case(1, 3, "33.3%")]
#[case(2, 3, "66.7%")]
#[case(1, 1, "100.0%")]
#[case(1, 8, "12.5%")]
#[case(0, 5, "0.0%")]
fn a_share_is_written_with_one_decimal(
    #[case] part: usize,
    #[case] whole: usize,
    #[case] expected: &str,
) {
    assert_eq!(percent(part, whole), expected);
}
