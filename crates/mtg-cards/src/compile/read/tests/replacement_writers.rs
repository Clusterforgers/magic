//! Goal: a replacement of damage, tokens, counters, a draw or a move between zones is written as the card words it, in every wording the card may use, and the wording of another player, source, kind of counter, number or zone is not its text.
use super::*;

/// Whether one of the wordings of the ability is the text.
fn writes(json: &str, text: &str) -> bool {
    render_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, text))
}

#[rstest]
#[case::furnace_of_rath(
    r#"{"Replacement":{"when":{"Damage":{}},"by":{"Multiply":2}}}"#,
    "If a source would deal damage to a permanent or player, it deals double that damage to that permanent or player instead."
)]
#[case::fiendish_duo(
    r#"{"Replacement":{"when":{"Damage":{"to":"Opponents"}},"by":{"Multiply":2}}}"#,
    "If a source would deal damage to an opponent, it deals double that damage to that player instead."
)]
#[case::gisela_with_that_source(
    r#"{"Replacement":{"when":{"Damage":{"to":"Opponents"}},"by":{"Multiply":2}}}"#,
    "If a source would deal damage to an opponent, that source deals double that damage to that player instead."
)]
#[case::the_sound_of_drums(
    r#"{"Replacement":{"when":{"Damage":{"source":[{"Is":"Attached"}],"combat":true}},"by":{"Multiply":2}}}"#,
    "If enchanted creature would deal combat damage to a permanent or player, it deals double that damage instead."
)]
#[case::heart_shaped_herb(
    r#"{"Replacement":{"when":{"Damage":{"to":"You","source":[{"ControlledBy":"Opponents"}]}},"by":{"PreventUpTo":1}}}"#,
    "If a source an opponent controls would deal damage to you, prevent 1 of that damage."
)]
#[case::prevent_all_to_you(
    r#"{"Replacement":{"when":{"Damage":{"to":"You"}},"by":"Prevent"}}"#,
    "If damage would be dealt to you, prevent that damage."
)]
#[case::a_creature_deals_noncombat_damage(
    r#"{"Replacement":{"when":{"Damage":{"source":[{"Type":"Creature"},{"ControlledBy":"You"}],"combat":false}},"by":{"Add":2}}}"#,
    "If a creature you control would deal noncombat damage to a permanent or player, it deals that much damage plus 2 instead."
)]
#[case::another_source_you_control(
    r#"{"Replacement":{"when":{"Damage":{"to":"Opponents","source":["Other",{"ControlledBy":"You"}]}},"by":{"Add":1}}}"#,
    "If another source you control would deal damage to an opponent, it deals that much damage plus one instead."
)]
#[case::anointed_procession(
    r#"{"Replacement":{"when":{"TokenCreation":{}},"by":{"Multiply":2}}}"#,
    "If an effect would create one or more tokens under your control, it creates twice that many of those tokens instead."
)]
#[case::adrix_and_nev(
    r#"{"Replacement":{"when":{"TokenCreation":{}},"by":{"Multiply":2}}}"#,
    "If one or more tokens would be created under your control, twice that many of those tokens are created instead."
)]
#[case::corpsejack_menace(
    r#"{"Replacement":{"when":{"CountersPut":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"counter":"+1/+1"}},"by":{"Multiply":2}}}"#,
    "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on it instead."
)]
#[case::hardened_scales(
    r#"{"Replacement":{"when":{"CountersPut":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"counter":"+1/+1"}},"by":{"Add":1}}}"#,
    "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead."
)]
#[case::kami_of_whispered_hopes(
    r#"{"Replacement":{"when":{"CountersPut":{"on":{"Each":[{"ControlledBy":"You"}]},"counter":"+1/+1"}},"by":{"Add":1}}}"#,
    "If one or more +1/+1 counters would be put on a permanent you control, that many plus one +1/+1 counters are put on that permanent instead."
)]
#[case::benevolent_hydra(
    r#"{"Replacement":{"when":{"CountersPut":{"on":{"Each":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]},"counter":"+1/+1"}},"by":{"Add":1}}}"#,
    "If one or more +1/+1 counters would be put on another creature you control, that many plus one +1/+1 counters are put on it instead."
)]
#[case::doubling_season_counters(
    r#"{"Replacement":{"when":{"CountersPut":{"on":{"Each":[{"ControlledBy":"You"}]}}},"by":{"Multiply":2}}}"#,
    "If one or more counters would be put on a permanent you control, twice that many of those counters are put on it instead."
)]
#[case::doubling_season_as_an_effect_puts(
    r#"{"Replacement":{"when":{"CountersPut":{"on":{"Each":[{"ControlledBy":"You"}]}}},"by":{"Multiply":2}}}"#,
    "If an effect would put one or more counters on a permanent you control, it puts twice that many of those counters on that permanent instead."
)]
#[case::thought_reflection(
    r#"{"Replacement":{"when":{"Draw":{}},"by":{"Multiply":2}}}"#,
    "If you would draw a card, draw two cards instead."
)]
#[case::rest_in_peace(
    r#"{"Replacement":{"when":{"ZoneChange":{"what":[],"to":"Graveyard"}},"by":{"MoveTo":"Exile"}}}"#,
    "If a card or token would be put into a graveyard from anywhere, exile it instead."
)]
#[case::leyline_of_the_void(
    r#"{"Replacement":{"when":{"ZoneChange":{"what":[{"OwnedBy":"Opponents"}],"to":"Graveyard"}},"by":{"MoveTo":"Exile"}}}"#,
    "If a card would be put into an opponent's graveyard from anywhere, exile it instead."
)]
#[case::lorcan_warlocks_die(
    r#"{"Replacement":{"when":{"ZoneChange":{"what":[{"Subtype":"Warlock"},{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}},"by":{"MoveTo":"Exile"}}}"#,
    "If a Warlock you control would die, exile it instead."
)]
#[case::nontoken_creatures_of_opponents_die(
    r#"{"Replacement":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"Not":"Token"},{"ControlledBy":"Opponents"}],"from":"Battlefield","to":"Graveyard"}},"by":{"MoveTo":"Exile"}}}"#,
    "If a nontoken creature an opponent controls would die, exile it instead."
)]
#[case::a_creature_leaves_to_the_hand(
    r#"{"Replacement":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"from":"Battlefield"}},"by":{"MoveTo":"Hand"}}}"#,
    "If a creature you control would leave the battlefield, return it to its owner's hand instead."
)]
fn replacements_are_written_as_the_card_words_them(#[case] json: &str, #[case] text: &str) {
    assert!(
        writes(json, text),
        "{text}: {:?}",
        render_variants(&ability(json))
    );
}

