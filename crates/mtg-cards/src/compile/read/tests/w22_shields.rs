//! Goal: the w22 writers give the oracle wording of a shield for the turn that prevents all damage, or all combat damage, dealt to players and permanents, by a kind of source, or both, and of the next damage to one target; another recipient, source, kind of damage or amount never reads back as the clause.
use super::*;

/// The spell with one or more shields of all damage.
fn spell(shields: &[String]) -> String {
    format!(r#"{{"Spell":{{"effects":[{}]}}}}"#, shields.join(","))
}

/// A shield that prevents the damage matched by `damage` (the fields of the `Damage` event).
fn shield(damage: &str) -> String {
    format!(
        r#"{{"Shield":{{"by":"Prevent","duration":"UntilEndOfTurn","when":{{"Damage":{{{damage}}}}}}}}}"#
    )
}

fn reads(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

#[rstest]
#[case::creatures(
    r#""to":{"Each":[{"Type":"Creature"}]}"#,
    "Prevent all damage that would be dealt to creatures this turn."
)]
#[case::your_creatures(
    r#""to":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#,
    "Prevent all damage that would be dealt to creatures you control this turn."
)]
#[case::your_creatures_turn_first(
    r#""to":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#,
    "Prevent all damage that would be dealt this turn to creatures you control."
)]
#[case::players_combat(
    r#""combat":true,"to":"EachPlayer""#,
    "Prevent all combat damage that would be dealt to players this turn."
)]
#[case::creatures_to_players(
    r#""source":[{"Type":"Creature"},{"In":"Battlefield"}],"to":"EachPlayer""#,
    "Prevent all damage that creatures would deal to players this turn."
)]
#[case::attacking_to_you(
    r#""source":[{"Type":"Creature"},"Attacking",{"Not":{"HasKeyword":"Flying"}}],"to":"You""#,
    "Prevent all damage that would be dealt to you this turn by attacking creatures without flying."
)]
#[case::by_creatures(
    r#""source":[{"Type":"Creature"},{"In":"Battlefield"}]"#,
    "Prevent all damage that would be dealt by creatures this turn."
)]
#[case::by_opponents_creatures(
    r#""source":[{"Type":"Creature"},{"ControlledBy":"Opponents"},{"In":"Battlefield"}]"#,
    "Prevent all damage that would be dealt this turn by creatures your opponents control."
)]
#[case::colors(
    r#""source":[{"AnyOf":[{"Color":"Black"},{"Color":"Red"}]}]"#,
    "Prevent all damage that black sources and red sources would deal this turn."
)]
fn w22_a_shield_of_all_damage_reads_as_printed(#[case] damage: &str, #[case] clause: &str) {
    assert!(reads(&spell(&[shield(damage)]), clause), "{clause}");
}

#[rstest]
#[case::you_and_creatures(
    &["\"to\":\"You\"", "\"to\":{\"Each\":[{\"Type\":\"Creature\"},{\"ControlledBy\":\"You\"}]}"],
    "Prevent all damage that would be dealt to you and creatures you control this turn."
)]
#[case::you_and_permanents(
    &["\"to\":\"You\"", "\"to\":{\"Each\":[{\"ControlledBy\":\"You\"}]}"],
    "Prevent all damage that would be dealt to you and permanents you control this turn."
)]
#[case::combat_you_and_planeswalkers(
    &["\"combat\":true,\"to\":\"You\"", "\"combat\":true,\"to\":{\"Each\":[{\"Type\":\"Planeswalker\"},{\"ControlledBy\":\"You\"}]}"],
    "Prevent all combat damage that would be dealt to you and planeswalkers you control this turn."
)]
fn w22_shields_of_two_recipients_are_one_sentence(#[case] damage: &[&str], #[case] clause: &str) {
    let shields: Vec<String> = damage.iter().map(|damage| shield(damage)).collect();
    assert!(reads(&spell(&shields), clause), "{clause}");
}

#[rstest]
#[case::combat_for_all(
    r#""to":{"Each":[{"Type":"Creature"}]}"#,
    "Prevent all combat damage that would be dealt to creatures this turn."
)]
#[case::other_recipient(
    r#""to":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}"#,
    "Prevent all damage that would be dealt to creatures you control this turn."
)]
#[case::players_for_you(
    r#""to":"You""#,
    "Prevent all damage that would be dealt to players this turn."
)]
#[case::noncombat_for_all(
    r#""combat":false,"to":"EachPlayer""#,
    "Prevent all combat damage that would be dealt to players this turn."
)]
#[case::other_source(
    r#""source":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]"#,
    "Prevent all damage that would be dealt this turn by creatures you control."
)]
#[case::one_color_for_two(
    r#""source":[{"AnyOf":[{"Color":"Black"},{"Color":"Red"}]}]"#,
    "Prevent all damage that black sources would deal this turn."
)]
#[case::source_dropped(
    r#""to":"EachPlayer""#,
    "Prevent all damage that creatures would deal to players this turn."
)]
fn w22_another_shield_never_reads_as_the_clause(#[case] damage: &str, #[case] clause: &str) {
    assert!(!reads(&spell(&[shield(damage)]), clause), "{clause}");
}

