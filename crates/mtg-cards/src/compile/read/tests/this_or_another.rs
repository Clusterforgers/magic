//! Goal: "Whenever ~ or another X you control enters, dies, attacks, deals combat damage to a player or is dealt damage" reads as an `AnyOf` of the card's own event and the same event of the other objects, one verb at a time, writes back as the card's own text, and every wording that is not that shape stays with the table.
use super::*;
use crate::effect::Effect;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

/// What the subject does.
#[derive(Clone, Copy)]
enum Verb {
    Enters,
    Dies,
    Leaves,
    Attacks,
    CombatDamageToAPlayer,
    DamageToAnOpponent,
    IsDealtDamage,
}

const THIS: &str = r#"["This"]"#;

/// The event of the subject (a selector as JSON) doing the verb.
fn event(verb: Verb, subject: &str) -> String {
    match verb {
        Verb::Enters => format!(r#"{{"ZoneChange":{{"what":{subject},"to":"Battlefield"}}}}"#),
        Verb::Dies => format!(
            r#"{{"ZoneChange":{{"what":{subject},"from":"Battlefield","to":"Graveyard"}}}}"#
        ),
        Verb::Leaves => format!(r#"{{"ZoneChange":{{"what":{subject},"from":"Battlefield"}}}}"#),
        Verb::Attacks => format!(r#"{{"Attacks":{{"what":{subject}}}}}"#),
        Verb::CombatDamageToAPlayer => {
            format!(r#"{{"DealsDamage":{{"source":{subject},"to":"EachPlayer","combat":true}}}}"#)
        }
        Verb::DamageToAnOpponent => {
            format!(r#"{{"DealsDamage":{{"source":{subject},"to":"Opponents"}}}}"#)
        }
        Verb::IsDealtDamage => {
            let to = if subject == THIS {
                r#""This""#.to_owned()
            } else {
                format!(r#"{{"Each":{subject}}}"#)
            };
            format!(r#"{{"DealsDamage":{{"source":[],"to":{to}}}}}"#)
        }
    }
}

/// The events of the card and of the second subject for each verb in turn, as a triggered ability that draws a card.
fn any_of(verbs: &[Verb], second: &str) -> String {
    let events: Vec<String> = verbs
        .iter()
        .flat_map(|verb| [event(*verb, THIS), event(*verb, second)])
        .collect();
    format!(
        r#"{{"Triggered":{{"when":{{"AnyOf":[{}]}},"effects":[{{"Draw":{{}}}}]}}}}"#,
        events.join(",")
    )
}

const YOUR_CREATURE: &str = r#"[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]"#;

#[rstest]
#[case::enters(
    "Whenever ~ or another creature you control enters, draw a card.",
    &[Verb::Enters],
    YOUR_CREATURE
)]
#[case::dies(
    "Whenever ~ or another creature you control dies, draw a card.",
    &[Verb::Dies],
    YOUR_CREATURE
)]
#[case::this_creature(
    "Whenever this creature or another creature you control dies, draw a card.",
    &[Verb::Dies],
    YOUR_CREATURE
)]
#[case::of_anyone(
    "Whenever ~ or another creature dies, draw a card.",
    &[Verb::Dies],
    r#"[{"Type":"Creature"},"Other"]"#
)]
#[case::a_subtype(
    "Whenever ~ or another Elf you control enters, draw a card.",
    &[Verb::Enters],
    r#"[{"Subtype":"Elf"},"Other",{"ControlledBy":"You"}]"#
)]
#[case::a_subtype_of_anyone(
    "Whenever ~ or another Zombie dies, draw a card.",
    &[Verb::Dies],
    r#"[{"Subtype":"Zombie"},"Other"]"#
)]
#[case::nontoken(
    "Whenever ~ or another nontoken creature you control dies, draw a card.",
    &[Verb::Dies],
    r#"[{"Type":"Creature"},{"Not":"Token"},"Other",{"ControlledBy":"You"}]"#
)]
#[case::a_colour(
    "Whenever ~ or another black creature you control enters, draw a card.",
    &[Verb::Enters],
    r#"[{"Type":"Creature"},{"Color":"Black"},"Other",{"ControlledBy":"You"}]"#
)]
#[case::colorless(
    "Whenever ~ or another colorless creature you control dies, draw a card.",
    &[Verb::Dies],
    r#"[{"Type":"Creature"},"Colorless","Other",{"ControlledBy":"You"}]"#
)]
#[case::a_card_type(
    "Whenever this creature or another artifact you control enters, draw a card.",
    &[Verb::Enters],
    r#"[{"Type":"Artifact"},"Other",{"ControlledBy":"You"}]"#
)]
#[case::on_an_enchantment(
    "Whenever this enchantment or another enchantment you control enters, draw a card.",
    &[Verb::Enters],
    r#"[{"Type":"Enchantment"},"Other",{"ControlledBy":"You"}]"#
)]
#[case::two_card_types(
    "Whenever this creature or another creature or planeswalker you control dies, draw a card.",
    &[Verb::Dies],
    r#"[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]},"Other",{"ControlledBy":"You"}]"#
)]
#[case::with_a_keyword(
    "Whenever ~ or another creature you control with flying enters, draw a card.",
    &[Verb::Enters],
    r#"[{"Type":"Creature"},"Other",{"ControlledBy":"You"},{"HasKeyword":"Flying"}]"#
)]
#[case::with_a_mana_value(
    "Whenever this creature or another creature you control with mana value 4 or greater dies, draw a card.",
    &[Verb::Dies],
    r#"[{"Type":"Creature"},"Other",{"ControlledBy":"You"},{"Compare":{"stat":"ManaValue","op":"Ge","to":4}}]"#
)]
#[case::leaves_the_battlefield(
    "Whenever ~ or another creature you control leaves the battlefield, draw a card.",
    &[Verb::Leaves],
    YOUR_CREATURE
)]
#[case::attacks(
    "Whenever ~ or another Dragon you control attacks, draw a card.",
    &[Verb::Attacks],
    r#"[{"Subtype":"Dragon"},"Other",{"ControlledBy":"You"}]"#
)]
#[case::combat_damage_to_a_player(
    "Whenever this creature or another Tyranid you control deals combat damage to a player, draw a card.",
    &[Verb::CombatDamageToAPlayer],
    r#"[{"Subtype":"Tyranid"},"Other",{"ControlledBy":"You"}]"#
)]
#[case::damage_to_an_opponent(
    "Whenever ~ or another creature you control deals damage to an opponent, draw a card.",
    &[Verb::DamageToAnOpponent],
    YOUR_CREATURE
)]
#[case::is_dealt_damage(
    "Whenever ~ or another creature you control is dealt damage, draw a card.",
    &[Verb::IsDealtDamage],
    YOUR_CREATURE
)]
#[case::two_verbs_in_turn(
    "Whenever ~ or another nontoken Spirit you control dies or deals combat damage to a player, draw a card.",
    &[Verb::Dies, Verb::CombatDamageToAPlayer],
    r#"[{"Subtype":"Spirit"},{"Not":"Token"},"Other",{"ControlledBy":"You"}]"#
)]
#[case::enters_or_attacks(
    "Whenever ~ or another creature you control enters or attacks, draw a card.",
    &[Verb::Enters, Verb::Attacks],
    YOUR_CREATURE
)]
#[case::the_equipped_creature(
    "Whenever ~ or equipped creature attacks, draw a card.",
    &[Verb::Attacks],
    r#"[{"Is":"Attached"}]"#
)]
#[case::the_equipped_creature_deals_damage(
    "Whenever ~ or equipped creature deals combat damage to a player, draw a card.",
    &[Verb::CombatDamageToAPlayer],
    r#"[{"Is":"Attached"}]"#
)]
fn the_card_or_another_object_reads_as_an_any_of_of_their_events(
    #[case] text: &str,
    #[case] verbs: &[Verb],
    #[case] second: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, &any_of(verbs, second));
}

