//! Goal: a `Dig` whose Oracle text reveals the card taken from among the cards looked at shows that card before it goes
//! to the hand; a card put into the hand unrevealed, onto the battlefield, or under `NoReveal$` is not revealed.
use rstest::rstest;
use serde_json::Value;

use super::abilities;

fn dig(params: &str, oracle: &str) -> Vec<Value> {
    let script = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ Dig | DigNum$ 4 | ChangeNum$ 1 | Optional$ True | {params} | SpellDescription$ x\nOracle:{oracle}\n"
    );
    let found = abilities(&script);
    found
        .first()
        .and_then(|ability| ability.pointer("/Spell/effects"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// The variant names of the effects, in order.
fn kinds(effects: &[Value]) -> Vec<String> {
    effects
        .iter()
        .filter_map(|effect| effect.as_object()?.keys().next().cloned())
        .collect()
}

const REVEALED: &str = "Look at the top four cards of your library. You may reveal a creature card from among them and put it into your hand. Put the rest on the bottom of your library in a random order.";
const UNREVEALED: &str = "Look at the top four cards of your library. You may put a creature card from among them into your hand. Put the rest on the bottom of your library in a random order.";
const BATTLEFIELD: &str = "Look at the top four cards of your library. You may reveal a creature card from among them and put it onto the battlefield. Put the rest on the bottom of your library in a random order.";

#[test]
fn the_card_taken_is_revealed_when_the_oracle_text_says_so() {
    let effects = dig("ChangeValid$ Creature | RestRandomOrder$ True", REVEALED);
    assert_eq!(
        kinds(&effects),
        ["LookAtTop", "Choose", "Reveal", "Move", "Move"]
    );
    assert_eq!(
        effects[2].pointer("/Reveal/what"),
        effects[3].pointer("/Move/what"),
        "the card revealed is the card moved"
    );
}

#[rstest]
#[case::not_said("ChangeValid$ Creature | RestRandomOrder$ True", UNREVEALED)]
#[case::no_reveal(
    "ChangeValid$ Creature | RestRandomOrder$ True | NoReveal$ True",
    REVEALED
)]
#[case::battlefield(
    "ChangeValid$ Creature | RestRandomOrder$ True | DestinationZone$ Battlefield",
    BATTLEFIELD
)]
#[case::any_card("RestRandomOrder$ True", REVEALED)]
fn no_other_dig_reveals_the_card_taken(#[case] params: &str, #[case] oracle: &str) {
    let effects = dig(params, oracle);
    assert!(!effects.is_empty(), "{params}");
    assert!(
        !kinds(&effects).iter().any(|kind| kind == "Reveal"),
        "{params}: {effects:?}"
    );
}