#[test]
fn w22_a_shield_for_one_recipient_does_not_read_as_two() {
    let one = spell(&[shield(r#""to":"You""#)]);
    assert!(!reads(
        &one,
        "Prevent all damage that would be dealt to you and creatures you control this turn."
    ));
    let mixed = spell(&[
        shield(r#""to":"You""#),
        shield(r#""combat":true,"to":{"Each":[{"Type":"Creature"}]}"#),
    ]);
    assert!(!reads(
        &mixed,
        "Prevent all damage that would be dealt to you and creatures this turn."
    ));
}

#[test]
fn w22_a_shield_that_gains_life_names_the_damage_prevented() {
    let json = r#"{"Spell":{"effects":[{"Shield":{"by":{"PreventAnd":[{"GainLife":{"amount":"Triggering"}}]},"duration":"UntilEndOfTurn","when":{"Damage":{"source":[{"Type":"Creature"},{"In":"Battlefield"}]}}}}]}}"#;
    let clause = "Prevent all damage that would be dealt by creatures this turn. You gain life equal to the damage prevented this way.";
    assert!(reads(json, clause));
    let opponent = json.replace("{\"GainLife\":{", "{\"GainLife\":{\"who\":\"Opponents\",");
    assert!(!reads(&opponent, clause));
    let other = json.replace(
        "{\"GainLife\":{\"amount\":\"Triggering\"}}",
        "{\"GainLife\":{\"amount\":3}}",
    );
    assert!(!reads(&other, clause));
}

const NEXT: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"Shield":{"by":{"PreventUpTo":1},"duration":"UntilEndOfTurn","when":{"Damage":{"to":{"Target":0}}}}}],"targets":[{"what":"AnyTarget"}]}}"#;

#[rstest]
#[case::as_printed("{\"PreventUpTo\":1}", "any target", true)]
#[case::other_amount("{\"PreventUpTo\":2}", "any target", false)]
fn w22_the_next_damage_to_any_target(#[case] by: &str, #[case] target: &str, #[case] same: bool) {
    let json = NEXT.replace("{\"PreventUpTo\":1}", by);
    let clause =
        format!("{{T}}: Prevent the next 1 damage that would be dealt to {target} this turn.");
    assert_eq!(reads(&json, &clause), same);
}

#[rstest]
#[case::chosen_type(
    r#""source":["ChosenSubtype",{"In":"Battlefield"}],"to":"You""#,
    "{1}{W}: The next time a creature of the chosen type would deal damage to you this turn, prevent that damage.",
    true
)]
#[case::other_player(
    r#""source":["ChosenSubtype",{"In":"Battlefield"}],"to":"Opponents""#,
    "{1}{W}: The next time a creature of the chosen type would deal damage to you this turn, prevent that damage.",
    false
)]
#[case::source_itself(
    r#""source":[{"Is":"This"}],"to":"You""#,
    "{1}{W}: The next time this creature would deal damage to you this turn, prevent that damage.",
    true
)]
#[case::combat_only(
    r#""combat":true,"source":[{"Is":"This"}],"to":"You""#,
    "{1}{W}: The next time this creature would deal damage to you this turn, prevent that damage.",
    false
)]
fn w22_the_next_time_a_source_deals_damage(
    #[case] damage: &str,
    #[case] clause: &str,
    #[case] same: bool,
) {
    let json = format!(
        r#"{{"Activated":{{"cost":"{{1}}{{W}}","effects":[{{"Shield":{{"by":"PreventNext","duration":"UntilEndOfTurn","when":{{"Damage":{{{damage}}}}}}}}}]}}}}"#
    );
    assert_eq!(reads(&json, clause), same, "{clause}");
}

#[rstest]
#[case::as_printed(true)]
#[case::other_group(false)]
fn w22_untap_and_goad_them(#[case] same: bool) {
    let json = r#"{"Spell":{"effects":[{"Untap":{"what":{"Each":[{"Type":"Creature"},"Attacking"]}}},{"Goad":{"what":{"Each":[{"Type":"Creature"},"Attacking"]}}},{"AddPhase":{"phases":["Combat"]}}]}}"#;
    let json = if same {
        json.to_owned()
    } else {
        json.replacen(
            r#""Goad":{"what":{"Each":[{"Type":"Creature"},"Attacking"]}}"#,
            r#""Goad":{"what":{"Each":[{"Type":"Creature"}]}}"#,
            1,
        )
    };
    let clause = "Untap all attacking creatures and goad them. After this phase, there is an additional combat phase.";
    assert_eq!(reads(&json, clause), same);
}