#[rstest]
#[case::an_ability_word(
    "Constellation \u{2014} Whenever this creature or another enchantment you control enters, draw a card.",
    "Whenever this creature or another enchantment you control enters, draw a card."
)]
#[case::another_ability_word(
    "Eerie \u{2014} Whenever ~ or another Elf you control dies, draw a card.",
    "Whenever ~ or another Elf you control dies, draw a card."
)]
fn an_ability_word_before_the_event_is_flavor(#[case] text: &str, #[case] rest: &str) {
    let read = trigger(text).unwrap_or_else(|| panic!("not read: {text}"));
    assert_eq!(read, golden(rest, ClauseKind::Triggered, false), "{text}");
}

#[rstest]
#[case::the_player_of_a_damage_event(
    "Whenever ~ or another creature you control deals combat damage to a player, that player loses 1 life.",
    r#"{"LoseLife":{"who":"Triggering","amount":1}}"#
)]
#[case::a_target(
    "Whenever ~ or another Goblin you control dies, ~ deals 1 damage to any target.",
    r#"{"DealDamage":{"to":{"Target":0},"amount":1}}"#
)]
fn the_effects_after_the_event_are_read_as_after_any_other_event(
    #[case] text: &str,
    #[case] effect: &str,
) {
    let read = trigger(text).unwrap_or_else(|| panic!("not read: {text}"));
    let Ability::Triggered { effects, .. } = &read else {
        panic!("not a triggered ability: {text}");
    };
    let expected: Effect = serde_json::from_str(effect).unwrap();
    assert!(effects.contains(&expected), "{text}: {effects:?}");
    assert!(
        render_variants(&read)
            .iter()
            .any(|written| without_period(&canonical(written)) == without_period(&canonical(text))),
        "{text}"
    );
}

