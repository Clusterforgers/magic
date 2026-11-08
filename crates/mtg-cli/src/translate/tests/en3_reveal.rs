//! Goal: `Reveal` maps to the card itself or to a choice among the cards of the player's own hand that is then shown, and
//! a shape beyond it (random, another player's hand, every matching card) stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn spell(line: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {line}\n")
}

#[rstest]
#[case::itself(
    "Reveal | RevealDefined$ Self",
    json!([{"Reveal": {"what": "This"}}])
)]
#[case::any_number_of_black_cards(
    "Reveal | Defined$ You | RevealValid$ Card.Black | AnyNumber$ True | RememberRevealed$ True",
    json!([
        {"Choose": {"from": {"Each": [{"Color": "Black"}, {"In": "Hand"}, {"OwnedBy": "You"}]},
            "count": {"min": 0, "max": null}, "as": "revealed"}},
        {"Reveal": {"what": {"Bound": "revealed"}}}
    ])
)]
#[case::up_to_one_red_card(
    "Reveal | RevealValid$ Card.Red | Optional$ True",
    json!([
        {"Choose": {"from": {"Each": [{"Color": "Red"}, {"In": "Hand"}, {"OwnedBy": "You"}]},
            "count": {"min": 0, "max": 1}, "as": "revealed"}},
        {"Reveal": {"what": {"Bound": "revealed"}}}
    ])
)]
#[case::one_creature(
    "Reveal | RevealValid$ Creature",
    json!([
        {"Choose": {"from": {"Each": [{"Type": "Creature"}, {"In": "Hand"}, {"OwnedBy": "You"}]}, "as": "revealed"}},
        {"Reveal": {"what": {"Bound": "revealed"}}}
    ])
)]
fn a_reveal_maps_to_the_nodes_that_show_the_cards(#[case] line: &str, #[case] nodes: Value) {
    let script = spell(line);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    assert_eq!(
        normal(found[0].clone()),
        normal(json!({"Spell": {"effects": nodes}}))
    );
}

#[rstest]
#[case::all_matching("Reveal | RevealAllValid$ Card.nonLand", "key:Reveal.RevealAllValid")]
#[case::another_player(
    "Reveal | Defined$ Opponent | RevealValid$ Creature",
    "key:Reveal.Defined"
)]
#[case::a_target_that_is_no_player(
    "Reveal | ValidTgts$ Creature | RevealValid$ Creature",
    "key:Reveal.ValidTgts"
)]
#[case::a_bare_subtype("Reveal | RevealValid$ Dragon | Optional$ True", "valid-base:Dragon")]
#[case::a_set_of_cards("Reveal | RevealDefined$ Remembered", "key:Reveal.RevealDefined")]
fn a_reveal_beyond_the_hand_stays_a_gap(#[case] line: &str, #[case] gap: &str) {
    let script = spell(line);
    assert_eq!(abilities(&script), Vec::<Value>::new());
    assert_eq!(gaps(&script), vec![gap.to_owned()]);
}
