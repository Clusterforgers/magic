//! Goal: "Choose two —" and "Choose one or both —" read as a `Modal` of those bounds whose modes keep their own targets, a fight of two targets, a shield from a target spell and a doubling read inside a mode, each writes back as the card's text, and every near-miss stays with the table.
use super::*;
use crate::effect::Effect;

const DROMOKA: &str = "Choose two \u{2014}\n\u{2022} Prevent all damage target instant or sorcery spell would deal this turn.\n\u{2022} Target player sacrifices an enchantment of their choice.\n\u{2022} Put a +1/+1 counter on target creature.\n\u{2022} Target creature you control fights target creature you don't control.";
const EPIC: &str = "Choose one or both \u{2014}\n\u{2022} Double target creature's power and toughness until end of turn.\n\u{2022} Target creature you control fights target creature an opponent controls.";
const BRIGID: &str = "Choose two \u{2014}\n\u{2022} Create a token that's a copy of target Kithkin you control.\n\u{2022} Target player creates a 1/1 green and white Kithkin creature token.\n\u{2022} Target creature you control gets +3/+3 until end of turn.\n\u{2022} Target creature you control fights target creature an opponent controls.";

/// The bullets after a header.
fn c9_fight_lines(header: &str, modes: &[&str]) -> String {
    let bullets: Vec<String> = modes.iter().map(|m| format!("\n\u{2022} {m}")).collect();
    format!("{header} \u{2014}{}", bullets.concat())
}

#[rstest]
#[case::dromoka(DROMOKA, 2, 2, 4)]
#[case::epic(EPIC, 1, 2, 2)]
#[case::brigid(BRIGID, 2, 2, 4)]
#[case::two_of_three(c9_fight_lines("Choose two", &["Draw a card.", "You gain 2 life.", "Scry 1."]).leak(), 2, 2, 3)]
#[case::both_of_a_pair(c9_fight_lines("Choose one or both", &["Draw a card.", "You gain 2 life."]).leak(), 1, 2, 2)]
fn c9_fight_choices_read_with_their_bounds_and_write_back(
    #[case] text: &str,
    #[case] min: u32,
    #[case] max: i32,
    #[case] count: usize,
) {
    let read = golden(text, ClauseKind::Modal, false);
    let [Effect::Modal { choose, modes, .. }] = read.effects() else {
        panic!("not one modal: {read:?}");
    };
    assert_eq!(choose.fixed_min(), Some(min), "{text}");
    assert_eq!(choose.max, Some(crate::value::Amount::Fixed(max)), "{text}");
    assert_eq!(modes.len(), count, "{text}");
}

#[rstest]
#[case::shield(
    "Prevent all damage target instant or sorcery spell would deal this turn.",
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]}}],"effects":[{"Shield":{"when":{"Damage":{"source":[{"Is":{"Target":0}}]}},"by":"Prevent","duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::double_both(
    "Double target creature's power and toughness until end of turn.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":{"Stat":{"of":{"Target":0},"stat":"Power"}},"toughness":{"Stat":{"of":{"Target":0},"stat":"Toughness"}}}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::double_power(
    "Double target creature's power until end of turn.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":{"Stat":{"of":{"Target":0},"stat":"Power"}},"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
fn c9_fight_mode_sentences_read_into_the_dsl(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::two_of_a_pair(c9_fight_lines("Choose two", &["Draw a card.", "You gain 2 life."]))]
#[case::both_of_three(c9_fight_lines("Choose one or both", &["Draw a card.", "You gain 2 life.", "Scry 1."]))]
#[case::two_at_random(c9_fight_lines("Choose two at random", &["Draw a card.", "You gain 2 life.", "Scry 1."]))]
#[case::three_of_three(c9_fight_lines("Choose three", &["Draw a card.", "You gain 2 life.", "Scry 1."]))]
#[case::two_with_an_unread_mode(c9_fight_lines("Choose two", &["Draw a card.", "You gain 2 life.", "Wish for a card."]))]
#[case::two_for_a_commander("Choose two. If you control a commander as you cast this spell, you may choose both instead.\n\u{2022} Draw a card.\n\u{2022} You gain 2 life.\n\u{2022} Scry 1.".to_owned())]
#[case::shield_next_turn("Prevent all damage target instant or sorcery spell would deal next turn.".to_owned())]
#[case::shield_two_targets("Prevent all damage target creature would deal this turn to target player.".to_owned())]
#[case::double_forever("Double target creature's power.".to_owned())]
#[case::double_toughness("Double target creature's toughness until end of turn.".to_owned())]
fn c9_fight_near_misses_stay_unread(#[case] text: String) {
    let kind = if text.starts_with("Choose") {
        ClauseKind::Modal
    } else {
        ClauseKind::Spell
    };
    assert!(clause(&text, kind, false).is_none(), "{text}");
}

#[test]
fn c9_fight_a_trigger_chooses_two_and_writes_back() {
    let text = "Whenever ~ attacks, choose two \u{2014}\n\u{2022} Draw a card.\n\u{2022} You gain 2 life.\n\u{2022} Scry 1.";
    let read = golden(text, ClauseKind::Triggered, false);
    let [Effect::Modal { choose, .. }] = read.effects() else {
        panic!("not one modal: {read:?}");
    };
    assert_eq!(choose.fixed_min(), Some(2));
}

/// A reduction of the spell itself while it targets what `wanted` selects.
fn c9_fight_targeting(mana: &str, wanted: &str) -> String {
    format!(
        r#"{{"CostModifier":{{"applies_to":["This",{{"Targeting":[{wanted}]}}],"change":"Reduce","mana":"{mana}"}}}}"#
    )
}

#[rstest]
#[case::a_dinosaur_you_control(
    "This spell costs {2} less to cast if it targets a Dinosaur you control.",
    "{2}",
    r#"{"Subtype":"Dinosaur"},{"ControlledBy":"You"}"#
)]
#[case::a_legendary_creature(
    "This spell costs {2} less to cast if it targets a legendary creature.",
    "{2}",
    r#"{"Type":"Creature"},{"Supertype":"Legendary"}"#
)]
#[case::an_attacking_creature(
    "This spell costs {2} less to cast if it targets an attacking creature.",
    "{2}",
    r#"{"Type":"Creature"},"Attacking""#
)]
#[case::an_artifact(
    "This spell costs {1} less to cast if it targets an artifact.",
    "{1}",
    r#"{"Type":"Artifact"}"#
)]
fn c9_fight_a_reduction_for_a_target_reads_as_targeting(
    #[case] text: &str,
    #[case] mana: &str,
    #[case] wanted: &str,
) {
    reads_as(
        text,
        ClauseKind::Static,
        false,
        &c9_fight_targeting(mana, wanted),
    );
}

#[rstest]
#[case::two_targets("This spell costs {2} less to cast if it targets two creatures.")]
#[case::another_spell(
    "Creature spells you cast cost {2} less to cast if it targets a Dinosaur you control."
)]
#[case::and_a_condition(
    "This spell costs {2} less to cast if it targets a Dinosaur you control and you control a Dragon."
)]
#[case::no_article("This spell costs {2} less to cast if it targets Dinosaurs you control.")]
fn c9_fight_a_reduction_for_another_target_stays_unread(#[case] text: &str) {
    assert!(clause(text, ClauseKind::Static, false).is_none(), "{text}");
}