/// Whole clauses as the cards print them (the name as `~`), with the effects the other templates read.
#[rstest]
#[case::lose_and_gain(
    "Whenever this creature or another creature dies, target player loses 1 life and you gain 1 life."
)]
#[case::each_opponent(
    "Whenever this creature or another creature you control dies, each opponent sacrifices a creature of their choice."
)]
#[case::two_types(
    "Whenever this creature or another creature or planeswalker you control dies, each opponent loses 1 life and you gain 1 life."
)]
#[case::a_mass_effect(
    "Whenever this creature or another enchantment you control enters, creatures your opponents control get -1/-1 until end of turn."
)]
#[case::a_token(
    "Whenever this creature or another nontoken Zombie you control dies, create a 2/2 black Zombie creature token."
)]
#[case::tap_a_creature(
    "Whenever this creature or another Human you control enters, tap target creature an opponent controls."
)]
#[case::damage_from_the_card(
    "Whenever ~ or another Goblin you control dies, ~ deals 1 damage to any target."
)]
#[case::draw_and_lose(
    "Whenever this creature or another Zombie you control dies, you draw a card and lose 1 life."
)]
fn whole_clauses_of_cards_are_read(#[case] text: &str) {
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::when_for_two_subjects("When ~ or another creature you control dies, draw a card.")]
#[case::the_other_first("Whenever another creature you control or ~ dies, draw a card.")]
#[case::without_another("Whenever ~ or a creature you control dies, draw a card.")]
#[case::without_the_noun("Whenever ~ or another you control dies, draw a card.")]
#[case::and_instead_of_or("Whenever ~ and another creature you control die, draw a card.")]
#[case::two_others("Whenever ~ or another creature or another Elf you control dies, draw a card.")]
#[case::a_repeated_verb("Whenever ~ or another creature you control dies or dies, draw a card.")]
#[case::an_unmodelled_second_verb(
    "Whenever ~ or another creature you control dies or is put into exile, draw a card."
)]
#[case::a_count_of_one_event(
    "Whenever ~ or another creature you control attacks for the first time each turn, draw a card."
)]
#[case::from_a_zone(
    "Whenever ~ or another creature you control enters from your graveyard, draw a card."
)]
#[case::a_name_is_not_a_subtype("Whenever ~ or another Commander you control dies, draw a card.")]
#[case::an_unknown_adjective(
    "Whenever ~ or another modified creature you control dies, draw a card."
)]
#[case::a_subtype_and_a_type("Whenever ~ or another Human creature you control dies, draw a card.")]
#[case::paired("Whenever ~ or a creature it's paired with is dealt damage, draw a card.")]
#[case::the_wrong_verb_form("Whenever ~ or another Elf you control enter, draw a card.")]
#[case::no_verb("Whenever ~ or another creature you control, draw a card.")]
#[case::a_plural_group("Whenever ~ or other Elves you control die, draw a card.")]
#[case::an_unread_effect("Whenever ~ or another creature you control dies, goad target creature.")]
#[case::no_effect("Whenever ~ or another creature you control dies,")]
fn near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text:?}");
}

/// Events the renderer has no single wording for are not written, so a card with them is never taken for read.
#[rstest]
#[case::different_verbs(vec![event(Verb::Enters, THIS), event(Verb::Dies, YOUR_CREATURE)])]
#[case::an_odd_count(vec![
    event(Verb::Enters, THIS), event(Verb::Enters, YOUR_CREATURE), event(Verb::Dies, THIS),
])]
#[case::the_other_first(vec![event(Verb::Enters, YOUR_CREATURE), event(Verb::Enters, THIS)])]
#[case::without_other(vec![
    event(Verb::Enters, THIS), event(Verb::Enters, r#"[{"Type":"Creature"}]"#),
])]
#[case::three_subjects(vec![
    event(Verb::Enters, THIS),
    event(Verb::Enters, YOUR_CREATURE),
    event(Verb::Dies, THIS),
    event(Verb::Dies, r#"[{"Type":"Artifact"},"Other"]"#),
])]
fn a_list_of_events_that_is_not_the_card_and_one_other_is_not_written(#[case] events: Vec<String>) {
    let json = format!(
        r#"{{"Triggered":{{"when":{{"AnyOf":[{}]}},"effects":[{{"Draw":{{}}}}]}}}}"#,
        events.join(",")
    );
    assert_eq!(render_variants(&ability(&json)), Vec::<String>::new());
}
