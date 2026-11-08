//! Goal: fix20's writers give the oracle wording of "each player discards their hand, then returns ..." and of cards put
//! back from the hand "in any order", and a rule of another meaning (other count, other zone, no order, a "may" carried
//! over, no exile first) never reads back as the clause.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const GAINS: &str = "Exile ~. Each player discards their hand, then returns up to three cards from their graveyard to their hand.";
const WHEEL_BACK: &str = "Each player discards their hand, then returns up to three cards from their graveyard to their hand.";

fn gains(first: &str, min: u32, zone: &str) -> String {
    let returns = format!(
        r#"{{"RepeatEach":{{"as":"player","effects":[{{"Choose":{{"as":"picked","count":{{"max":3,"min":{min}}},"from":{{"Each":[{{"In":"{zone}"}},{{"OwnedBy":{{"Bound":"player"}}}}]}},"who":{{"Bound":"player"}}}}}},{{"Move":{{"to":"Hand","what":{{"Bound":"picked"}}}}}}],"over":"EachPlayer"}}}}"#
    );
    let discard = r#"{"Discard":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":"EachPlayer"}]},"who":"EachPlayer"}}"#;
    format!(r#"{{"Spell":{{"effects":[{first}{discard},{returns}]}}}}"#)
}

const EXILE: &str = r#"{"Exile":{"what":"This"}},"#;

#[rstest]
#[case::exile_first(gains(EXILE, 0, "Graveyard"), GAINS, true)]
#[case::without_exile(gains("", 0, "Graveyard"), WHEEL_BACK, true)]
#[case::exile_missing(gains("", 0, "Graveyard"), GAINS, false)]
#[case::exactly_three(gains(EXILE, 3, "Graveyard"), GAINS, false)]
#[case::from_exile(gains(EXILE, 0, "Exile"), GAINS, false)]
fn fix20_each_player_discards_then_returns(
    #[case] json: String,
    #[case] clause: &str,
    #[case] expected: bool,
) {
    assert_eq!(written(&json, clause), expected, "{json}");
}

const BRAINSTORM: &str =
    "Draw three cards, then put two cards from your hand on top of your library in any order.";

fn put_back(order: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Draw":{{"count":3}}}},{{"Choose":{{"as":"picked","count":{{"max":2,"min":2}},"from":{{"Each":[{{"In":"Hand"}},{{"OwnedBy":"You"}}]}}}}}},{{"Move":{{"to":{{"Library":{{{order}"place":"Top"}}}},"what":{{"Bound":"picked"}}}}}}]}}}}"#
    )
}

#[rstest]
#[case::chosen(put_back(r#""order":"Chosen","#), true)]
#[case::no_order(put_back(""), false)]
#[case::random(put_back(r#""order":"Random","#), false)]
fn fix20_cards_put_back_are_put_in_any_order(#[case] json: String, #[case] expected: bool) {
    assert_eq!(written(&json, BRAINSTORM), expected, "{json}");
}
