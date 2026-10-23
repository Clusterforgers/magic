//! Goal: a described creature token "with flying" (one keyword, two joined by "and", three or more in a list) and a token that "enters attacking" without being tapped read into `CreateToken` with the keywords as the token's abilities, alone, in a trigger, in an activation and in a mode, and write back the same text; quoted abilities, keywords with parameters, a predefined token with a keyword, and other orders and agreements stay with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

fn activated(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Activated, false)
}

#[rstest]
#[case::flying(
    "Create a 1/1 white Spirit creature token with flying.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Spirit","colors":["White"],"types":["Creature"],"subtypes":["Spirit"],"power":1,"toughness":1,"abilities":[{"Keyword":"Flying"}]}}}}]}}"#
)]
#[case::three_with_flying(
    "Create three 1/1 white Spirit creature tokens with flying.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Spirit","colors":["White"],"types":["Creature"],"subtypes":["Spirit"],"power":1,"toughness":1,"abilities":[{"Keyword":"Flying"}]}},"count":3}}]}}"#
)]
#[case::an_artifact_creature(
    "Create two 1/1 colorless Thopter artifact creature tokens with flying.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Thopter","types":["Artifact","Creature"],"subtypes":["Thopter"],"power":1,"toughness":1,"abilities":[{"Keyword":"Flying"}]}},"count":2}}]}}"#
)]
#[case::two_keywords(
    "Create a 4/4 white Angel creature token with flying and vigilance.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Angel","colors":["White"],"types":["Creature"],"subtypes":["Angel"],"power":4,"toughness":4,"abilities":[{"Keyword":"Flying"},{"Keyword":"Vigilance"}]}}}}]}}"#
)]
#[case::three_keywords(
    "Create a 3/1 black and red Vampire creature token with trample, lifelink, and haste.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Vampire","colors":["Black","Red"],"types":["Creature"],"subtypes":["Vampire"],"power":3,"toughness":1,"abilities":[{"Keyword":"Trample"},{"Keyword":"Lifelink"},{"Keyword":"Haste"}]}}}}]}}"#
)]
#[case::two_word_keyword(
    "Create a 2/2 black Zombie creature token with first strike.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Zombie","colors":["Black"],"types":["Creature"],"subtypes":["Zombie"],"power":2,"toughness":2,"abilities":[{"Keyword":"FirstStrike"}]}}}}]}}"#
)]
#[case::counted_by_x(
    "Create X 1/1 white Soldier creature tokens with lifelink, where X is the number of creatures you control.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],"power":1,"toughness":1,"abilities":[{"Keyword":"Lifelink"}]}},"count":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}]}}"#
)]
#[case::attacking_without_being_tapped(
    "Create a 1/1 white Cat Soldier creature token with vigilance that's attacking.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Cat Soldier","colors":["White"],"types":["Creature"],"subtypes":["Cat","Soldier"],"power":1,"toughness":1,"abilities":[{"Keyword":"Vigilance"}]}},"attacking":true}}]}}"#
)]
#[case::tapped_and_attacking(
    "Create a 4/4 white Angel creature token with flying that's tapped and attacking.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Angel","colors":["White"],"types":["Creature"],"subtypes":["Angel"],"power":4,"toughness":4,"abilities":[{"Keyword":"Flying"}]}},"tapped":true,"attacking":true}}]}}"#
)]
fn a_token_with_keywords_reads_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::flying("flying")]
#[case::reach("reach")]
#[case::trample("trample")]
#[case::vigilance("vigilance")]
#[case::lifelink("lifelink")]
#[case::haste("haste")]
#[case::menace("menace")]
#[case::deathtouch("deathtouch")]
#[case::double_strike("double strike")]
#[case::indestructible("indestructible")]
#[case::hexproof("hexproof")]
#[case::defender("defender")]
#[case::prowess("prowess")]
#[case::changeling("changeling")]
#[case::infect("infect")]
#[case::shroud("shroud")]
#[case::two_joined("flying and deathtouch")]
#[case::three_listed("flying, vigilance, and indestructible")]
#[case::four_listed("flying, trample, haste, and lifelink")]
fn every_plain_keyword_and_a_list_of_them_read_and_write_back(#[case] keywords: &str) {
    for text in [
        format!("Create a 2/2 green Beast creature token with {keywords}."),
        format!("Create two 1/3 colorless Golem artifact creature tokens with {keywords}."),
        format!(
            "Create X 1/1 blue Bird creature tokens with {keywords}, where X is the number of Birds you control."
        ),
    ] {
        golden(&text, ClauseKind::Spell, false);
    }
}