#[rstest]
#[case::fiendish_duo_for_you(
    r#"{"Replacement":{"when":{"Damage":{"to":"You"}},"by":{"Multiply":2}}}"#,
    "If a source would deal damage to an opponent, it deals double that damage to that player instead."
)]
#[case::triple_for_double(
    r#"{"Replacement":{"when":{"Damage":{}},"by":{"Multiply":3}}}"#,
    "If a source would deal damage to a permanent or player, it deals double that damage to that permanent or player instead."
)]
#[case::combat_only_for_any_damage(
    r#"{"Replacement":{"when":{"Damage":{"combat":true}},"by":{"Multiply":2}}}"#,
    "If a source would deal damage to a permanent or player, it deals double that damage to that permanent or player instead."
)]
#[case::your_sources_for_an_opponents(
    r#"{"Replacement":{"when":{"Damage":{"to":"You","source":[{"ControlledBy":"You"}]}},"by":{"PreventUpTo":1}}}"#,
    "If a source an opponent controls would deal damage to you, prevent 1 of that damage."
)]
#[case::two_for_one_prevented(
    r#"{"Replacement":{"when":{"Damage":{"to":"You","source":[{"ControlledBy":"Opponents"}]}},"by":{"PreventUpTo":2}}}"#,
    "If a source an opponent controls would deal damage to you, prevent 1 of that damage."
)]
#[case::plus_two_for_plus_one(
    r#"{"Replacement":{"when":{"CountersPut":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"counter":"+1/+1"}},"by":{"Add":2}}}"#,
    "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead."
)]
#[case::minus_counters_for_plus_counters(
    r#"{"Replacement":{"when":{"CountersPut":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"counter":"-1/-1"}},"by":{"Multiply":2}}}"#,
    "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on it instead."
)]
#[case::opponents_creatures_for_yours(
    r#"{"Replacement":{"when":{"CountersPut":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]},"counter":"+1/+1"}},"by":{"Multiply":2}}}"#,
    "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on it instead."
)]
#[case::an_opponents_tokens(
    r#"{"Replacement":{"when":{"TokenCreation":{"controller":"Opponents"}},"by":{"Multiply":2}}}"#,
    "If an effect would create one or more tokens under your control, it creates twice that many of those tokens instead."
)]
#[case::exile_for_the_hand(
    r#"{"Replacement":{"when":{"ZoneChange":{"what":[{"Subtype":"Warlock"},{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}},"by":{"MoveTo":"Hand"}}}"#,
    "If a Warlock you control would die, exile it instead."
)]
#[case::dying_for_leaving(
    r#"{"Replacement":{"when":{"ZoneChange":{"what":[{"Subtype":"Warlock"},{"ControlledBy":"You"}],"from":"Battlefield"}},"by":{"MoveTo":"Exile"}}}"#,
    "If a Warlock you control would die, exile it instead."
)]
#[case::your_graveyard_for_an_opponents(
    r#"{"Replacement":{"when":{"ZoneChange":{"what":[{"OwnedBy":"You"}],"to":"Graveyard"}},"by":{"MoveTo":"Exile"}}}"#,
    "If a card would be put into an opponent's graveyard from anywhere, exile it instead."
)]
#[case::three_cards_for_two(
    r#"{"Replacement":{"when":{"Draw":{}},"by":{"Multiply":3}}}"#,
    "If you would draw a card, draw two cards instead."
)]
fn the_wording_of_another_replacement_is_not_the_text(#[case] json: &str, #[case] text: &str) {
    assert!(!writes(json, text), "{text}");
}

#[rstest]
#[case::a_condition(
    r#"{"Replacement":{"when":{"Damage":{}},"by":{"Multiply":2},"condition":"YourTurn"}}"#
)]
#[case::a_zone_the_card_does_not_name(
    r#"{"Replacement":{"when":{"ZoneChange":{"what":[],"from":"Hand","to":"Library"}},"by":{"MoveTo":"Exile"}}}"#
)]
#[case::an_effect_instead(
    r#"{"Replacement":{"when":{"Damage":{}},"by":{"Instead":[{"Draw":{}}]}}}"#
)]
#[case::from_another_zone(
    r#"{"Replacement":{"when":{"Damage":{}},"by":{"Multiply":2},"from":"Graveyard"}}"#
)]
fn a_replacement_the_writers_cannot_say_is_not_written(#[case] json: &str) {
    assert_eq!(render_variants(&ability(json)), Vec::<String>::new());
}
