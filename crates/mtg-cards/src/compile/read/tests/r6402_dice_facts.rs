//! Goal: "You may cast this card from your graveyard as long as you've rolled a N this turn. If you cast it this way and it would be put into your graveyard, exile it instead." reads into a permission of the card itself with the tally of the dice that came up N and the exile rider, and writes back as the card's text, for every result and article; a wrong article, a result of 0, another player's rolls and a rider about other spells stay unread or do not write back.
use super::*;

fn permission(result: u32, who: &str) -> String {
    format!(
        r#"{{"Rule":{{"rule":{{"MayCast":{{"what":["This"],"from":"Graveyard","riders":["ExiledInstead"]}}}},"condition":{{"Compare":{{"left":{{"Tallied":{{"who":"{who}","what":{{"R6402Rolled":{result}}}}}}},"op":"Ge","right":1}}}},"from":"Graveyard"}}}}"#
    )
}

fn text(article: &str, result: u32) -> String {
    format!(
        "You may cast ~ from your graveyard as long as you've rolled {article} {result} this turn. If you cast it this way and it would be put into your graveyard, exile it instead."
    )
}

#[rstest]
#[case::a_six("a", 6)]
#[case::a_one("a", 1)]
#[case::an_eight("an", 8)]
#[case::an_eleven("an", 11)]
#[case::a_twenty("a", 20)]
fn r6402_a_result_rolled_this_turn_reads_and_writes_back(
    #[case] article: &str,
    #[case] result: u32,
) {
    let words = text(article, result);
    reads_as(
        &words,
        ClauseKind::Static,
        false,
        &permission(result, "You"),
    );
    let found = written_variants(&ability(&permission(result, "You")));
    assert!(found.iter().any(|w| same_text(w, &words)), "{found:?}");
}

#[rstest]
#[case::a_wrong_article(text("an", 6))]
#[case::another_wrong_article(text("a", 8))]
#[case::a_zero(text("a", 0))]
#[case::not_this_turn(
    "You may cast ~ from your graveyard as long as you've rolled a 6. If you cast it this way and it would be put into your graveyard, exile it instead.".to_owned()
)]
fn r6402_near_misses_stay_unread(#[case] words: String) {
    assert_eq!(clause(&words, ClauseKind::Static, false), None, "{words}");
}

#[test]
fn r6402_another_players_rolls_do_not_write_back() {
    let found = written_variants(&ability(&permission(6, "Opponents")));
    assert!(
        !found.iter().any(|w| same_text(w, &text("a", 6))),
        "{found:?}"
    );
}