#[rstest]
#[case("Create a 1/1 white Spirit creature token with flying. Draw a card.")]
#[case("Draw a card. Create a 1/1 white Spirit creature token with flying.")]
#[case("Create two 1/1 white Spirit creature tokens with flying. You gain 2 life.")]
#[case("Destroy all creatures. Create three 1/1 black Faerie Rogue creature tokens with flying.")]
#[case("Counter target spell. Create a 1/1 colorless Thopter artifact creature token with flying.")]
#[case("You may create a 1/1 white Spirit creature token with flying.")]
#[case(
    "Create a 1/1 white Cat Soldier creature token with vigilance that's attacking. You gain 1 life."
)]
#[case("Create two 4/4 white Angel creature tokens with flying and vigilance that are attacking.")]
fn a_token_with_keywords_beside_other_sentences_reads_and_writes_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case("When ~ enters, create a 1/1 white Spirit creature token with flying.")]
#[case("When ~ dies, create a 3/3 white Bird creature token with flying.")]
#[case(
    "Whenever you cast a creature spell, create a 1/1 white Warrior creature token with vigilance."
)]
#[case("Whenever ~ attacks, create a 4/4 white Angel creature token with flying.")]
#[case(
    "Whenever ~ attacks, create a 1/1 white Cat Soldier creature token with vigilance that's attacking."
)]
#[case(
    "Whenever ~ attacks, create two 4/4 white Angel creature tokens with flying and vigilance that are attacking."
)]
#[case(
    "At the beginning of your upkeep, you lose 1 life and create a 1/1 black Faerie Rogue creature token with flying."
)]
#[case(
    "Whenever a land you control enters, you may create a 1/1 white Bird creature token with flying."
)]
#[case(
    "When ~ enters, create X 1/1 white Spirit creature tokens with flying, where X is the number of creatures you control."
)]
#[case("When ~ enters, create two 2/2 red Human Knight creature tokens with trample and haste.")]
fn a_token_with_keywords_in_a_trigger_reads_and_writes_back(#[case] text: &str) {
    assert!(trigger(text).is_some(), "{text}");
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case("{3}{U}: Create a 1/1 blue Faerie creature token with flying.")]
#[case(
    "{5}, {T}, Sacrifice ~: Create two 1/1 colorless Thopter artifact creature tokens with flying."
)]
#[case("{T}: Create a 2/2 black Vampire creature token with flying.")]
#[case("{1}, Sacrifice a creature: Create a 2/2 white Knight creature token with vigilance.")]
fn a_token_with_keywords_in_an_activation_reads_and_writes_back(#[case] text: &str) {
    assert!(activated(text).is_some(), "{text}");
    golden(text, ClauseKind::Activated, false);
}

#[test]
fn a_token_with_keywords_in_a_mode_reads_beside_the_other_modes() {
    let text = "Choose one \u{2014}\n\u{2022} Create a 2/2 white Knight creature token with vigilance.\n\u{2022} Draw a card.";
    golden(text, ClauseKind::Modal, false);
}

#[rstest]
#[case::gains_defender("Target creature gains defender until end of turn.")]
#[case::gets_and_gains_prowess("Target creature gets +1/+1 and gains prowess until end of turn.")]
#[case::gains_two_new_keywords("Creatures you control gain shroud and infect until end of turn.")]
#[case::destroy_a_creature_with_infect("Destroy target creature with infect.")]
#[case::exile_a_creature_with_defender("Exile target creature with defender.")]
fn the_keywords_of_the_vocabulary_also_read_in_a_pump_and_a_selector(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case::a_quoted_ability(
    r#"Create a 1/1 colorless Pilot creature token with "This token can't block.""#
)]
#[case::a_keyword_and_a_quoted_ability(
    r#"Create a 6/6 black and red Dragon creature token with flying, menace, and "Whenever this token deals combat damage to a player, draw a card.""#
)]
#[case::a_keyword_with_a_number(
    "Create a 1/1 colorless Phyrexian Mite artifact creature token with toxic 1."
)]
#[case::a_landwalk("Create a 1/1 blue Squid creature token with islandwalk.")]
#[case::a_keyword_the_dsl_lacks("Create a 2/2 black Zombie creature token with decayed.")]
#[case::a_keyword_the_vocabulary_lacks("Create a 1/1 white Soldier creature token with flanking.")]
#[case::protection("Create a 1/1 white Knight creature token with protection from red.")]
#[case::a_predefined_token("Create a Treasure token with flying.")]
#[case::a_clue_with_a_keyword("Create a Clue token with haste.")]
#[case::without_the_list_joined("Create a 1/1 white Spirit creature token with flying and.")]
#[case::an_empty_list("Create a 1/1 white Spirit creature token with.")]
#[case::a_dangling_comma("Create a 1/1 white Spirit creature token with flying,.")]
#[case::no_comma_before_the_last_of_three(
    "Create a 1/1 white Spirit creature token with flying, vigilance and haste."
)]
#[case::and_after_the_comma_of_two(
    "Create a 1/1 white Spirit creature token with flying, and haste."
)]
#[case::keyword_before_the_noun("Create a 1/1 white Spirit creature with flying token.")]
#[case::has("Create a 1/1 white Spirit creature token that has flying.")]
#[case::keyword_after_the_entering(
    "Create a 1/1 white Spirit creature token that's attacking with flying."
)]
#[case::capitalized("Create a 1/1 white Spirit creature token with Flying.")]
#[case::tapped_only("Create a 1/1 white Spirit creature token with flying that's tapped.")]
#[case::wrong_agreement_attacking(
    "Create two 1/1 white Spirit creature tokens with flying that's attacking."
)]
#[case::wrong_agreement_of_a_single(
    "Create a 1/1 white Spirit creature token with flying that are attacking."
)]
#[case::attacking_after_an_open_count(
    "Create X 1/1 white Spirit creature tokens with flying that are attacking."
)]
#[case::an_open_count_named_otherwise(
    "Create X 1/1 white Pegasus creature tokens with flying, where X is your life total."
)]
#[case::a_second_token_that_is_not_a_kind(
    "Create a 1/1 white Spirit creature token with flying and a 2/2 white Knight."
)]
#[case::a_following_sentence_that_is_not_read(
    "Create a 4/4 white Angel creature token with flying. Exile that token at end of combat."
)]
#[case::no_type("Create a 1/1 white Spirit token with flying.")]
#[case::a_legendary_token("Create a legendary 1/1 white Spirit creature token with flying.")]
#[case::a_named_token("Create a 1/1 white Spirit creature token named Wisp with flying.")]
fn near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
