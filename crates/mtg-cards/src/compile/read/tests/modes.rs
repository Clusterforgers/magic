//! Goal: "Choose one —" and its bullets read as one `Modal` whose modes are read by the effect templates and keep targets of their own; other choices ("choose two", "one or both", "up to"), one mode alone, a mode the templates do not read and a modal beside other sentences stay with the table.
use super::*;

/// A header, a dash and a bullet line for each mode.
fn lines(header: &str, modes: &[&str]) -> String {
    let bullets: Vec<String> = modes
        .iter()
        .map(|mode| format!("\u{2022} {mode}"))
        .collect();
    format!("{header} \u{2014}\n{}", bullets.join("\n"))
}

fn choose(modes: &[&str]) -> String {
    lines("Choose one", modes)
}

fn modal(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Modal, false)
}

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::izzet_charm(
    &[
        "Counter target noncreature spell unless its controller pays {2}.",
        "~ deals 2 damage to target creature.",
        "Draw two cards, then discard two cards.",
    ],
    r#"{"Spell":{"effects":[{"Modal":{"modes":[
        {"targets":[{"what":{"Object":["Spell",{"Not":{"Type":"Creature"}}]}}],"effects":[{"Pay":{"who":{"ControllerOf":{"Target":0}},"cost":"{2}","unpaid":[{"Counter":{"what":{"Target":0}}}]}}]},
        {"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":2}}]},
        {"effects":[{"Draw":{"count":2}},{"Discard":{"count":2}}]}]}}]}}"#
)]
#[case::two_modes_without_targets(
    &["Draw a card.", "You gain 2 life."],
    r#"{"Spell":{"effects":[{"Modal":{"modes":[{"effects":[{"Draw":{}}]},{"effects":[{"GainLife":{"amount":2}}]}]}}]}}"#
)]
#[case::every_mode_has_its_own_target(
    &["Destroy target artifact.", "Destroy target enchantment."],
    r#"{"Spell":{"effects":[{"Modal":{"modes":[
        {"targets":[{"what":{"Object":[{"Type":"Artifact"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]},
        {"targets":[{"what":{"Object":[{"Type":"Enchantment"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}]}}]}}"#
)]
#[case::a_mode_of_two_sentences(
    &["Scry 2. Draw a card.", "Target player discards a card."],
    r#"{"Spell":{"effects":[{"Modal":{"modes":[
        {"effects":[{"Scry":{"count":2}},{"Draw":{}}]},
        {"targets":[{"what":"Player"}],"effects":[{"Discard":{"who":{"Target":0}}}]}]}}]}}"#
)]
fn modes_in_a_spell_read_in_the_shape_of_the_hand_written_files(
    #[case] modes: &[&str],
    #[case] json: &str,
) {
    let json: String = json.split_whitespace().collect();
    reads_as(&choose(modes), ClauseKind::Modal, false, &json);
}

#[rstest]
#[case::enters(
    "When ~ enters, ",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Modal":{"modes":[{"effects":[{"Draw":{}}]},{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}}]}]}}]}}"#
)]
#[case::attacks(
    "Whenever ~ attacks, ",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Modal":{"modes":[{"effects":[{"Draw":{}}]},{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}}]}]}}]}}"#
)]
fn modes_in_a_trigger_read_in_the_shape_of_the_hand_written_files(
    #[case] head: &str,
    #[case] json: &str,
) {
    let modes = choose(&["Draw a card.", "~ deals 1 damage to any target."]);
    let text = format!("{head}{}", modes.replacen("Choose", "choose", 1));
    reads_as(&text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::dromars_charm(&["You gain 5 life.", "Counter target spell.", "Target creature gets -2/-2 until end of turn."])]
#[case::esper_charm(&["Destroy target enchantment.", "Draw two cards.", "Target player discards two cards."])]
#[case::boros_charm(&[
    "~ deals 4 damage to target player or planeswalker.",
    "Permanents you control gain indestructible until end of turn.",
    "Target creature gains double strike until end of turn.",
])]
#[case::abrade(&["~ deals 3 damage to target creature.", "Destroy target artifact."])]
#[case::valorous_stance(&[
    "Target creature gains indestructible until end of turn.",
    "Destroy target creature with toughness 4 or greater.",
])]
#[case::sultai_charm_partly(&["Destroy target artifact or enchantment.", "Draw two cards, then discard a card."])]
#[case::a_choice_of_players(&["Target opponent loses 3 life.", "Each opponent discards a card."])]
#[case::a_choice_of_a_pump(&["Creatures you control get +1/+1 until end of turn.", "Target creature gets +3/+3 until end of turn."])]
#[case::a_pay_in_a_mode(&["Counter target spell unless its controller pays {1}.", "Draw a card."])]
#[case::four_modes(&["Draw a card.", "You gain 2 life.", "Scry 1.", "Each opponent loses 1 life."])]
fn more_oracle_wordings_of_modes_read_and_write_back(#[case] modes: &[&str]) {
    golden(&choose(modes), ClauseKind::Modal, false);
}

