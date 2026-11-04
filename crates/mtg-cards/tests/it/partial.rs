//! Goal: the proof of a partial translation marks every printed clause or refuses by name; a clause is never lost between the ledger and the text, a by-hand line that would change what runs beside it is refused, and the clauses plus the reminder text cover all of every card's text.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::compile::compile;
use mtg_cards::load::Library;
use mtg_cards::proposal::partial::{LedgerLine, Mark, Refusal, prove};
use mtg_cards::proposal::{Report, check};
use mtg_cards::support::{PrintedCard, PrintedFace};
use rstest::rstest;

use common::{printed, token_sources};

fn card(name: &str, mana: &str, type_line: &str, oracle: &str) -> PrintedCard {
    PrintedCard {
        oracle_id: "00000000-0000-0000-0000-000000000001".to_owned(),
        name: name.to_owned(),
        layout: "normal".to_owned(),
        faces: vec![PrintedFace {
            name: name.to_owned(),
            mana_cost: mana.to_owned(),
            type_line: type_line.to_owned(),
            oracle_text: oracle.to_owned(),
            ..PrintedFace::default()
        }],
        ..PrintedCard::default()
    }
}

fn report(card: &PrintedCard, rules: &str) -> Report {
    let tokens = Library::from_sources(&token_sources()).unwrap();
    let names: BTreeSet<String> = BTreeSet::from([card.name.to_lowercase()]);
    check(rules, card, &tokens, &names, &Caps::everything())
}

fn row(tag: &str, text: &str, abilities: &[usize], gap: Option<&str>) -> LedgerLine {
    LedgerLine {
        face: 0,
        tag: tag.to_owned(),
        text: text.to_owned(),
        abilities: abilities.to_vec(),
        gap: gap.map(str::to_owned),
    }
}

fn names(result: &Result<impl Sized, Vec<Refusal>>) -> Vec<&'static str> {
    match result {
        Ok(_) => vec![],
        Err(list) => list.iter().map(Refusal::name).collect(),
    }
}

const ANGEL_ROLL: &str = "Flying\n{2}, {T}: Roll a six-sided die.";
const FLYING: &str = r#"{"faces":[{"abilities":[{"Keyword":"Flying"}]}]}"#;
const BANEFIRE: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"X"}}]}}]}]}"#;
const AETHER_WEB: &str = r#"{"faces":[{"abilities":[{"Keyword":"Flash"},{"Keyword":{"Enchant":[{"Type":"Creature"}]}},{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":1,"toughness":1}},{"AddKeyword":"Reach"}]}}]}]}"#;
const PHOENIX: &str = r#"{"faces":[{"abilities":[{"Keyword":"Flying"},{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Move":{"what":"This","to":{"Battlefield":{}}}}]}}]}]}"#;

fn banefire() -> PrintedCard {
    card(
        "Banefire",
        "{X}{R}",
        "Sorcery",
        "Banefire deals X damage to any target.\nIf X is 5 or more, this spell can't be countered and the damage can't be prevented.",
    )
}

fn aether_web() -> PrintedCard {
    card(
        "Aether Web",
        "{1}{G}",
        "Enchantment \u{2014} Aura",
        "Flash\nEnchant creature\nEnchanted creature gets +1/+1 and has reach and can block an additional creature each combat.",
    )
}

fn phoenix() -> PrintedCard {
    card(
        "Bogardan Phoenix",
        "{2}{R}{R}",
        "Creature \u{2014} Phoenix",
        "Flying\nWhen Bogardan Phoenix dies, if it doesn't have a death counter on it, return it to the battlefield under its owner's control and put a death counter on it.",
    )
}

#[rstest]
#[case::banefire(
    banefire(),
    BANEFIRE,
    vec![
        row("A", "CARDNAME deals X damage to any target.", &[0], None),
        row("S", "If X is 5 or more, this spell can't be countered and the damage can't be prevented.", &[], Some("static:CantBeCountered")),
    ],
)]
#[case::aether_web(
    aether_web(),
    AETHER_WEB,
    vec![
        row("K", "Flash", &[0], None),
        row("K", "Enchant:Creature", &[1], None),
        row("S", "Enchanted creature gets +1/+1 and has reach.", &[2], None),
        row("S", "Enchanted creature can block an additional creature each combat.", &[], Some("static:AdditionalBlock")),
    ],
)]
#[case::bogardan_phoenix(
    phoenix(),
    PHOENIX,
    vec![
        row("K", "Flying", &[0], None),
        row("T", "When CARDNAME dies, return it to the battlefield under its owner's control.", &[1], None),
        row("T", "When CARDNAME dies, if it doesn't have a death counter on it, put a death counter on it.", &[], Some("effect:PutCounter")),
    ],
)]
fn a_gap_inside_an_automatic_line_is_refused(
    #[case] printed: PrintedCard,
    #[case] rules: &str,
    #[case] ledger: Vec<LedgerLine>,
) {
    let result = prove(&report(&printed, rules), &ledger);
    assert!(names(&result).contains(&"GapInsideLine"), "{result:?}");
}

