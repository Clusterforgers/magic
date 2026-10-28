//! Goal: X in a trigger on becoming monstrous reads as the amount of the event (the X of the monstrosity) wherever it stands, writes back as the card's text, and X stays unread in a trigger on another event.
use super::*;

#[rstest]
#[case::hydras(
    "When this creature becomes monstrous, create X X/X green Hydra creature tokens.",
    r#"{"Triggered":{"when":{"BecomesMonstrous":{"what":["This"]}},"effects":[{"CreateToken":{"token":{"Sized":{"size":"Triggering","token":{"name":"Hydra","colors":["Green"],"types":["Creature"],"subtypes":["Hydra"]}}},"count":"Triggering"}}]}}"#
)]
#[case::draw("When this creature becomes monstrous, draw X cards.", r#"{"Triggered":{"when":{"BecomesMonstrous":{"what":["This"]}},"effects":[{"Draw":{"count":"Triggering"}}]}}"#)]
#[case::no_x(
    "When this creature becomes monstrous, draw a card.",
    r#"{"Triggered":{"when":{"BecomesMonstrous":{"what":["This"]}},"effects":[{"Draw":{}}]}}"#
)]
fn c10_pa_x_of_a_monstrosity_reads_as_the_amount_of_the_event(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::renowned("When this creature becomes renowned, draw X cards.")]
#[case::enters("When this creature enters, draw X cards.")]
fn c10_pa_x_of_another_trigger_stays_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}
