//! Goal: "create ... tokens" reads into `CreateToken` for the predefined Treasure, Clue and Food tokens and for described creature tokens, with their count, colors, subtypes, "tapped and attacking" and the count of "where X is the number of", and writes back as the card's text; any other token wording stays with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::hero_of_bladehold(
    "Whenever ~ attacks, create two 1/1 white Soldier creature tokens that are tapped and attacking.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],"power":1,"toughness":1}},"count":2,"tapped":true,"attacking":true}}]}}"#
)]
#[case::dockside_extortionist(
    "When ~ enters, create X Treasure tokens, where X is the number of artifacts and enchantments your opponents control.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"CreateToken":{"token":{"Named":"treasure"},"count":{"Count":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]},{"ControlledBy":"Opponents"}]}}}]}}"#
)]
#[case::landfall_investigate(
    "Whenever a land you control enters, investigate.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Land"},{"ControlledBy":"You"}],"to":"Battlefield"}},"effects":[{"CreateToken":{"token":{"Named":"clue"}}}]}}"#
)]
#[case::a_treasure(
    "When ~ dies, create a Treasure token.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"CreateToken":{"token":{"Named":"treasure"}}}]}}"#
)]
#[case::a_clue_by_its_name(
    "When ~ dies, create a Clue token.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"CreateToken":{"token":{"Named":"clue"}}}]}}"#
)]
#[case::a_food(
    "At the beginning of your end step, create a Food token.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"CreateToken":{"token":{"Named":"food"}}}]}}"#
)]
#[case::a_creature_token_attacking(
    "Whenever ~ attacks, create a 1/1 white Soldier creature token that's tapped and attacking.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],"power":1,"toughness":1}},"tapped":true,"attacking":true}}]}}"#
)]
fn a_trigger_that_creates_tokens_reads_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::colorless_artifact(
    "Create a 1/1 colorless Servo artifact creature token.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Servo","types":["Artifact","Creature"],"subtypes":["Servo"],"power":1,"toughness":1}}}}]}}"#
)]
#[case::two_colors(
    "Create a 1/1 green and white Elf Warrior creature token.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Elf Warrior","colors":["Green","White"],"types":["Creature"],"subtypes":["Elf","Warrior"],"power":1,"toughness":1}}}}]}}"#
)]
#[case::three_treasures(
    "Create three Treasure tokens.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Named":"treasure"},"count":3}}]}}"#
)]
fn a_spell_that_creates_tokens_reads_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case("Create a Treasure token.")]
#[case("Create a Food token.")]
#[case("Create a Clue token.")]
#[case("Investigate.")]
#[case("Create two Treasure tokens.")]
#[case("Create two Clue tokens.")]
#[case("Create three Food tokens.")]
#[case("Create a 1/1 white Soldier creature token.")]
#[case("Create three 1/1 white Soldier creature tokens.")]
#[case("Create a 2/2 black Zombie creature token.")]
#[case("Create two 2/2 black Zombie creature tokens.")]
#[case("Create a 0/1 white Plant creature token.")]
#[case("Create an 8/8 blue Octopus creature token.")]
#[case("Create an 11/11 green Hydra creature token.")]
#[case("Create a 3/3 colorless Golem artifact creature token.")]
#[case("Create a 1/1 red Goblin creature token.")]
#[case("Create four 1/1 green Saproling creature tokens.")]
#[case("Create a 1/1 white and blue Elf Warrior creature token.")]
#[case("Create a 4/4 white Angel creature token that's tapped and attacking.")]
#[case("Create two 1/1 red Goblin creature tokens that are tapped and attacking.")]
#[case("Create X Treasure tokens, where X is the number of creatures you control.")]
#[case("Create X Food tokens, where X is the number of artifacts you control.")]
#[case(
    "Create X 1/1 white Soldier creature tokens, where X is the number of creatures your opponents control."
)]
#[case("Draw a card. Create a Treasure token.")]
#[case("Create a Treasure token. You gain 2 life.")]
#[case("You may create a Treasure token.")]
#[case("You may create a 1/1 white Soldier creature token.")]
fn more_oracle_wordings_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case("When ~ enters, create a Treasure token.")]
#[case("When ~ enters, create two Treasure tokens.")]
#[case("When ~ enters, investigate.")]
#[case("When ~ enters, create a 1/1 white Soldier creature token.")]
#[case("Whenever ~ attacks, create a Treasure token.")]
#[case("Whenever you cast a spell, create a 1/1 white Soldier creature token.")]
#[case("Whenever a creature you control dies, you may create a Treasure token.")]
#[case("Whenever you sacrifice a Treasure, create a Food token.")]
#[case("At the beginning of your upkeep, create a 1/1 red Goblin creature token.")]
#[case("Whenever ~ enters or attacks, create a 1/1 white Soldier creature token.")]
#[case(
    "Constellation \u{2014} Whenever an enchantment you control enters, create a Treasure token."
)]
fn more_oracle_wordings_in_a_trigger_read_and_write_back(#[case] text: &str) {
    let rest = text.split_once(" \u{2014} ").map_or(text, |(_, rest)| rest);
    assert_eq!(
        trigger(text),
        Some(golden(rest, ClauseKind::Triggered, false)),
        "{text}"
    );
}

#[rstest]
#[case::tapped_first("Create a tapped Treasure token.")]
#[case::with_an_ability_the_dsl_lacks("Create a 1/1 white Spirit creature token with decayed.")]
#[case::tapped_only("Create a 1/1 white Soldier creature token that's tapped.")]
#[case::wrong_agreement_attacking(
    "Create a 1/1 white Soldier creature token that are tapped and attacking."
)]
#[case::a_name_that_is_not_predefined("Create a Goblin token.")]
#[case::no_color("Create a 1/1 Soldier creature token.")]
#[case::no_subtype("Create a 1/1 white creature token.")]
#[case::no_type("Create a 1/1 white Soldier token.")]
#[case::a_legendary_token("Create a legendary 1/1 white Soldier creature token.")]
#[case::for_each("Create a Treasure token for each artifact you control.")]
#[case::x_without_a_definition("Create X Treasure tokens.")]
#[case::a_count_and_a_definition(
    "Create two Treasure tokens, where X is the number of creatures you control."
)]
#[case::digits("Create 2 Treasure tokens.")]
#[case::agreement_of_the_noun("Create two Treasure token.")]
#[case::agreement_of_a_single_noun("Create a Treasure tokens.")]
#[case::a_wrong_article("Create an Treasure token.")]
#[case::lowercase_names("Create a treasure token.")]
#[case::a_chain("Create a Treasure token, then draw a card.")]
#[case::a_pronoun("Create a Treasure token. It enters tapped.")]
#[case::under_another_control("Target player creates a Treasure token.")]
#[case::a_number_that_is_not_a_size("Create a 1 white Soldier creature token.")]
#[case::an_open_count("Create that many Treasure tokens.")]
#[case::investigate_twice("Investigate twice.")]
#[case::investigate_for_each("Investigate for each creature you control.")]
#[case::a_defined_name("Create a Treasure token named Gold.")]
#[case::a_named_x("Create X Treasure tokens, where X is your life total.")]
#[case::a_plural_where("Create X Treasure tokens, where X is the number of creature you control.")]
fn near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[test]
fn investigate_and_a_clue_token_are_the_same_text() {
    assert_eq!(
        spell("Investigate."),
        spell("Create a Clue token."),
        "the keyword action is its reminder text"
    );
    assert!(spell("Investigate.").is_some());
    assert_eq!(
        trigger("When ~ dies, investigate."),
        trigger("When ~ dies, create a Clue token.")
    );
}
