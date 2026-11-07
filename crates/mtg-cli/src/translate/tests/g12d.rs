//! Goal: round 12 pack g12d: a `Repeat` ended by a check is a do-while, and `DigUntil` reads the rest's places, an
//! optional found move, the revealing player's shuffle and permanent cards.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn spell(line: &str, rest: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ {line}\nSVar:Body:DB$ GainLife | LifeAmount$ 2\nSVar:X:Count$ValidHand Card.YouOwn\n{rest}"
    )
}

fn effects(script: &str) -> Value {
    let found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    found[0]["Spell"]["effects"].clone()
}

/// A count or a presence checked after each pass is a repeat done once and then while it holds, bounded by `MaxRepeat`.
#[rstest]
#[case::count(
    "RepeatCheckSVar$ X | RepeatSVarCompare$ LT7",
    json!({"Compare": {"left": {"Count": [{"OwnedBy": "You"}, {"In": "Hand"}]}, "op": "Lt", "right": 7}}),
    1000
)]
#[case::count_at_least_one(
    "RepeatCheckSVar$ X",
    json!({"Compare": {"left": {"Count": [{"OwnedBy": "You"}, {"In": "Hand"}]}, "op": "Ge", "right": 1}}),
    1000
)]
#[case::bounded(
    "RepeatCheckSVar$ X | RepeatSVarCompare$ GE8 | MaxRepeat$ 2",
    json!({"Compare": {"left": {"Count": [{"OwnedBy": "You"}, {"In": "Hand"}]}, "op": "Ge", "right": 8}}),
    2
)]
fn a_checked_repeat_is_a_do_while(#[case] keys: &str, #[case] condition: Value, #[case] most: u32) {
    let script = spell(&format!("Repeat | RepeatSubAbility$ Body | {keys}"), "");
    assert_eq!(
        effects(&script),
        json!([{"RepeatWhile": {"at_least_once": true, "condition": condition,
            "effects": [{"GainLife": {"amount": 2}}], "most": most}}])
    );
}

/// A check stated before the next pass, a compare with nothing to compare and a choice to go on stay gaps.
#[rstest]
#[case::check_first(
    "RepeatCheckSVar$ X | RepeatSVarCompare$ LT7 | StackDescription$ If you have fewer than 7 cards in hand, repeat this process.",
    "key:Repeat.check-first"
)]
#[case::compare_alone(
    "RepeatSVarCompare$ EQ1 | RepeatPresent$ Card | RepeatDefined$ Remembered",
    "key:Repeat.RepeatSVarCompare"
)]
#[case::defined_alone(
    "RepeatDefined$ Remembered | RepeatCheckSVar$ X",
    "key:Repeat.RepeatDefined"
)]
#[case::optional(
    "RepeatCheckSVar$ X | RepeatOptional$ True",
    "key:Repeat.RepeatOptional"
)]
#[case::max_amount(
    "RepeatCheckSVar$ X | MaxRepeat$ X",
    "key:Repeat.MaxRepeat-with-a-check"
)]
fn other_checked_repeats_are_gaps(#[case] keys: &str, #[case] gap: &str) {
    let script = spell(&format!("Repeat | RepeatSubAbility$ Body | {keys}"), "");
    assert_eq!(gaps(&script), [gap]);
}

/// The steps of a `DigUntil` after the revealing, for each place the rest go and each way the found card moves.
#[rstest]
#[case::rest_to_bottom_by_default(
    "Valid$ Creature | FoundDestination$ Battlefield | RevealedDestination$ Library | Shuffle$ True",
    json!([{"Move": {"to": {"Battlefield": {}}, "what": {"Bound": "found"}}},
        {"Move": {"to": {"Library": {"order": "Chosen", "place": "Bottom"}}, "what": {"Bound": "revealed2"}}},
        {"Shuffle": {}}])
)]
#[case::random_bottom(
    "Valid$ Creature | FoundDestination$ Battlefield | RevealedDestination$ Library | RevealRandomOrder$ True",
    json!([{"Move": {"to": {"Battlefield": {}}, "what": {"Bound": "found"}}},
        {"Move": {"to": {"Library": {"order": "Random", "place": "Bottom"}}, "what": {"Bound": "revealed2"}}}])
)]
#[case::all_to_hand(
    "Valid$ Card.nonLand | RevealedDestination$ Hand",
    json!([{"Move": {"to": "Hand", "what": {"Bound": "revealed2"}}}])
)]
#[case::may_put(
    "Valid$ Creature | FoundDestination$ Battlefield | OptionalFoundMove$ True | RevealedDestination$ Library | RevealedLibraryPosition$ -1",
    json!([{"May": {"effects": [{"Move": {"to": {"Battlefield": {}}, "what": {"Bound": "found"}}}]}},
        {"Move": {"to": {"Library": {"order": "Chosen", "place": "Bottom"}}, "what": {"Bound": "revealed2"}}}])
)]
#[case::their_shuffle(
    "Defined$ Opponent | Valid$ Creature | FoundDestination$ Battlefield | RevealedDestination$ Library | Shuffle$ True",
    json!([{"Move": {"to": {"Battlefield": {}}, "what": {"Bound": "found"}}},
        {"Move": {"to": {"Library": {"order": "Chosen", "place": "Bottom"}}, "what": {"Bound": "revealed2"}}},
        {"Shuffle": {"who": "Opponents"}}])
)]
fn a_dig_until_moves_the_found_and_the_rest(#[case] keys: &str, #[case] after: Value) {
    let found = effects(&spell(&format!("DigUntil | {keys}"), ""));
    let steps = found
        .as_array()
        .map(|list| list[1..].to_vec())
        .unwrap_or_default();
    assert_eq!(Value::Array(steps), after);
}

