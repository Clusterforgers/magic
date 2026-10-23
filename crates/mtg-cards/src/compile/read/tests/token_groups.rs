//! Goal: the tokens a sentence creates beyond one plain kind (a list of kinds, a token beside a draw or a mill, a legendary or a named token, an activated or triggered ability in quotes) read into `CreateToken` and write back as the same text, alone and in a trigger; wordings the data cannot hold stay with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::three_kinds(
    "Create a 1/1 green Snake creature token, a 2/2 green Wolf creature token, and a 3/3 green Elephant creature token.",
    r#"{"Spell":{"effects":[
        {"CreateToken":{"token":{"Custom":{"name":"Snake","colors":["Green"],"types":["Creature"],"subtypes":["Snake"],"power":1,"toughness":1}}}},
        {"CreateToken":{"token":{"Custom":{"name":"Wolf","colors":["Green"],"types":["Creature"],"subtypes":["Wolf"],"power":2,"toughness":2}}}},
        {"CreateToken":{"token":{"Custom":{"name":"Elephant","colors":["Green"],"types":["Creature"],"subtypes":["Elephant"],"power":3,"toughness":3}}}}]}}"#
)]
#[case::a_creature_and_a_food(
    "Create a 1/1 white Halfling creature token and a Food token.",
    r#"{"Spell":{"effects":[
        {"CreateToken":{"token":{"Custom":{"name":"Halfling","colors":["White"],"types":["Creature"],"subtypes":["Halfling"],"power":1,"toughness":1}}}},
        {"CreateToken":{"token":{"Named":"food"}}}]}}"#
)]
#[case::named(
    "Create a 3/1 red Beast creature token named Carnivore.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Carnivore","colors":["Red"],"types":["Creature"],"subtypes":["Beast"],"power":3,"toughness":1}}}}]}}"#
)]
#[case::named_after_a_keyword(
    "Create a 1/2 blue Bird creature token with flying named Storm Crow.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Storm Crow","colors":["Blue"],"types":["Creature"],"subtypes":["Bird"],"power":1,"toughness":2,"abilities":[{"Keyword":"Flying"}]}}}}]}}"#
)]
#[case::legendary(
    "Create Tiny, a legendary 2/2 green Dog Detective creature token with trample.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Tiny","colors":["Green"],"supertypes":["Legendary"],"types":["Creature"],"subtypes":["Dog","Detective"],"power":2,"toughness":2,"abilities":[{"Keyword":"Trample"}]}}}}]}}"#
)]
#[case::vibranium(
    "Create a tapped Vibranium token.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Named":"vibranium"},"tapped":true}}]}}"#
)]
#[case::map(
    "Create a Map token.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Named":"map"}}}]}}"#
)]
#[case::mutagen(
    "Create a Mutagen token.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Named":"mutagen"}}}]}}"#
)]
fn tokens_read_in_the_shape_of_the_hand_written_files(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case("Draw two cards and create a Treasure token.")]
#[case("Draw a card and create three Treasure tokens.")]
#[case("Draw a card, then create a Food token.")]
#[case("Create three 1/1 green Elf Warrior creature tokens, then mill three cards.")]
#[case("Create a Food token, then scry 2.")]
#[case("Create two Treasure tokens and a Clue token.")]
#[case("Create a 2/2 black Zombie creature token, a Treasure token, and a Food token.")]
fn tokens_beside_other_steps_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case(
    "Create a 0/1 colorless Eldrazi Spawn creature token with \"Sacrifice this token: Add {C}.\""
)]
#[case(
    "Create a 1/1 black and green Pest creature token with \"When this token dies, you gain 1 life.\""
)]
#[case(
    "Create a 3/3 white Bird creature token with flying and \"Whenever this token attacks, you gain 1 life.\""
)]
fn a_token_with_an_ability_in_quotes_writes_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case("When this creature enters, create a Map token.")]
#[case("Whenever this creature attacks, create a tapped Vibranium token.")]
#[case(
    "When this creature dies, create Tuktuk the Returned, a legendary 5/5 colorless Goblin Golem artifact creature token."
)]
#[case(
    "When this creature enters, create a 1/1 white Soldier creature token and a Treasure token."
)]
#[case(
    "When this creature enters, create two 1/1 blue Fish creature tokens and create two tapped Treasure tokens."
)]
#[case(
    "When this creature dies, create a 0/1 colorless Eldrazi Spawn creature token with \"Sacrifice this token: Add {C}.\""
)]
#[case(
    "Whenever a nontoken creature you control dies, create a 1/1 black Pest creature token with \"When this token dies, you gain 1 life.\""
)]
fn the_new_token_wordings_read_in_a_trigger(#[case] text: &str) {
    assert!(trigger(text).is_some(), "{text}");
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::no_second_kind("Create a 1/1 green Snake creature token, and.")]
#[case::a_name_without_the_legend("Create Tiny, a 2/2 green Dog Detective creature token.")]
#[case::a_name_in_lower_case("Create a 3/1 red Beast creature token named carnivore.")]
#[case::a_kind_that_is_not_a_token("Create a 1/1 green Snake creature token and draw a card and.")]
#[case::steps_with_no_token("Draw two cards and scry 2.")]
fn wordings_the_data_cannot_hold_stay_unread(#[case] text: &str) {
    let read = spell(text);
    assert!(
        read.as_ref().is_none_or(|ability| render_variants(ability)
            .iter()
            .all(|written| !same_text(written, text))),
        "{text}: {read:?}"
    );
}
