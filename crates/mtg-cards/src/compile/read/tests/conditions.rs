//! Goal: a static ability under "as long as", a list of protections, the trigger effect "you create a token and that player mills ten cards", a plural subtype and a card that returns itself from the graveyard read into the shapes of the hand-written files, write back as the same text, and leave every near-miss to the table.
use super::*;

fn stat(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Static, false)
}

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

const OPPONENT_AT_TEN: &str =
    r#"{"Compare":{"left":{"LowestLife":"Opponents"},"op":"Le","right":10}}"#;

#[rstest]
#[case::bloodghast(
    "This creature has haste as long as an opponent has 10 or less life.",
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Haste"}],"condition":CONDITION}}"#,
    OPPONENT_AT_TEN
)]
#[case::the_card_name(
    "~ has haste as long as an opponent has 10 or less life.",
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Haste"}],"condition":CONDITION}}"#,
    OPPONENT_AT_TEN
)]
#[case::each_opponent(
    "This creature has flying as long as each opponent has 20 or more life.",
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Flying"}],"condition":CONDITION}}"#,
    r#"{"Compare":{"left":{"LowestLife":"Opponents"},"op":"Ge","right":20}}"#
)]
#[case::a_player(
    "This creature has trample as long as a player has 5 or less life.",
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Trample"}],"condition":CONDITION}}"#,
    r#"{"Compare":{"left":{"LowestLife":"EachPlayer"},"op":"Le","right":5}}"#
)]
#[case::the_condition_first(
    "As long as it's your turn, this creature has haste.",
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Haste"}],"condition":CONDITION}}"#,
    r#""YourTurn""#
)]
#[case::a_bonus_for_itself(
    "This creature gets +2/+2 as long as it's your turn.",
    r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":2,"toughness":2}}],"condition":CONDITION}}"#,
    r#""YourTurn""#
)]
#[case::a_bonus_and_a_keyword_for_itself(
    "This creature gets +1/+1 and has flying as long as you control a Plains.",
    r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":1,"toughness":1}},{"AddKeyword":"Flying"}],"condition":CONDITION}}"#,
    r#"{"Compare":{"left":{"Count":[{"Subtype":"Plains"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}"#
)]
#[case::an_equipment(
    "Equipped creature gets +1/+1 as long as you control a Plains.",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":1,"toughness":1}}],"condition":CONDITION}}"#,
    r#"{"Compare":{"left":{"Count":[{"Subtype":"Plains"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}"#
)]
#[case::an_aura_on_your_turn(
    "Enchanted creature has first strike as long as it's your turn.",
    r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":"FirstStrike"}],"condition":CONDITION}}"#,
    r#""YourTurn""#
)]
#[case::a_group(
    "Creatures you control have flying as long as it's your turn.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Flying"}],"condition":CONDITION}}"#,
    r#""YourTurn""#
)]
#[case::controlling_a_kind(
    "This creature has lifelink as long as you control a creature with flying.",
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Lifelink"}],"condition":CONDITION}}"#,
    r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"HasKeyword":"Flying"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}"#
)]
fn a_static_ability_under_a_condition_reads_as_the_hand_written_files_say(
    #[case] text: &str,
    #[case] json: &str,
    #[case] condition: &str,
) {
    reads_as(
        text,
        ClauseKind::Static,
        false,
        &json.replace("CONDITION", condition),
    );
}

#[rstest]
#[case::itself_without_a_condition("This creature has haste.")]
#[case::a_bonus_for_itself_without_a_condition("This creature gets +2/+2.")]
#[case::your_own_exact_life("This creature has haste as long as you have 10 life.")]
#[case::an_exact_life_total("This creature has haste as long as an opponent has 10 life.")]
#[case::more_than_a_life_total(
    "This creature has haste as long as an opponent has more than 10 life."
)]
#[case::another_turn("This creature has haste as long as it's an opponent's turn.")]
#[case::two_conditions(
    "This creature has haste as long as it's your turn and you control a Plains."
)]
#[case::a_turn_among_others(
    "This creature has haste as long as it's your turn or you control a Plains."
)]
#[case::a_clause_after("This creature has haste as long as it's your turn, and it can't block.")]
#[case::no_condition_words("This creature has haste as long as.")]
fn a_condition_that_is_not_a_known_one_stays_with_the_table(#[case] text: &str) {
    assert_eq!(stat(text), None, "{text}");
}