/// A permanent card in a library is one of a permanent type: the found card's filter says so.
#[rstest]
#[case::permanent("Permanent", json!([]))]
#[case::nonland_permanent("Permanent.nonLand", json!([{"Not": {"Type": "Land"}}]))]
fn a_permanent_card_found_is_of_a_permanent_type(#[case] valid: &str, #[case] first: Value) {
    let found = effects(&spell(
        &format!(
            "DigUntil | Valid$ {valid} | FoundDestination$ Hand | RevealedDestination$ Graveyard"
        ),
        "",
    ));
    let mut want: Vec<Value> = first.as_array().cloned().unwrap_or_default();
    want.push(
        json!({"AnyOf": [{"Type": "Artifact"}, {"Type": "Battle"}, {"Type": "Creature"},
            {"Type": "Enchantment"}, {"Type": "Land"}, {"Type": "Planeswalker"}]}),
    );
    assert_eq!(found[0]["RevealUntil"]["filter"], Value::Array(want));
}

/// "Put any number of those" among several found cards is a choice, not one "may": a gap.
#[test]
fn an_optional_move_of_several_found_cards_is_a_gap() {
    let script = spell(
        "DigUntil | Amount$ 2 | Valid$ Creature | FoundDestination$ Battlefield | OptionalFoundMove$ True | RevealedDestination$ Library",
        "",
    );
    assert_eq!(gaps(&script), ["key:DigUntil.OptionalFoundMove"]);
}

fn draw_cost(count: &str) -> String {
    format!(
        "Name:T\nTypes:Creature\nPT:2/2\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigDraw | OptionalDecider$ You | TriggerDescription$ x\nSVar:TrigDraw:AB$ Discard | Cost$ Draw<{count}/You> | Defined$ You | NumCards$ 2 | Mode$ TgtChoose\nSVar:X:Count$CardPower\n"
    )
}

/// A counted draw paid on resolution ("you may draw cards equal to its power; if you do, discard two") is a draw of that
/// count; a fixed count stays a number and nought stays a gap.
#[rstest]
#[case::counted("X", json!({"Stat": {"of": "This", "stat": "Power"}}))]
#[case::fixed("2", json!(2))]
fn a_counted_draw_cost_is_paid_as_it_resolves(#[case] count: &str, #[case] drawn: Value) {
    let found = abilities(&draw_cost(count));
    assert_eq!(found.len(), 1, "{:?}", gaps(&draw_cost(count)));
    let text = found[0].to_string();
    let want = json!({"Pay": {"cost": [{"Draw": drawn}], "paid": [{"Discard": {"count": 2}}]}});
    assert!(text.contains(&want.to_string()), "{text}");
}

/// "Put a -1/-1 counter on a creature you control" as a cost is blight; another counter or another creature stays a gap.
#[rstest]
#[case::blight("AddCounter<1/M1M1/Creature.YouCtrl/a creature you control>", Some(json!([{"Blight": 1}, "Tap"])))]
#[case::two("AddCounter<2/M1M1/Creature.YouCtrl>", Some(json!([{"Blight": 2}, "Tap"])))]
#[case::plus_counter("AddCounter<1/P1P1/Creature.YouCtrl/a creature you control>", None)]
#[case::their_creature(
    "AddCounter<1/M1M1/Creature.OppCtrl/a creature an opponent controls>",
    None
)]
fn a_minus_counter_on_your_creature_is_blight(#[case] cost: &str, #[case] read: Option<Value>) {
    let script =
        format!("Name:T\nTypes:Creature\nPT:2/2\nA:AB$ Draw | Cost$ {cost} T | NumCards$ 1\n");
    match read {
        Some(cost) => assert_eq!(
            abilities(&script)[0]["Activated"]["cost"],
            cost,
            "{:?}",
            gaps(&script)
        ),
        None => assert_eq!(gaps(&script), ["cost:AddCounter"]),
    }
}

#[test]
fn a_draw_cost_of_nought_is_a_gap() {
    assert_eq!(gaps(&draw_cost("0")), ["cost:counted-draw-or-discard"]);
}

/// "Untap up to N lands": the controller picks, or the player `Defined$` names ("that player untaps a land").
#[rstest]
#[case::yours("Defined$ You", None)]
#[case::unsaid("", None)]
#[case::opponent("Defined$ Opponent", Some(json!("Opponents")))]
fn the_player_defined_picks_what_untaps(#[case] defined: &str, #[case] who: Option<Value>) {
    let found = effects(&spell(
        &format!("Untap | UntapUpTo$ True | UntapType$ Land | Amount$ 2 | {defined}"),
        "",
    ));
    let mut choose = json!({"Choose": {"as": "untap", "count": {"max": 2, "min": 0},
        "from": {"Each": [{"Type": "Land"}]}}});
    if let Some(who) = who {
        choose["Choose"]["who"] = who;
    }
    assert_eq!(
        found,
        json!([choose, {"Untap": {"what": {"Bound": "untap"}}}])
    );
}
