//! Goal: a characteristic-defining ability of power and toughness ("~'s power and toughness are each equal to the number of lands you control", "~'s power is equal to the number of cards in your hand", "... and its toughness is equal to that number plus 1") reads into the defining static ability of the hand-written files, takes the stat it leaves out from the printed number, writes back as the same text, and leaves every near-miss to the table.
use super::*;

/// A defining static ability that sets power and toughness.
fn defining(power: &str, toughness: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":"This","mods":[{{"SetPT":{{"power":{power},"toughness":{toughness}}}}}],"defining":true}}}}"#
    )
}

/// Reads the wording on a face with these printed numbers and checks that the renderer writes the same text back.
fn defined(text: &str, power: Option<i32>, toughness: Option<i32>) -> Ability {
    let own = Own {
        power,
        toughness,
        ..Own::default()
    };
    let read = clause_with(text, ClauseKind::Static, false, &own)
        .unwrap_or_else(|| panic!("not read: {text}"));
    assert!(
        render_variants(&read)
            .iter()
            .any(|written| same_text(written, text)),
        "{text}: {:?}",
        render_variants(&read)
    );
    read
}

const LANDS: &str = r#"{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}"#;
const CREATURES: &str = r#"{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#;
const HAND: &str = r#"{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}"#;
const CARD_TYPES_EVERYWHERE: &str = r#"{"CardTypes":[{"In":"Graveyard"}]}"#;