#[rstest]
#[case::two_colors(
    "Equipped creature gets +2/+2 and has protection from white and from black.",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":2,"toughness":2}},{"AddKeyword":{"Protection":[{"Color":"White"}]}},{"AddKeyword":{"Protection":[{"Color":"Black"}]}}]}}"#
)]
#[case::sword_of_body_and_mind(
    "Equipped creature gets +2/+2 and has protection from green and from blue.",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":2,"toughness":2}},{"AddKeyword":{"Protection":[{"Color":"Green"}]}},{"AddKeyword":{"Protection":[{"Color":"Blue"}]}}]}}"#
)]
#[case::one_color(
    "Enchanted creature has protection from red.",
    r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":{"Protection":[{"Color":"Red"}]}}]}}"#
)]
#[case::a_keyword_and_a_protection(
    "Enchanted creature gets +1/+1 and has flying and protection from red.",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":1,"toughness":1}},{"AddKeyword":"Flying"},{"AddKeyword":{"Protection":[{"Color":"Red"}]}}]}}"#
)]
#[case::a_protection_and_a_keyword(
    "Equipped creature has protection from red and from blue and flying.",
    r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":{"Protection":[{"Color":"Red"}]}},{"AddKeyword":{"Protection":[{"Color":"Blue"}]}},{"AddKeyword":"Flying"}]}}"#
)]
#[case::three_qualities(
    "Equipped creature has protection from red and from white and from creatures.",
    r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":{"Protection":[{"Color":"Red"}]}},{"AddKeyword":{"Protection":[{"Color":"White"}]}},{"AddKeyword":{"Protection":[{"Type":"Creature"}]}}]}}"#
)]
#[case::a_group(
    "Creatures you control have protection from colorless.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":{"Protection":["Colorless"]}}]}}"#
)]
#[case::everything(
    "Enchanted creature has protection from everything.",
    r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":{"Protection":[]}}]}}"#
)]
fn a_list_of_protections_reads_as_the_hand_written_files_say(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::until_end_of_turn(
    "Target creature gains protection from red until end of turn.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":{"Protection":[{"Color":"Red"}]}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::two_colors_and_a_bonus(
    "Target creature gets +2/+2 and gains protection from red and from blue until end of turn.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":2,"toughness":2}},{"AddKeyword":{"Protection":[{"Color":"Red"}]}},{"AddKeyword":{"Protection":[{"Color":"Blue"}]}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
fn a_pump_with_protections_reads_as_the_hand_written_files_say(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::a_subtype_quality("Equipped creature has protection from Vampires.")]
#[case::without_the_second_from("Equipped creature has protection from red and blue.")]
#[case::a_comma("Equipped creature has protection from red, from blue.")]
#[case::no_quality("Equipped creature has protection from.")]
#[case::no_from("Equipped creature has protection red.")]
#[case::a_chosen_color("Equipped creature has protection from the chosen color.")]
#[case::a_player("Equipped creature has protection from each player.")]
#[case::all_colors("Equipped creature has protection from all colors.")]
#[case::a_second_protection("Equipped creature has protection from red and protection from blue.")]
#[case::an_unknown_quality("Equipped creature has protection from sorceries.")]
#[case::a_trailing_and("Equipped creature has protection from red and.")]
fn a_protection_that_is_not_a_known_one_stays_with_the_table(#[case] text: &str) {
    assert_eq!(stat(text), None, "{text}");
}

#[test]
fn the_wolf_token_and_the_mill_of_sword_of_body_and_mind_read_as_its_file_says() {
    reads_as(
        "Whenever equipped creature deals combat damage to a player, you create a 2/2 green Wolf creature token and that player mills ten cards.",
        ClauseKind::Triggered,
        false,
        r#"{"Triggered":{"when":{"DealsDamage":{"source":[{"Is":"Attached"}],"to":"EachPlayer","combat":true}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Wolf","colors":["Green"],"types":["Creature"],"subtypes":["Wolf"],"power":2,"toughness":2}}}},{"Mill":{"who":"Triggering","count":10}}]}}"#,
    );
}

#[rstest]
#[case::a_treasure(
    "Whenever equipped creature deals combat damage to a player, you create a Treasure token and that player mills three cards.",
    r#"{"Triggered":{"when":{"DealsDamage":{"source":[{"Is":"Attached"}],"to":"EachPlayer","combat":true}},"effects":[{"CreateToken":{"token":{"Named":"treasure"}}},{"Mill":{"who":"Triggering","count":3}}]}}"#
)]
#[case::the_card_itself(
    "Whenever ~ deals damage to an opponent, you create a Clue token and that player mills a card.",
    r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"Opponents"}},"effects":[{"CreateToken":{"token":{"Named":"clue"}}},{"Mill":{"who":"Triggering","count":1}}]}}"#
)]
fn a_token_and_a_mill_of_that_player_read_in_other_wordings(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::no_player(
    "Whenever ~ deals damage to a creature, you create a Treasure token and that player mills three cards."
)]
#[case::another_player(
    "Whenever ~ deals combat damage to a player, you create a Treasure token and target player mills three cards."
)]
#[case::a_spell_has_no_that_player(
    "You create a Treasure token and that player mills three cards."
)]
fn that_player_needs_a_player_in_the_event(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}

