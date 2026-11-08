//! Goal: a whole translation with a clause that reads back as other text (`Differs`) is neither offered nor promoted to a
//! draft, named `readback:Differs`; one that reads back (`Accepted`) or cannot be read back (`Unchecked`) still is.
use std::collections::BTreeSet;

use rstest::rstest;
use serde_json::{Value, json};

use mtg_cards::load::Library;
use mtg_cards::proposal::{Verdict, check_exact};
use mtg_cards::support::{PrintedCard, PrintedFace};
use mtg_db::SOURCE_FORGE;

use crate::w2a_promote::W2aSkip;
use crate::w2a_promote::draft::decide;

fn printed() -> PrintedCard {
    PrintedCard {
        oracle_id: "00000000-0000-4000-8000-0000000f2100".into(),
        name: "Alpha".into(),
        keywords: Vec::new(),
        layout: "normal".into(),
        faces: vec![PrintedFace {
            name: "Alpha".into(),
            mana_cost: "{U}".into(),
            type_line: "Sorcery".into(),
            oracle_text: "Draw a card.".into(),
            ..PrintedFace::default()
        }],
    }
}

#[rstest]
#[case::reads(json!({"Draw": {}}), Verdict::Accepted, None)]
#[case::another_count(json!({"Draw": {"count": 2}}), Verdict::Mismatch, Some("offer:readback:Differs"))]
#[case::another_player(
    json!({"Draw": {"who": "EachPlayer"}}),
    Verdict::Mismatch,
    Some("offer:readback:Differs")
)]
#[case::unwritten(
    json!({"Draw": {"count": {"Count": [{"Named": "Alpha"}, {"In": "Graveyard"}]}}}),
    Verdict::Unchecked,
    None
)]
fn a_clause_that_reads_back_as_other_text_is_not_offered_or_drafted(
    #[case] draw: Value,
    #[case] verdict: Verdict,
    #[case] refused: Option<&str>,
) {
    let card = printed();
    let caps = mtg_engine::caps();
    let names = BTreeSet::from(["alpha".to_owned()]);
    let json = json!({"faces": [{"abilities": [{"Spell": {"effects": [draw]}}]}]}).to_string();
    let report = check_exact(&json, &card, &Library::default(), &names, &caps);
    assert_eq!(report.verdict(), verdict, "{:?}", report.clauses);
    let refusal = crate::translate::stored::refusal(&report, &caps);
    assert_eq!(refusal.as_deref(), refused);
    let drafted = decide(
        &card,
        "alpha".into(),
        report.clone(),
        SOURCE_FORGE.into(),
        true,
        false,
    );
    match refused {
        Some(_) => assert!(matches!(drafted, Err(W2aSkip::Differs)), "{drafted:?}"),
        None => assert!(drafted.is_ok(), "{drafted:?}"),
    }
    // With `--differs` the same card is a draft, marked exactly when a clause reads back as other text.
    let marked = decide(
        &card,
        "alpha".into(),
        report,
        SOURCE_FORGE.into(),
        true,
        true,
    );
    assert_eq!(
        marked.map(|promotion| promotion.differs).ok(),
        Some(refused.is_some())
    );
}