#[rstest]
#[case::lands(
    "~'s power and toughness are each equal to the number of lands you control.",
    LANDS,
    LANDS
)]
#[case::this_creature(
    "This creature's power and toughness are each equal to the number of lands you control.",
    LANDS,
    LANDS
)]
#[case::creatures(
    "~'s power and toughness are each equal to the number of creatures you control.",
    CREATURES,
    CREATURES
)]
#[case::a_subtype(
    "~'s power and toughness are each equal to the number of Swamps you control.",
    r#"{"Count":[{"Subtype":"Swamp"},{"ControlledBy":"You"}]}"#,
    r#"{"Count":[{"Subtype":"Swamp"},{"ControlledBy":"You"}]}"#
)]
#[case::artifacts(
    "~'s power and toughness are each equal to the number of artifacts you control.",
    r#"{"Count":[{"Type":"Artifact"},{"ControlledBy":"You"}]}"#,
    r#"{"Count":[{"Type":"Artifact"},{"ControlledBy":"You"}]}"#
)]
#[case::on_the_battlefield(
    "~'s power and toughness are each equal to the number of creatures on the battlefield.",
    r#"{"Count":[{"Type":"Creature"}]}"#,
    r#"{"Count":[{"Type":"Creature"}]}"#
)]
#[case::cards_in_your_hand(
    "~'s power and toughness are each equal to the number of cards in your hand.",
    HAND,
    HAND
)]
#[case::cards_in_a_graveyard(
    "~'s power and toughness are each equal to the number of creature cards in your graveyard.",
    r#"{"Count":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}"#,
    r#"{"Count":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}"#
)]
#[case::other_cards_in_a_graveyard(
    "~'s power and toughness are each equal to the number of other creature cards in your graveyard.",
    r#"{"Count":[{"Type":"Creature"},"Other",{"In":"Graveyard"},{"OwnedBy":"You"}]}"#,
    r#"{"Count":[{"Type":"Creature"},"Other",{"In":"Graveyard"},{"OwnedBy":"You"}]}"#
)]
#[case::cards_in_every_graveyard(
    "~'s power and toughness are each equal to the number of cards in all graveyards.",
    r#"{"Count":[{"In":"Graveyard"}]}"#,
    r#"{"Count":[{"In":"Graveyard"}]}"#
)]
#[case::creature_cards_in_every_graveyard(
    "~'s power and toughness are each equal to the number of creature cards in all graveyards.",
    r#"{"Count":[{"Type":"Creature"},{"In":"Graveyard"}]}"#,
    r#"{"Count":[{"Type":"Creature"},{"In":"Graveyard"}]}"#
)]
#[case::cards_in_the_graveyards_of_opponents(
    "~'s power and toughness are each equal to the number of cards in your opponents' graveyards.",
    r#"{"Count":[{"In":"Graveyard"},{"OwnedBy":"Opponents"}]}"#,
    r#"{"Count":[{"In":"Graveyard"},{"OwnedBy":"Opponents"}]}"#
)]
#[case::two_card_types(
    "~'s power and toughness are each equal to the number of instant and sorcery cards in your graveyard.",
    r#"{"Count":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"In":"Graveyard"},{"OwnedBy":"You"}]}"#,
    r#"{"Count":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"In":"Graveyard"},{"OwnedBy":"You"}]}"#
)]
#[case::your_life_total(
    "~'s power and toughness are each equal to your life total.",
    r#"{"Life":"You"}"#,
    r#"{"Life":"You"}"#
)]
#[case::twice_the_cards_in_your_hand(
    "~'s power and toughness are each equal to twice the number of cards in your hand.",
    &format!(r#"{{"Sum":[{HAND},{HAND}]}}"#),
    &format!(r#"{{"Sum":[{HAND},{HAND}]}}"#)
)]
#[case::two_counts(
    "~'s power and toughness are each equal to the number of Elves you control plus the number of creatures on the battlefield.",
    r#"{"Sum":[{"Count":[{"Subtype":"Elf"},{"ControlledBy":"You"}]},{"Count":[{"Type":"Creature"}]}]}"#,
    r#"{"Sum":[{"Count":[{"Subtype":"Elf"},{"ControlledBy":"You"}]},{"Count":[{"Type":"Creature"}]}]}"#
)]
#[case::a_count_and_a_number(
    "~'s power and toughness are each equal to the number of lands you control plus 2.",
    &format!(r#"{{"Sum":[{LANDS},2]}}"#),
    &format!(r#"{{"Sum":[{LANDS},2]}}"#)
)]
#[case::devotion(
    "~'s power and toughness are each equal to your devotion to black.",
    r#"{"Devotion":["Black"]}"#,
    r#"{"Devotion":["Black"]}"#
)]
#[case::devotion_to_two_colors(
    "~'s power and toughness are each equal to your devotion to black and red.",
    r#"{"Devotion":["Black","Red"]}"#,
    r#"{"Devotion":["Black","Red"]}"#
)]
#[case::the_greatest_value(
    "~'s power and toughness are each equal to the greatest mana value among creatures you control.",
    r#"{"Greatest":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"ManaValue"}}"#,
    r#"{"Greatest":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"ManaValue"}}"#
)]
#[case::the_total_power(
    "~'s power and toughness are each equal to the total power of creatures you control.",
    r#"{"Total":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Power"}}"#,
    r#"{"Total":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Power"}}"#
)]
fn power_and_toughness_defined_together_read_as_the_hand_written_files_say(
    #[case] text: &str,
    #[case] power: &str,
    #[case] toughness: &str,
) {
    assert_eq!(
        defined(text, None, None),
        ability(&defining(power, toughness)),
        "{text}"
    );
}

#[rstest]
#[case::power_alone(
    "~'s power is equal to the number of creatures you control.",
    (None, Some(4)), CREATURES, "4"
)]
#[case::power_alone_with_a_zero_toughness(
    "~'s power is equal to the number of cards in your hand.",
    (None, Some(0)), HAND, "0"
)]
#[case::toughness_alone(
    "~'s toughness is equal to the number of Knights you control.",
    (Some(2), None), "2", r#"{"Count":[{"Subtype":"Knight"},{"ControlledBy":"You"}]}"#
)]
#[case::a_number_before_a_count(
    "~'s power is equal to 1 plus the number of card types among cards in your opponents' graveyards.",
    (None, Some(3)),
    r#"{"Sum":[1,{"CardTypes":[{"In":"Graveyard"},{"OwnedBy":"Opponents"}]}]}"#, "3"
)]
fn a_stat_the_ability_leaves_out_is_the_printed_number(
    #[case] text: &str,
    #[case] printed: (Option<i32>, Option<i32>),
    #[case] power: &str,
    #[case] toughness: &str,
) {
    assert_eq!(
        defined(text, printed.0, printed.1),
        ability(&defining(power, toughness)),
        "{text}"
    );
}

