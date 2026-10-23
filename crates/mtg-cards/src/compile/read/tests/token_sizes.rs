//! Goal: an X/X token whose size is an amount (a characteristic of an object, a count, a spell's mana value) reads into `TokenRef::Sized` and writes back as the same text with its where clause, alone and in a trigger.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::its_power(
    "When this creature dies, create an X/X black Horror creature token, where X is this creature's power."
)]
#[case::lands_in_a_graveyard(
    "When this creature dies, create an X/X colorless Shapeshifter creature token with changeling and deathtouch, where X is the number of land cards in your graveyard."
)]
#[case::creatures_you_control(
    "When this creature enters, create an X/X green Beast creature token, where X is the number of creatures you control."
)]
fn a_token_sized_by_an_amount_writes_back_in_a_trigger(#[case] text: &str) {
    assert!(trigger(text).is_some(), "{text}");
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::a_count(
    "Create an X/X colorless Shapeshifter creature token with changeling and deathtouch, where X is the number of land cards in your graveyard."
)]
#[case::creatures_that_died(
    "Create an X/X black Horror creature token, where X is the number of creatures that died this turn."
)]
fn a_token_sized_by_a_count_writes_back_as_a_spell(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case::life_total(
    "Create X 1/1 white Pegasus creature tokens with flying, where X is your life total."
)]
#[case::one_for_each_that_died(
    "Create a 3/3 green Beast creature token for each creature put into your graveyard from the battlefield this turn."
)]
#[case::a_static_in_quotes(
    "Create two 1/1 black Rat creature tokens with \"This token can't block.\""
)]
#[case::they_have_a_quoted_ability(
    "Create three 1/1 colorless Eldrazi Scion creature tokens. They have \"Sacrifice this token: Add {C}.\""
)]
#[case::it_has_a_quoted_ability(
    "Create a 1/1 colorless Triskelavite artifact creature token with flying. It has \"Sacrifice this token: This token deals 1 damage to any target.\""
)]
fn counts_and_abilities_of_tokens_write_back_as_a_spell(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case::its_power(
    "When this creature dies, create a number of tapped Treasure tokens equal to its power."
)]
#[case::counters_on_it(
    "When this creature dies, create X Junk tokens, where X is the number of +1/+1 counters on it."
)]
#[case::a_spells_mana_value(
    "Whenever you cast a noncreature spell, create an X/X blue Shark creature token with flying, where X is that spell's mana value."
)]
#[case::nontoken_creatures_that_died(
    "When this creature enters, create a 2/2 green Bear creature token for each nontoken creature put into your graveyard from the battlefield this turn."
)]
#[case::they_have_after_a_count(
    "Whenever this creature deals combat damage to a player, create that many 0/1 colorless Eldrazi Spawn creature tokens. They have \"Sacrifice this token: Add {C}.\""
)]
fn counts_of_tokens_write_back_in_a_trigger(#[case] text: &str) {
    golden(text, ClauseKind::Triggered, false);
}

#[test]
fn the_size_is_the_amount_of_the_where_clause() {
    reads_as(
        "Create an X/X colorless Shapeshifter creature token with deathtouch, where X is the number of land cards in your graveyard.",
        ClauseKind::Spell,
        false,
        r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Sized":{"size":{"Count":[{"Type":"Land"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"token":{"name":"Shapeshifter","types":["Creature"],"subtypes":["Shapeshifter"],"abilities":[{"Keyword":"Deathtouch"}]}}}}}]}}"#,
    );
}

#[rstest]
#[case::no_where_clause_and_no_x_in_the_cost("Create an X/X green Beast creature token.")]
#[case::a_size_that_is_a_number_and_x(
    "Create an X/3 green Beast creature token, where X is the number of creatures you control."
)]
fn a_size_with_nothing_to_name_it_stays_unread(#[case] text: &str) {
    let read = spell(text);
    assert!(
        read.as_ref().is_none_or(|ability| render_variants(ability)
            .iter()
            .all(|written| !same_text(written, text))),
        "{text}: {read:?}"
    );
}
