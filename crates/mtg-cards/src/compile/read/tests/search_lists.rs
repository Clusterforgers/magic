//! Goal: a search for a card whose land types or card types are a comma list ("a basic Forest, Plains, or Island card", "a Plains, Island, Swamp, or Mountain card") reads into one `AnyOf` filter and writes back as the same text, in a spell and in an activation; a list without "or", with the comma in a list of two, without the comma in a longer list, or ending in something that is not a type stays with the table.
use super::*;

fn activated(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Activated, false)
}

#[rstest]
#[case::farseek(
    "Search your library for a Plains, Island, Swamp, or Mountain card, put it onto the battlefield tapped, then shuffle.",
    r#"{"Spell":{"effects":[{"Search":{"filter":[{"AnyOf":[{"Subtype":"Plains"},{"Subtype":"Island"},{"Subtype":"Swamp"},{"Subtype":"Mountain"}]}],"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}}]}}"#
)]
#[case::three_basic_types(
    "Search your library for a basic Forest, Plains, or Island card, put it onto the battlefield tapped, then shuffle.",
    r#"{"Spell":{"effects":[{"Search":{"filter":[{"Supertype":"Basic"},{"AnyOf":[{"Subtype":"Forest"},{"Subtype":"Plains"},{"Subtype":"Island"}]}],"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}}]}}"#
)]
#[case::three_card_types(
    "Search your library for an artifact, creature, or land card, reveal it, put it into your hand, then shuffle.",
    r#"{"Spell":{"effects":[{"Search":{"filter":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"},{"Type":"Land"}]}],"reveal":true,"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":"Hand"}},{"Shuffle":{}}]}}"#
)]
fn a_comma_list_of_types_reads_into_one_filter(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case(
    "Search your library for a Plains or Island card, put it onto the battlefield, then shuffle."
)]
#[case(
    "Search your library for a Forest, Plains, or Island card, put it onto the battlefield, then shuffle."
)]
#[case(
    "Search your library for a basic Swamp, Mountain, or Forest card, reveal it, put it into your hand, then shuffle."
)]
#[case(
    "Search your library for up to two Plains, Island, or Swamp cards, put them onto the battlefield tapped, then shuffle."
)]
#[case(
    "You may search your library for a Plains, Island, or Swamp card, put it onto the battlefield, then shuffle."
)]
#[case(
    "Search your library for a legendary Dragon, Wizard, or Elf creature card, reveal it, put it into your hand, then shuffle."
)]
fn a_list_of_types_writes_back_as_the_text(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case(
    "{1}, {T}, Sacrifice this land: Search your library for a basic Forest, Plains, or Island card, put it onto the battlefield tapped, then shuffle."
)]
#[case(
    "{T}, Sacrifice this land: Search your library for a basic Island, Mountain, or Plains card, put it onto the battlefield tapped, then shuffle."
)]
fn a_list_of_basic_land_types_in_an_activation_reads_and_writes_back(#[case] text: &str) {
    assert!(activated(text).is_some(), "{text}");
    golden(text, ClauseKind::Activated, false);
}

#[rstest]
#[case::no_or(
    "Search your library for a Plains, Island, Swamp card, put it onto the battlefield, then shuffle."
)]
#[case::comma_before_or_in_a_pair(
    "Search your library for a Plains, or Island card, put it onto the battlefield, then shuffle."
)]
#[case::no_comma_in_a_long_list(
    "Search your library for a Plains, Island or Swamp card, put it onto the battlefield, then shuffle."
)]
#[case::ends_in_a_word_that_is_no_type(
    "Search your library for a Plains, Island, or Wolfgang card, put it onto the battlefield, then shuffle."
)]
#[case::a_dangling_comma(
    "Search your library for a Plains, Island, card, put it onto the battlefield, then shuffle."
)]
fn a_list_that_is_not_worded_that_way_stays_unread(#[case] text: &str) {
    assert!(spell(text).is_none(), "{text}");
}