#[rstest]
#[case::enters(
    "When ~ enters, ",
    &["Draw a card.", "You gain 2 life."]
)]
#[case::attacks("Whenever ~ attacks, ", &["Scry 1.", "Destroy target artifact."])]
#[case::dies("When ~ dies, ", &["Each opponent loses 2 life.", "You gain 2 life."])]
#[case::cast("Whenever you cast a creature spell, ", &["Draw a card.", "You gain 1 life."])]
#[case::upkeep("At the beginning of your upkeep, ", &["Scry 1.", "Each opponent loses 1 life."])]
fn more_oracle_wordings_of_modes_in_a_trigger_read_and_write_back(
    #[case] head: &str,
    #[case] modes: &[&str],
) {
    let text = format!("{head}{}", choose(modes).replacen("Choose", "choose", 1));
    assert!(trigger(&text).is_some(), "{text}");
    golden(&text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::choose_two(lines("Choose two", &["Draw a card.", "You gain 2 life.", "Scry 1."]))]
#[case::both(lines("Choose one or both", &["Draw a card.", "You gain 2 life."]))]
#[case::one_or_more(lines("Choose one or more", &["Draw a card.", "You gain 2 life."]))]
#[case::up_to(lines("Choose up to two", &["Draw a card.", "You gain 2 life.", "Scry 1."]))]
#[case::three(lines("Choose three", &["Draw a card.", "You gain 2 life.", "Scry 1.", "Each opponent loses 1 life."]))]
#[case::any_number(lines("Choose any number", &["Draw a card.", "You gain 2 life."]))]
#[case::lowercase(lines("choose one", &["Draw a card.", "You gain 2 life."]))]
#[case::one_mode(choose(&["Draw a card."]))]
#[case::no_modes("Choose one \u{2014}".to_owned())]
#[case::an_unread_mode(choose(&["Draw a card.", "Goad target creature."]))]
#[case::an_unread_first_mode(choose(&["Goad target creature.", "Draw a card."]))]
#[case::an_empty_mode("Choose one \u{2014}\n\u{2022} Draw a card.\n\u{2022}".to_owned())]
#[case::a_second_target(choose(&["Destroy target artifact and target enchantment.", "Draw a card."]))]
#[case::it_from_another_mode(choose(&["Destroy target creature.", "Its controller gains 3 life."]))]
#[case::a_nested_choice("Choose one \u{2014}\n\u{2022} Choose one \u{2014} \u{2022} Draw a card. \u{2022} You gain 2 life.\n\u{2022} Draw a card.".to_owned())]
#[case::an_ascii_dash("Choose one -\n\u{2022} Draw a card.\n\u{2022} You gain 2 life.".to_owned())]
#[case::no_dash("Choose one\n\u{2022} Draw a card.\n\u{2022} You gain 2 life.".to_owned())]
#[case::no_bullets("Choose one \u{2014} Draw a card. You gain 2 life.".to_owned())]
#[case::another_bullet("Choose one \u{2014}\n- Draw a card.\n- You gain 2 life.".to_owned())]
#[case::a_cost_on_a_mode(choose(&["You may pay {1}. If you do, draw a card.", "You gain 2 life."]))]
#[case::a_choice_of_a_number(choose(&["Draw X cards.", "You gain 2 life."]))]
#[case::a_plain_sentence("Draw a card.".to_owned())]
fn a_choice_the_template_does_not_model_stays_unread(#[case] text: String) {
    assert_eq!(modal(&text), None, "{text}");
}

#[test]
fn a_modal_is_the_only_effect_of_its_ability() {
    let modes = choose(&["Draw a card.", "You gain 2 life."]);
    assert_eq!(spell(&format!("Draw a card.\n{modes}")), None);
    let trigger_modes = modes.replacen("Choose", "choose", 1);
    assert_eq!(
        trigger(&format!("When ~ enters, draw a card. {trigger_modes}")),
        None
    );
    assert!(trigger(&format!("When ~ enters, {trigger_modes}")).is_some());
}

#[test]
fn modes_read_in_a_spell_only_through_the_modal_kind_or_the_spell_kind() {
    let text = choose(&["Draw a card.", "You gain 2 life."]);
    assert_eq!(spell(&text), modal(&text));
    assert!(modal(&text).is_some());
    assert_eq!(clause(&text, ClauseKind::Static, false), None);
}

#[test]
fn the_targets_of_the_modes_are_not_the_abilitys() {
    let Some(Ability::Spell { targets, effects }) = modal(&choose(&[
        "Destroy target artifact.",
        "Destroy target enchantment.",
    ])) else {
        panic!("not read");
    };
    assert_eq!(targets, Vec::new());
    assert_eq!(effects.len(), 1);
}