#[rstest]
#[case::invalid(card("X", "", "Creature", "Flying"), "not json", vec![], vec!["InvalidRules"])]
#[case::stray(
    card("X", "", "Creature", "Flying"),
    r#"{"faces":[{"abilities":[{"Keyword":"Flying"},{"Keyword":"Haste"}]}]}"#,
    vec![row("K", "Flying", &[0], None)],
    vec!["Stray"],
)]
#[case::by_hand_without_a_gap_line(
    card("X", "", "Creature", ANGEL_ROLL),
    FLYING,
    vec![row("K", "Flying", &[0], None)],
    vec!["OpenWithoutGap"],
)]
#[case::gap_without_text(
    card("X", "", "Creature", ANGEL_ROLL),
    FLYING,
    vec![row("K", "Flying", &[0], None), row("A", "", &[], Some("effect:RollDice"))],
    vec!["UntextedGap", "OpenWithoutGap"],
)]
#[case::gap_naming_no_clause(
    card("X", "", "Creature", ANGEL_ROLL),
    FLYING,
    vec![row("K", "Flying", &[0], None), row("A", "Draw seven cards and win.", &[], Some("effect:Draw"))],
    vec!["UnmappedGap", "OpenWithoutGap"],
)]
#[case::nothing_runs(
    card("X", "", "Creature", "{2}, {T}: Roll a six-sided die."),
    r#"{"faces":[{"abilities":[]}]}"#,
    vec![row("A", "{2}, {T}: Roll a six-sided die.", &[], Some("effect:RollDice"))],
    vec!["NothingRuns"],
)]
#[case::a_keyword_by_hand(
    card("X", "", "Creature", "Flying\nTrample"),
    FLYING,
    vec![row("K", "Flying", &[0], None), row("K", "Trample", &[], Some("keyword:Trample"))],
    vec!["UnsafeLine"],
)]
#[case::a_static_by_hand(
    card("X", "", "Creature", "Flying\nOther creatures you control get +1/+1."),
    FLYING,
    vec![row("K", "Flying", &[0], None), row("S", "Other creatures you control get +1/+1.", &[], Some("static:Anthem"))],
    vec!["UnsafeLine"],
)]
#[case::a_restriction_by_hand(
    card("X", "", "Creature", "Flying\nThis creature can't block."),
    FLYING,
    vec![row("K", "Flying", &[0], None), row("S", "CARDNAME can't block.", &[], Some("static:CantBlock"))],
    vec!["UnsafeLine"],
)]
fn a_card_that_cannot_be_proved_is_refused_by_name(
    #[case] printed: PrintedCard,
    #[case] rules: &str,
    #[case] ledger: Vec<LedgerLine>,
    #[case] expected: Vec<&str>,
) {
    let result = prove(&report(&printed, rules), &ledger);
    assert_eq!(names(&result), expected, "{result:?}");
}

#[test]
fn an_activated_ability_by_hand_beside_a_keyword_is_proved() {
    let printed = card("X", "", "Creature", ANGEL_ROLL);
    let ledger = vec![
        row("K", "Flying", &[0], None),
        row(
            "A",
            "{2}, {T}: Roll a six-sided die.",
            &[],
            Some("effect:RollDice"),
        ),
    ];
    let proof = prove(&report(&printed, FLYING), &ledger).unwrap();
    let marks: Vec<Mark> = proof.marks.iter().map(|m| m.mark).collect();
    assert_eq!(marks, vec![Mark::Auto, Mark::ByHand]);
}

/// The text of a face outside its clauses and its reminder text.
fn leftover(text: &str, spans: &[(u32, u32)]) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut covered = vec![false; chars.len()];
    for (start, end) in spans {
        let end = (*end as usize).min(chars.len());
        let start = (*start as usize).min(end);
        covered[start..end].fill(true);
    }
    let mut depth = 0u32;
    let mut out = String::new();
    for (at, c) in chars.iter().enumerate() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 && !covered[at] => out.push(*c),
            _ => {}
        }
    }
    out
}

#[test]
fn the_clauses_and_the_reminder_text_cover_all_the_text_of_every_card() {
    let mut lost = Vec::new();
    for printed in printed() {
        let compiled = compile(&printed);
        for (face, shown) in printed.faces.iter().enumerate() {
            let spans: Vec<(u32, u32)> = compiled
                .clauses
                .iter()
                .filter(|c| c.face == face)
                .map(|c| (c.span.start, c.span.end))
                .collect();
            let rest = leftover(&shown.oracle_text, &spans);
            let rest: String = rest
                .chars()
                .filter(|c| !c.is_whitespace() && !matches!(c, ',' | ';' | '.' | '\u{2014}'))
                .collect();
            if !rest.is_empty() {
                lost.push(format!("{}: {rest}", printed.name));
            }
        }
    }
    assert!(
        lost.is_empty(),
        "{} cards: {:?}",
        lost.len(),
        &lost[..lost.len().min(8)]
    );
}
