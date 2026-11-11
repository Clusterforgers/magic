//! Goal: Pain's Reward's bidding reads as a `BidLife` and the loss and draw of the high bidder, writes back as the
//! printed text, and a near miss of the text or of the rules reads or writes as something else.
use super::*;

const PAINS_REWARD: &str = "Each player may bid life. You start the bidding with a bid of any number. In turn order, each player may top the high bid. The bidding ends if the high bid stands. The high bidder loses life equal to the high bid and draws four cards.";

const RULES: &str = r#"{"Spell":{"effects":[{"BidLife":{"as":"bid","winner":"bidder"}},{"LoseLife":{"who":{"Bound":"bidder"},"amount":{"ChosenAs":"bid"}}},{"Draw":{"who":{"Bound":"bidder"},"count":4}}]}}"#;

#[test]
fn pains_reward_reads_and_writes_back() {
    reads_as(PAINS_REWARD, ClauseKind::Spell, false, RULES);
}

#[rstest]
#[case::a_bid_of_zero("a bid of any number", "a bid of 0")]
#[case::gains_life("loses life equal", "gains life equal")]
#[case::one_card("draws four cards", "draws a card")]
#[case::without_turn_order("In turn order, each", "Each")]
fn a_near_miss_of_the_text_stays_unread(#[case] from: &str, #[case] to: &str) {
    let text = PAINS_REWARD.replace(from, to);
    assert_eq!(clause(&text, ClauseKind::Spell, false), None, "{text}");
}

#[rstest]
#[case::a_fixed_opening(r#""BidLife":{"#, r#""BidLife":{"start":0,"#)]
#[case::opponents_bid(r#""BidLife":{"#, r#""BidLife":{"who":"Opponents","#)]
#[case::you_lose(r#""LoseLife":{"who":{"Bound":"bidder"},"#, r#""LoseLife":{"#)]
#[case::three_cards(r#""count":4"#, r#""count":3"#)]
fn a_near_miss_of_the_rules_is_not_written_as_the_text(#[case] from: &str, #[case] to: &str) {
    let json = RULES.replacen(from, to, 1);
    let written = render_variants(&ability(&json));
    assert!(
        !written.iter().any(|said| same_text(said, PAINS_REWARD)),
        "{json}: {written:?}"
    );
}