#[rstest]
#[case::the_thing_ben_grimm(
    "Whenever one or more Heroes you control deal damage to a player, put two +1/+1 counters on ~.",
    r#"{"Triggered":{"when":{"Batch":{"DealsDamage":{"source":[{"Subtype":"Hero"},{"ControlledBy":"You"}],"to":"EachPlayer"}}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1","count":2}}]}}"#
)]
#[case::an_irregular_plural(
    "Whenever one or more Elves you control deal combat damage to a player, draw a card.",
    r#"{"Triggered":{"when":{"Batch":{"DealsDamage":{"source":[{"Subtype":"Elf"},{"ControlledBy":"You"}],"to":"EachPlayer","combat":true}}},"effects":[{"Draw":{"who":"You","count":1}}]}}"#
)]
#[case::wolves(
    "Whenever one or more Wolves you control deal damage to an opponent, draw a card.",
    r#"{"Triggered":{"when":{"Batch":{"DealsDamage":{"source":[{"Subtype":"Wolf"},{"ControlledBy":"You"}],"to":"Opponents"}}},"effects":[{"Draw":{"who":"You","count":1}}]}}"#
)]
#[case::an_ordinary_plural(
    "Whenever one or more Vampires you control deal damage to a player, draw a card.",
    r#"{"Triggered":{"when":{"Batch":{"DealsDamage":{"source":[{"Subtype":"Vampire"},{"ControlledBy":"You"}],"to":"EachPlayer"}}},"effects":[{"Draw":{"who":"You","count":1}}]}}"#
)]
#[case::attacking(
    "Whenever two or more Pirates you control attack, draw a card.",
    r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"Subtype":"Pirate"},{"ControlledBy":"You"}],"at_least":2}},"effects":[{"Draw":{"who":"You","count":1}}]}}"#
)]
fn a_plural_subtype_reads_as_the_hand_written_files_say(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case("Destroy all Goblins.")]
#[case("Destroy all Elves.")]
#[case("Destroy all Wolves.")]
#[case("Destroy all Heroes.")]
#[case("Destroy all Spies.")]
#[case("Destroy all Foxes.")]
#[case("Destroy all Dwarves.")]
#[case("Destroy all Mice.")]
#[case("Destroy all Plains.")]
#[case("Destroy all Fungi.")]
#[case("Destroy all Werewolves.")]
fn the_plurals_of_subtypes_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case::the_wrong_plural("Destroy all Elfs.")]
#[case::a_double_plural("Destroy all Goblinss.")]
#[case::a_singular("Destroy all Goblin.")]
#[case::a_misspelling("Destroy all Heroees.")]
#[case::not_a_subtype("Destroy all Zorks.")]
#[case::a_card_type_in_capitals("Destroy all Creatures.")]
#[case::lowercase("Destroy all goblins.")]
fn a_plural_that_is_not_a_subtype_stays_with_the_table(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[test]
fn a_subtype_of_the_cards_own_type_line_is_read_in_the_singular_and_the_plural() {
    let text = "Whenever one or more Gizmos you control deal damage to a player, draw a card.";
    assert_eq!(trigger(text), None);
    let own = Own {
        subtypes: vec!["Gizmo".to_owned()],
        ..Own::default()
    };
    let read = clause_with(text, ClauseKind::Triggered, false, &own).unwrap();
    assert!(
        render_variants(&read)
            .iter()
            .any(|written| same_text(written, text))
    );
    let single = "Whenever a Gizmo you control dies, draw a card.";
    assert_eq!(trigger(single), None);
    assert!(clause_with(single, ClauseKind::Triggered, false, &own).is_some());
    // Only the subtype the type line names.
    let other = "Whenever one or more Widgets you control deal damage to a player, draw a card.";
    assert_eq!(clause_with(other, ClauseKind::Triggered, false, &own), None);
}

#[rstest]
#[case::bloodghast(
    "Whenever a land you control enters, you may return this card from your graveyard to the battlefield.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Land"},{"ControlledBy":"You"}],"to":"Battlefield"}},"from":"Graveyard","effects":[{"May":{"effects":[{"Move":{"what":"This","to":{"Battlefield":{}}}}]}}]}}"#
)]
#[case::without_a_choice(
    "Whenever a creature you control enters, return this card from your graveyard to the battlefield.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"to":"Battlefield"}},"from":"Graveyard","effects":[{"Move":{"what":"This","to":{"Battlefield":{}}}}]}}"#
)]
fn a_card_that_returns_itself_has_its_ability_in_the_graveyard(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::its_own_event(
    "When ~ dies, you may return this card from your graveyard to the battlefield."
)]
#[case::its_own_entering("When ~ enters, return this card from your graveyard to the battlefield.")]
#[case::attacking("Whenever ~ attacks, return this card from your graveyard to the battlefield.")]
#[case::to_the_hand(
    "Whenever a land you control enters, return this card from your graveyard to your hand."
)]
fn a_return_of_the_card_from_the_graveyard_stays_with_the_table_otherwise(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}

#[test]
fn a_return_from_the_graveyard_is_not_a_spell() {
    assert_eq!(
        spell("Return this card from your graveyard to the battlefield."),
        None
    );
}
