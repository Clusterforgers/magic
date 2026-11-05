//! Goal: round 6 piece 5 translates the memory a step keeps (the cards a draw drew, the hand a reveal showed) to a name the later lines read, and names a gap when the memory would be read outside the step that wrote it or the value is unknown.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn spell(body: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {body}\n")
}

#[rstest]
#[case::draw_true(
    "Draw | NumCards$ 3 | RememberDrawn$ True | SubAbility$ D\nSVar:D:DB$ Discard | Defined$ You | NumCards$ 1 | Mode$ TgtChoose | DiscardValid$ Card.IsRemembered",
    json!([{"Draw": {"count": 3, "as": "drawn"}}, {"Discard": {"filter": [{"Is": {"Bound": "drawn"}}]}}])
)]
#[case::draw_all_replaced_counts_the_cards_drawn(
    "Draw | NumCards$ 2 | RememberDrawn$ AllReplaced | SubAbility$ D\nSVar:D:DB$ Discard | Defined$ You | NumCards$ Y | Mode$ TgtChoose\nSVar:Y:Remembered$Amount",
    json!([{"Draw": {"count": 2, "as": "drawn"}}, {"Discard": {"count": {"Bound": "drawn~moved"}}}])
)]
fn a_draw_that_remembers_names_the_cards_it_drew(#[case] line: &str, #[case] nodes: Value) {
    let text = spell(line);
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(abilities(&text)[0]["Spell"]["effects"], nodes);
}

#[rstest]
#[case::group_draw(
    "Draw | Defined$ Player | RememberDrawn$ True",
    "key:Draw.RememberDrawn.group"
)]
#[case::unknown_value("Draw | RememberDrawn$ Maybe", "key:Draw.RememberDrawn=Maybe")]
#[case::inside_a_condition(
    "Draw | ConditionPresent$ Creature.YouCtrl | RememberDrawn$ True | SubAbility$ L\nSVar:L:DB$ GainLife | LifeAmount$ 3 | ConditionDefined$ Remembered | ConditionPresent$ Card | ConditionCompare$ EQ0",
    "defined:Remembered-without-a-change"
)]
#[case::inside_a_may(
    "Draw | Optional$ True | RememberDrawn$ True | SubAbility$ L\nSVar:L:DB$ Discard | Defined$ You | NumCards$ 2 | ConditionDefined$ Remembered | ConditionPresent$ Card | ConditionCompare$ GE1",
    "defined:Remembered-without-a-change"
)]
fn a_memory_that_cannot_be_read_is_a_gap(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&spell(line)), [gap]);
}

#[rstest]
#[case::white(
    "RevealHand | ValidTgts$ Player | RememberRevealed$ True | SubAbility$ D\nSVar:D:DB$ DealDamage | Defined$ Targeted | NumDmg$ X\nSVar:X:Remembered$Valid Card.White",
    "White"
)]
fn a_revealed_hand_is_named_as_it_was_shown(#[case] line: &str, #[case] color: &str) {
    let text = spell(line);
    assert_eq!(gaps(&text), Vec::<String>::new());
    let effects = &abilities(&text)[0]["Spell"]["effects"];
    assert_eq!(
        effects[0],
        json!({"Bind": {"as": "revealed", "what": {"Each": [{"OwnedBy": {"Target": 0}}, {"In": "Hand"}]}}})
    );
    assert_eq!(
        effects[1],
        json!({"Reveal": {"what": {"Bound": "revealed"}}})
    );
    assert_eq!(
        effects[2]["DealDamage"]["amount"]["Count"][2],
        json!({"Color": color})
    );
}

#[test]
fn a_reveal_that_remembers_nothing_stays_one_step() {
    let text = spell("RevealHand | ValidTgts$ Player");
    assert_eq!(
        abilities(&text)[0]["Spell"]["effects"],
        json!([{"Reveal": {"what": {"Each": [{"OwnedBy": {"Target": 0}}, {"In": "Hand"}]}}}])
    );
}
