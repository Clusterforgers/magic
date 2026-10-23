//! Goal: tokens another player creates ("target opponent creates ...", "its controller creates ..."), tokens that enter tapped ("a tapped Treasure token"), a count of X taken from the cost and "that many" read into `CreateToken` and write back as the same text, alone, beside other sentences and in a trigger; wordings the data cannot hold (an X without an X in the cost, a verb that does not agree with its player, a copy for another player) stay with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::tapped_treasure(
    "Create a tapped Treasure token.",
    false,
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Named":"treasure"},"tapped":true}}]}}"#
)]
#[case::tapped_zombies(
    "Create two tapped 2/2 black Zombie creature tokens.",
    false,
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Zombie","colors":["Black"],"types":["Creature"],"subtypes":["Zombie"],"power":2,"toughness":2}},"count":2,"tapped":true}}]}}"#
)]
#[case::tapped_spirits_with_flying(
    "Create three tapped 1/1 white Spirit creature tokens with flying.",
    false,
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Spirit","colors":["White"],"types":["Creature"],"subtypes":["Spirit"],"power":1,"toughness":1,"abilities":[{"Keyword":"Flying"}]}},"count":3,"tapped":true}}]}}"#
)]
#[case::x_of_the_cost(
    "Create X 1/1 white Soldier creature tokens.",
    true,
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],"power":1,"toughness":1}},"count":"X"}}]}}"#
)]
#[case::x_treasures(
    "Create X Treasure tokens.",
    true,
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Named":"treasure"},"count":"X"}}]}}"#
)]
fn tokens_read_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] x_free: bool,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Spell, x_free, json);
}

#[rstest]
#[case("Create a tapped Clue token.")]
#[case("Create four tapped 1/1 red Goblin creature tokens.")]
#[case("Create a 2/2 white Knight creature token that's tapped and attacking.")]
#[case("Draw a card. Create a tapped Treasure token.")]
#[case("Create a tapped 1/1 white Soldier creature token for each creature you control.")]
#[case("Target player creates a Treasure token.")]
#[case("Destroy target creature. Its controller creates a 3/3 green Beast creature token.")]
#[case("Exile target creature. Its controller creates a 2/2 blue Bird creature token with flying.")]
#[case("Destroy target creature. Its controller creates two tapped Treasure tokens.")]
#[case("Target opponent creates a Treasure token.")]
#[case("Target opponent creates two 1/1 white Spirit creature tokens with flying.")]
#[case("Target opponent creates a Treasure token and you gain 2 life.")]
fn a_token_beside_a_player_or_tapped_writes_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case("Create X 1/1 white Soldier creature tokens.")]
#[case("Create X tapped 2/2 black Zombie creature tokens.")]
#[case("Draw X cards. Create X Treasure tokens.")]
fn a_count_of_x_writes_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, true);
}

#[rstest]
#[case("When ~ enters, target opponent creates two 1/1 white Spirit creature tokens with flying.")]
#[case("Whenever you tap ~ for mana, target opponent creates a 1/1 white Spirit creature token.")]
#[case(
    "Whenever ~ deals combat damage to a player, create that many 1/1 white Soldier creature tokens."
)]
#[case("Whenever ~ deals combat damage to a player, create that many Treasure tokens.")]
#[case("When ~ dies, create two tapped 2/2 black Zombie creature tokens.")]
#[case("Whenever a creature you control dies, create a tapped Treasure token.")]
fn a_token_in_a_trigger_reads_and_writes_back(#[case] text: &str) {
    assert!(trigger(text).is_some(), "{text}");
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::x_without_an_x_cost("Create X 1/1 white Soldier creature tokens.")]
#[case::x_where_without_a_comma(
    "Create X 1/1 white Soldier creature tokens where X is the number of creatures you control."
)]
#[case::tapped_after_the_token("Create a Treasure token tapped.")]
#[case::tapped_with_a_wrong_noun("Create a tapped Treasure tokens.")]
#[case::a_copy_for_another_player(
    "Target opponent creates a token that's a copy of target creature."
)]
#[case::a_verb_that_does_not_agree_with_you("You creates a Treasure token.")]
#[case::a_verb_that_does_not_agree_with_the_player("Target opponent create a Treasure token.")]
fn a_wording_the_data_cannot_hold_stays_unread(#[case] text: &str) {
    assert!(spell(text).is_none(), "{text}");
}
