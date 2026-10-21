//! Goal: an X of the card's own cost is read where it outlives the spell (what a spell may target or destroy, the counters the card enters with, what its enters trigger does, the mana value of a library search) and written back in the same words, a card without an X in its cost leaves each of them to the table, and "It deals" opens an activated ability that damages for the source.
use super::*;

fn activated(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Activated, false)
}

#[rstest]
#[case::exactly_x(
    "Destroy target creature with mana value X.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"Compare":{"stat":"ManaValue","op":"Eq","to":"X"}}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#
)]
#[case::x_or_less(
    "Destroy target creature with mana value X or less.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"Compare":{"stat":"ManaValue","op":"Le","to":"X"}}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#
)]
#[case::power_x_or_less(
    "Destroy target creature with power X or less.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Le","to":"X"}}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#
)]
#[case::all_creatures(
    "Destroy all creatures with mana value X or less.",
    r#"{"Spell":{"effects":[{"Destroy":{"what":{"Each":[{"Type":"Creature"},{"Compare":{"stat":"ManaValue","op":"Le","to":"X"}}]}}}]}}"#
)]
fn an_x_in_what_a_spell_selects_is_read_and_written_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, true, json);
    assert_eq!(
        spell(text),
        None,
        "a card without an X in its cost has none to read"
    );
}

#[rstest]
#[case::a_creature(
    "Search your library for a creature card with mana value X or less, put it onto the battlefield, then shuffle.",
    r#"{"Spell":{"effects":[{"Search":{"filter":[{"Type":"Creature"},{"Compare":{"stat":"ManaValue","op":"Le","to":"X"}}],"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":{"Battlefield":{}}}},{"Shuffle":{}}]}}"#
)]
#[case::a_permanent(
    "Search your library for a permanent card with mana value X or less, put it onto the battlefield, then shuffle.",
    r#"{"Spell":{"effects":[{"Search":{"filter":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"},{"Type":"Battle"}]},{"Compare":{"stat":"ManaValue","op":"Le","to":"X"}}],"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":{"Battlefield":{}}}},{"Shuffle":{}}]}}"#
)]
#[case::into_your_hand(
    "Search your library for an artifact card with mana value X or less, reveal it, put it into your hand, then shuffle.",
    r#"{"Spell":{"effects":[{"Search":{"filter":[{"Type":"Artifact"},{"Compare":{"stat":"ManaValue","op":"Le","to":"X"}}],"reveal":true,"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":"Hand"}},{"Shuffle":{}}]}}"#
)]
fn the_mana_value_of_a_library_search_reads_the_x_of_the_cost(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Spell, true, json);
    assert_eq!(
        spell(text),
        None,
        "a card without an X in its cost has none to read"
    );
}

#[test]
fn counters_the_card_enters_with_and_what_its_enters_trigger_draws_read_the_x_of_the_cost() {
    reads_as(
        "This creature enters with X +1/+1 counters on it.",
        ClauseKind::Static,
        true,
        r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":"+1/+1","count":"X"}}}}"#,
    );
    reads_as(
        "When ~ enters, draw X cards.",
        ClauseKind::Triggered,
        true,
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Draw":{"count":"X"}}]}}"#,
    );
    assert_eq!(
        clause("When ~ enters, draw X cards.", ClauseKind::Triggered, false),
        None
    );
}

#[test]
fn it_deals_damage_for_the_source_only_as_the_first_thing_an_ability_does() {
    reads_as(
        "Remove a +1/+1 counter from this creature: It deals 1 damage to any target.",
        ClauseKind::Activated,
        false,
        r#"{"Activated":{"cost":[{"RemoveCounters":{"counter":"+1/+1"}}],"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}}]}}"#,
    );
    assert_eq!(
        activated("{T}: Destroy target creature. It deals 2 damage to you."),
        None,
        "after a target, \"it\" is that creature"
    );
}
