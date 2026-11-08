//! Goal: round 14 pack e14b reads the morph costs that take more than mana as the cards print them ("Reveal a white card in your hand", "Return two Islands you control to their owner's hand", "Discard a Zombie card") and writes "If ~ would enter, sacrifice two untapped lands instead. If you do, put ~ onto the battlefield. If you don't, put it into its owner's graveyard" back; another count, kind, zone, a missing "may" or "you may" too many does not read back.
use super::*;
use crate::compile::read::cost_in_words;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

#[rstest]
#[case::reveal_in_hand(
    "Reveal a white card in your hand",
    r#"[{"RevealFromHand":{"filter":[{"Color":"White"}]}}]"#
)]
#[case::reveal_from_hand(
    "Reveal a red card from your hand",
    r#"[{"RevealFromHand":{"filter":[{"Color":"Red"}]}}]"#
)]
#[case::two_islands(
    "Return two Islands you control to their owner's hand",
    r#"[{"ReturnToHand":{"count":2,"filter":[{"Subtype":"Island"}]}}]"#
)]
#[case::one_land(
    "Return a land you control to its owner's hand",
    r#"[{"ReturnToHand":{"filter":[{"Type":"Land"}]}}]"#
)]
#[case::a_zombie_card(
    "Discard a Zombie card",
    r#"[{"Discard":{"filter":[{"Subtype":"Zombie"}]}}]"#
)]
fn a_morph_cost_in_words_reads_as_its_parts(#[case] text: &str, #[case] json: &str) {
    let expected: Cost = serde_json::from_str(json).unwrap();
    assert_eq!(cost_in_words(text), Some(expected), "{text}");
}

#[rstest]
#[case::reveal_in_a_graveyard("Reveal a white card in your graveyard")]
#[case::one_returned_to_their_hand("Return a land you control to their owner's hand")]
#[case::two_returned_to_its_hand("Return two Islands you control to its owner's hand")]
fn a_cost_in_other_words_does_not_read(#[case] text: &str) {
    assert_eq!(cost_in_words(text), None, "{text}");
}

const TWO_LANDS: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"PayOrGraveyard":{"cost":[{"Sacrifice":{"count":2,"filter":[{"Type":"Land"},"Untapped"]}}]}}}}"#;
const A_PLAINS: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"PayOrGraveyard":{"cost":[{"Sacrifice":{"filter":[{"Subtype":"Plains"}]}}]}}}}"#;
const A_LAND_CARD: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"PayOrGraveyard":{"cost":[{"Discard":{"filter":[{"Type":"Land"}]}}],"optional":true}}}}"#;
const A_LAND_CARD_MUST: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"PayOrGraveyard":{"cost":[{"Discard":{"filter":[{"Type":"Land"}]}}]}}}}"#;

const LOTUS_VALE: &str = "If ~ would enter, sacrifice two untapped lands instead. If you do, put ~ onto the battlefield. If you don't, put it into its owner's graveyard.";
const KJELDORAN_OUTPOST: &str = "If ~ would enter, sacrifice a Plains instead. If you do, put ~ onto the battlefield. If you don't, put it into its owner's graveyard.";
const MOX_DIAMOND: &str = "If ~ would enter, you may discard a land card instead. If you do, put ~ onto the battlefield. If you don't, put it into its owner's graveyard.";

#[rstest]
#[case::lotus_vale(TWO_LANDS, LOTUS_VALE, true)]
#[case::kjeldoran_outpost(A_PLAINS, KJELDORAN_OUTPOST, true)]
#[case::mox_diamond(A_LAND_CARD, MOX_DIAMOND, true)]
#[case::another_count(A_PLAINS, LOTUS_VALE, false)]
#[case::another_kind(TWO_LANDS, KJELDORAN_OUTPOST, false)]
#[case::a_missing_may(A_LAND_CARD_MUST, MOX_DIAMOND, false)]
#[case::a_may_too_many(
    A_LAND_CARD,
    "If ~ would enter, discard a land card instead. If you do, put ~ onto the battlefield. If you don't, put it into its owner's graveyard.",
    false
)]
#[case::another_zone(
    A_PLAINS,
    "If ~ would enter, sacrifice a Plains instead. If you do, put ~ onto the battlefield. If you don't, exile it.",
    false
)]
fn an_entering_paid_for_or_put_into_the_graveyard_writes_back(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] same: bool,
) {
    assert_eq!(written(json, clause), same, "{clause}");
}
