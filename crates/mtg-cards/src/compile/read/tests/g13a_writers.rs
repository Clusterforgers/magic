//! Goal: the round 13 writers give the oracle wording of a search for one card of each of two kinds whatever the found cards are called ("and" or "and/or"), life equal to the difference between a creature's power and toughness, a spell that targets an opponent or a creature an opponent controls, and a creature attacking alone or attacking you or a planeswalker you control; a rule of another meaning (no card left out, other stats or object, other player, attacking only you) never reads back as the clause.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn pair(second: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Search":{{"filter":[{{"Type":"Creature"}}],"reveal":true,"as":"found"}}}},{{"Search":{{"filter":{second},"reveal":true,"as":"found2"}}}},{{"Move":{{"what":{{"Bound":"found"}},"to":"Hand"}}}},{{"Move":{{"what":{{"Bound":"found2"}},"to":"Hand"}}}},{{"Shuffle":{{}}}}]}}}}"#
    )
}

const LEFT_OUT: &str = r#"[{"Type":"Artifact"},{"Not":{"Is":{"Bound":"found"}}}]"#;
const KEPT_IN: &str = r#"[{"Type":"Artifact"}]"#;
const AND: &str = "Search your library for a creature card and an artifact card, reveal them, put them into your hand, then shuffle.";
const AND_OR: &str = "Search your library for a creature card and/or an artifact card, reveal them, put them into your hand, then shuffle.";

#[rstest]
#[case::and(pair(LEFT_OUT), AND, true)]
#[case::and_or(pair(LEFT_OUT), AND_OR, true)]
#[case::second_may_find_the_first(pair(KEPT_IN), AND, false)]
#[case::and_or_of_the_same(pair(KEPT_IN), AND_OR, false)]
fn g13a_one_card_of_each_kind(#[case] json: String, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(&json, clause), reads, "{json}");
}

fn jaws(left: &str, right: &str) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{{"LoseLife":{{"amount":{{"Difference":[{left},{right}]}},"who":{{"Target":0}}}}}}],"targets":[{{"what":"Opponent"}}],"when":{{"ZoneChange":{{"to":"Battlefield","what":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}]}}}}}}}}"#
    )
}

const POWER: &str = r#"{"Stat":{"of":"Triggering","stat":"Power"}}"#;
const TOUGHNESS: &str = r#"{"Stat":{"of":"Triggering","stat":"Toughness"}}"#;
const OWN_TOUGHNESS: &str = r#"{"Stat":{"of":"This","stat":"Toughness"}}"#;
const JAWS: &str = "Whenever a creature you control enters, target opponent loses life equal to the difference between that creature's power and its toughness.";

#[rstest]
#[case::power_and_toughness(jaws(POWER, TOUGHNESS), true)]
#[case::power_twice(jaws(POWER, POWER), false)]
#[case::another_creatures_toughness(jaws(POWER, OWN_TOUGHNESS), false)]
fn g13a_life_equal_to_a_difference(#[case] json: String, #[case] reads: bool) {
    assert_eq!(written(&json, JAWS), reads, "{json}");
}

fn targeting(player: &str) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{{"PutCounters":{{"counter":"+1/+1","on":"This"}}}}],"when":{{"SpellCast":{{"by":"You","what":[{{"AnyOf":[{{"TargetsPlayer":"{player}"}},{{"Targeting":[{{"Type":"Creature"}},{{"ControlledBy":"Opponents"}},{{"In":"Battlefield"}}]}}]}}]}}}}}}}}"#
    )
}

const DANITHA: &str = "Whenever you cast a spell that targets an opponent or a creature an opponent controls, put a +1/+1 counter on ~.";

#[rstest]
#[case::an_opponent("Opponents", true)]
#[case::you("You", false)]
fn g13a_a_spell_that_targets_an_opponent(#[case] player: &str, #[case] reads: bool) {
    assert_eq!(written(&targeting(player), DANITHA), reads, "{player}");
}

fn attacking(what: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{W}}"}},"SacrificeSelf"],"effects":[{{"Exile":{{"what":{{"Target":0}}}}}}],"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},{what}]}}}}]}}}}"#
    )
}

const ALONE: &str = r#""AttackingAlone""#;
const YOU_OR_YOURS: &str =
    r#"{"AnyOf":[{"AttackingPlayer":"You"},{"AttackingPlaneswalkerOf":"You"}]}"#;
const YOU_ONLY: &str = r#"{"AttackingPlayer":"You"}"#;
const SNARE: &str =
    "{W}, Sacrifice ~: Exile target creature that's attacking you or a planeswalker you control.";
const LONE: &str = "{W}, Sacrifice ~: Exile target creature that's attacking alone.";

#[rstest]
#[case::alone(ALONE, LONE, true)]
#[case::you_or_yours(YOU_OR_YOURS, SNARE, true)]
#[case::you_only_for_you_or_yours(YOU_ONLY, SNARE, false)]
#[case::alone_for_you_or_yours(ALONE, SNARE, false)]
#[case::you_or_yours_for_alone(YOU_OR_YOURS, LONE, false)]
fn g13a_how_a_target_attacks(#[case] what: &str, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(&attacking(what), clause), reads, "{what}");
}