#[rstest]
#[case::that_number_plus_one(
    "~'s power is equal to the number of card types among cards in all graveyards and its toughness is equal to that number plus 1.",
    &format!(r#"{{"Sum":[{CARD_TYPES_EVERYWHERE},1]}}"#)
)]
#[case::that_number_plus_two(
    "~'s power is equal to the number of card types among cards in all graveyards and its toughness is equal to that number plus 2.",
    &format!(r#"{{"Sum":[{CARD_TYPES_EVERYWHERE},2]}}"#)
)]
#[case::a_count_of_its_own(
    "~'s power is equal to the number of card types among cards in all graveyards and its toughness is equal to the number of lands you control.",
    LANDS
)]
fn power_and_toughness_defined_apart(#[case] text: &str, #[case] toughness: &str) {
    assert_eq!(
        defined(text, None, None),
        ability(&defining(CARD_TYPES_EVERYWHERE, toughness)),
        "{text}"
    );
}

#[rstest]
#[case::a_plain_number("~'s power and toughness are each equal to 3.")]
#[case::a_plain_number_alone("~'s power is equal to 3.")]
#[case::no_zone_for_cards("~'s power and toughness are each equal to the number of cards.")]
#[case::no_battlefield_for_lands("~'s power and toughness are each equal to the number of lands.")]
#[case::a_zone_the_filter_does_not_know(
    "~'s power and toughness are each equal to the number of cards in your library."
)]
#[case::the_graveyard_of_one_opponent(
    "~'s power and toughness are each equal to the number of cards in an opponent's graveyard."
)]
#[case::creatures_in_a_graveyard(
    "~'s power and toughness are each equal to the number of creatures in your graveyard."
)]
#[case::a_dangling_plus(
    "~'s power and toughness are each equal to the number of lands you control plus."
)]
#[case::plus_nothing_counted("~'s power and toughness are each equal to 1 plus 2.")]
#[case::its_own_stat("~'s power is equal to its toughness.")]
#[case::another_subject(
    "Target creature's power and toughness are each equal to the number of lands you control."
)]
#[case::the_wrong_verb("~'s power and toughness is equal to the number of lands you control.")]
#[case::the_wrong_stat("~'s loyalty is equal to the number of lands you control.")]
#[case::until_end_of_turn(
    "~'s power and toughness are each equal to the number of lands you control until end of turn."
)]
#[case::a_toughness_that_is_not_that_number(
    "~'s power is equal to the number of lands you control and its toughness is equal to that number minus 1."
)]
#[case::both_without_a_power("~'s power is equal to and its toughness is equal to 3.")]
#[case::a_devotion_to_no_color("~'s power and toughness are each equal to your devotion to.")]
#[case::a_devotion_to_a_word("~'s power and toughness are each equal to your devotion to tangled.")]
#[case::the_life_of_an_opponent(
    "~'s power and toughness are each equal to the life total of target opponent."
)]
#[case::an_unknown_stat(
    "~'s power and toughness are each equal to the greatest loyalty among planeswalkers you control."
)]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    let own = Own {
        power: Some(2),
        toughness: Some(3),
        ..Own::default()
    };
    assert_eq!(
        clause_with(text, ClauseKind::Static, false, &own),
        None,
        "{text}"
    );
}

#[test]
fn a_stat_left_out_without_a_printed_number_is_left_to_the_table() {
    let text = "~'s power is equal to the number of lands you control.";
    assert_eq!(clause(text, ClauseKind::Static, false), None);
    let star = Own {
        toughness: None,
        ..Own::default()
    };
    assert_eq!(clause_with(text, ClauseKind::Static, false, &star), None);
    let printed = Own {
        toughness: Some(5),
        ..Own::default()
    };
    assert!(clause_with(text, ClauseKind::Static, false, &printed).is_some());
}

#[rstest]
#[case::spell(
    "~'s power and toughness are each equal to the number of lands you control.",
    ClauseKind::Spell
)]
#[case::triggered(
    "~'s power and toughness are each equal to the number of lands you control.",
    ClauseKind::Triggered
)]
#[case::activated(
    "~'s power and toughness are each equal to the number of lands you control.",
    ClauseKind::Activated
)]
fn only_a_static_clause_defines_a_stat(#[case] text: &str, #[case] kind: ClauseKind) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}
