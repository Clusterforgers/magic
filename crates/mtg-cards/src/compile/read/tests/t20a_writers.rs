//! Goal: a trigger whose intervening "if" describes the object of its event reads back with the description in the event, and an "if" about the source, a count or another event never does.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// A cast of a spell by you with an intervening "if" and one draw.
fn cast_if(held: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"SpellCast":{{"by":"You"}}}},"if":{held},"effects":[{{"Draw":{{}}}}]}}}}"#
    )
}

/// A card of the given kind put into a graveyard from the battlefield with an intervening "if" and a counter on the source.
fn dies_if(kind: &str, held: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":[{kind}],"from":"Battlefield","to":"Graveyard"}}}},"if":{held},"effects":[{{"PutCounters":{{"counter":"+1/+1","on":"This"}}}}]}}}}"#
    )
}

const NONCREATURE: &str =
    r#"{"Matches":{"subject":"Triggering","selector":[{"Not":{"Type":"Creature"}}]}}"#;
const MV_THREE: &str =
    r#"{"Compare":{"left":{"Stat":{"of":"Triggering","stat":"ManaValue"}},"op":"Ge","right":3}}"#;
const YOURS: &str = r#"{"Matches":{"subject":"Triggering","selector":[{"ControlledBy":"You"}]}}"#;
const UNTAPPED_SOURCE: &str =
    r#"{"Matches":{"subject":"This","selector":[{"ControlledBy":"You"}]}}"#;

#[rstest]
#[case::noncreature(
    cast_if(NONCREATURE),
    "Whenever you cast a noncreature spell, draw a card."
)]
#[case::mana_value(
    cast_if(MV_THREE),
    "Whenever you cast a spell with mana value 3 or greater, draw a card."
)]
#[case::both(
    cast_if(&format!(r#"{{"All":[{MV_THREE},{NONCREATURE}]}}"#)),
    "Whenever you cast a noncreature spell with mana value 3 or greater, draw a card."
)]
#[case::both_other_order(
    cast_if(&format!(r#"{{"All":[{NONCREATURE},{MV_THREE}]}}"#)),
    "Whenever you cast a noncreature spell with mana value 3 or greater, draw a card."
)]
#[case::enchantment_you_control(
    dies_if(r#"{"Type":"Enchantment"}"#, YOURS),
    "Whenever an enchantment you control is put into a graveyard from the battlefield, put a +1/+1 counter on ~."
)]
#[case::creature_you_control_dies(
    dies_if(r#"{"Type":"Creature"}"#, YOURS),
    "Whenever a creature you control dies, put a +1/+1 counter on ~."
)]
fn t20a_the_event_carries_the_description(#[case] json: String, #[case] clause: &str) {
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::another_kind(
    cast_if(NONCREATURE),
    "Whenever you cast a creature spell, draw a card."
)]
#[case::another_number(
    cast_if(&format!(r#"{{"All":[{MV_THREE},{NONCREATURE}]}}"#)),
    "Whenever you cast a noncreature spell with mana value 4 or greater, draw a card."
)]
#[case::dropped_half(
    cast_if(&format!(r#"{{"All":[{MV_THREE},{NONCREATURE}]}}"#)),
    "Whenever you cast a noncreature spell, draw a card."
)]
#[case::another_controller(
    dies_if(r#"{"Type":"Enchantment"}"#, YOURS),
    "Whenever an enchantment an opponent controls is put into a graveyard from the battlefield, put a +1/+1 counter on ~."
)]
#[case::another_kind_moved(
    dies_if(r#"{"Type":"Enchantment"}"#, YOURS),
    "Whenever an artifact you control is put into a graveyard from the battlefield, put a +1/+1 counter on ~."
)]
#[case::no_description(cast_if(NONCREATURE), "Whenever you cast a spell, draw a card.")]
#[case::the_source_is_no_description(
    cast_if(UNTAPPED_SOURCE),
    "Whenever you cast a spell you control, draw a card."
)]
fn t20a_a_wrong_description_never_reads(#[case] json: String, #[case] clause: &str) {
    assert!(!written(&json, clause), "{clause}");
}

/// An upkeep trigger of the controller with a draw, when the greatest `stat` of their `own` objects is at least that of `all`.
fn best_of(own: &str, stat: &str, all: &str, op: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"Upkeep","whose":"You"}}}},"if":{{"Compare":{{"left":{{"Greatest":{{"of":[{own}],"stat":"{stat}"}}}},"op":"{op}","right":{{"Greatest":{{"of":[{all}],"stat":"{stat}"}}}}}}}},"effects":[{{"Draw":{{}}}}]}}}}"#
    )
}

#[rstest]
#[case::artifact_mana_value(
    best_of(
        r#"{"Type":"Artifact"},{"ControlledBy":"You"}"#,
        "ManaValue",
        r#"{"Type":"Artifact"}"#,
        "Ge"
    ),
    "At the beginning of your upkeep, if you control the artifact with the greatest mana value or tied for the greatest mana value, draw a card."
)]
#[case::creature_power(
    best_of(
        r#"{"Type":"Creature"},{"ControlledBy":"You"}"#,
        "Power",
        r#"{"Type":"Creature"}"#,
        "Ge"
    ),
    "At the beginning of your upkeep, if you control the creature with the greatest power or tied for the greatest power, draw a card."
)]
#[case::controlled_first(
    best_of(
        r#"{"ControlledBy":"You"},{"Type":"Creature"}"#,
        "Toughness",
        r#"{"Type":"Creature"}"#,
        "Ge"
    ),
    "At the beginning of your upkeep, if you control the creature with the greatest toughness or tied for the greatest toughness, draw a card."
)]
fn t20a_the_best_of_a_kind_reads(#[case] json: String, #[case] clause: &str) {
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::another_stat(
    best_of(
        r#"{"Type":"Artifact"},{"ControlledBy":"You"}"#,
        "ManaValue",
        r#"{"Type":"Artifact"}"#,
        "Ge"
    ),
    "At the beginning of your upkeep, if you control the artifact with the greatest power or tied for the greatest power, draw a card."
)]
#[case::another_kind(
    best_of(
        r#"{"Type":"Artifact"},{"ControlledBy":"You"}"#,
        "ManaValue",
        r#"{"Type":"Artifact"}"#,
        "Ge"
    ),
    "At the beginning of your upkeep, if you control the creature with the greatest mana value or tied for the greatest mana value, draw a card."
)]
#[case::strictly_greater_is_not_tied(
    best_of(
        r#"{"Type":"Artifact"},{"ControlledBy":"You"}"#,
        "ManaValue",
        r#"{"Type":"Artifact"}"#,
        "Gt"
    ),
    "At the beginning of your upkeep, if you control the artifact with the greatest mana value or tied for the greatest mana value, draw a card."
)]
#[case::compared_with_another_kind(
    best_of(
        r#"{"Type":"Artifact"},{"ControlledBy":"You"}"#,
        "ManaValue",
        r#"{"Type":"Creature"}"#,
        "Ge"
    ),
    "At the beginning of your upkeep, if you control the artifact with the greatest mana value or tied for the greatest mana value, draw a card."
)]
#[case::of_the_opponents(
    best_of(
        r#"{"Type":"Artifact"},{"ControlledBy":"Opponents"}"#,
        "ManaValue",
        r#"{"Type":"Artifact"}"#,
        "Ge"
    ),
    "At the beginning of your upkeep, if you control the artifact with the greatest mana value or tied for the greatest mana value, draw a card."
)]
fn t20a_the_best_of_a_kind_needs_the_same_kind_stat_and_tie(
    #[case] json: String,
    #[case] clause: &str,
) {
    assert!(!written(&json, clause), "{clause}");
}

/// A creature of yours enters; a draw if `held`, otherwise two counters on it.
fn either(held: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}],"to":"Battlefield"}}}},"effects":[{{"If":{{"condition":{held},"then":[{{"Draw":{{}}}}],"otherwise":[{{"PutCounters":{{"counter":"+1/+1","on":"Triggering","count":2}}}}]}}}}]}}}}"#
    )
}

/// The creature has `stat` compared with a number, as a selector test.
fn stat_is(stat: &str, op: &str, number: u32) -> String {
    format!(
        r#"{{"Matches":{{"subject":"Triggering","selector":[{{"Compare":{{"op":"{op}","stat":"{stat}","to":{number}}}}}]}}}}"#
    )
}

#[rstest]
#[case::power_at_least(
    either(&stat_is("Power", "Ge", 3)),
    "Whenever a creature you control enters, draw a card if its power is 3 or greater. Otherwise, put two +1/+1 counters on it."
)]
#[case::toughness_at_most(
    either(&stat_is("Toughness", "Le", 2)),
    "Whenever a creature you control enters, draw a card if its toughness is 2 or less. Otherwise, put two +1/+1 counters on it."
)]
#[case::mana_value_exactly(
    either(&stat_is("ManaValue", "Eq", 4)),
    "Whenever a creature you control enters, draw a card if its mana value is 4. Otherwise, put two +1/+1 counters on it."
)]
#[case::compare_form(
    either(
        r#"{"Compare":{"left":{"Stat":{"of":"Triggering","stat":"Power"}},"op":"Ge","right":3}}"#
    ),
    "Whenever a creature you control enters, draw a card if its power is 3 or greater. Otherwise, put two +1/+1 counters on it."
)]
fn t20a_a_choice_on_a_stat_of_the_creature_reads(#[case] json: String, #[case] clause: &str) {
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::another_number(
    either(&stat_is("Power", "Ge", 3)),
    "Whenever a creature you control enters, draw a card if its power is 4 or greater. Otherwise, put two +1/+1 counters on it."
)]
#[case::another_stat(
    either(&stat_is("Power", "Ge", 3)),
    "Whenever a creature you control enters, draw a card if its toughness is 3 or greater. Otherwise, put two +1/+1 counters on it."
)]
#[case::another_relation(
    either(&stat_is("Power", "Ge", 3)),
    "Whenever a creature you control enters, draw a card if its power is 3 or less. Otherwise, put two +1/+1 counters on it."
)]
#[case::branches_swapped(
    either(&stat_is("Power", "Ge", 3)),
    "Whenever a creature you control enters, put two +1/+1 counters on it if its power is 3 or greater. Otherwise, draw a card."
)]
fn t20a_a_choice_on_a_stat_of_the_creature_needs_its_own_words(
    #[case] json: String,
    #[case] clause: &str,
) {
    assert!(!written(&json, clause), "{clause}");
}

/// A card of the given kinds put into a graveyard from the battlefield, with a counter on the source.
fn moved(kinds: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":[{kinds}],"from":"Battlefield","to":"Graveyard"}}}},"effects":[{{"PutCounters":{{"counter":"+1/+1","on":"This"}}}}]}}}}"#
    )
}

#[rstest]
#[case::artifact(
    r#"{"Type":"Artifact"},{"ControlledBy":"You"}"#,
    "Whenever an artifact you control is put into a graveyard from the battlefield, put a +1/+1 counter on ~."
)]
#[case::land(
    r#"{"Type":"Land"},{"ControlledBy":"You"}"#,
    "Whenever a land you control is put into a graveyard from the battlefield, put a +1/+1 counter on ~."
)]
#[case::creature_still_dies(
    r#"{"Type":"Creature"},{"ControlledBy":"You"}"#,
    "Whenever a creature you control dies, put a +1/+1 counter on ~."
)]
fn t20a_a_permanent_that_is_no_creature_is_put_into_a_graveyard(
    #[case] kinds: &str,
    #[case] clause: &str,
) {
    assert!(written(&moved(kinds), clause), "{clause}");
}

#[rstest]
#[case::creature_is_not_put(
    r#"{"Type":"Creature"},{"ControlledBy":"You"}"#,
    "Whenever a creature you control is put into a graveyard from the battlefield, put a +1/+1 counter on ~."
)]
#[case::artifact_dies(
    r#"{"Type":"Artifact"},{"ControlledBy":"You"}"#,
    "Whenever an artifact you control is put into a graveyard from the battlefield, put a +2/+2 counter on ~."
)]
fn t20a_the_longer_wording_needs_a_non_creature_permanent(
    #[case] kinds: &str,
    #[case] clause: &str,
) {
    assert!(!written(&moved(kinds), clause), "{clause}");
}
