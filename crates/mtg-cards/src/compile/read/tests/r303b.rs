//! Goal: the r303b writers give the oracle wording of an opening-hand reveal, a redirection of a counted total, the rules that deal damage as wither or let sick creatures attack your opponents, a blocking requirement on Walls, the loss of all abilities but mana abilities, a creature type added by choice, absorb, combat damage prevented from a kind of source and the target "creature, planeswalker, or player", and a rule of another meaning (another step, number, player, subtype or damage) never reads back as the clause.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const REVEAL: &str = "You may reveal this card from your opening hand. If you do, at the beginning of the first upkeep, your life total becomes 26.";

#[rstest]
#[case::first_upkeep(r#"{"OpeningReveal":{"when":{"StepBegins":{"step":"Upkeep"}},"effects":[{"SetLife":{"amount":26}}]}}"#, true)]
#[case::other_life(r#"{"OpeningReveal":{"when":{"StepBegins":{"step":"Upkeep"}},"effects":[{"SetLife":{"amount":20}}]}}"#, false)]
#[case::your_upkeep(r#"{"OpeningReveal":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"SetLife":{"amount":26}}]}}"#, false)]
#[case::end_step(r#"{"OpeningReveal":{"when":{"StepBegins":{"step":"End"}},"effects":[{"SetLife":{"amount":26}}]}}"#, false)]
fn r303b_an_opening_reveal_reads_back_only_as_itself(#[case] json: &str, #[case] same: bool) {
    assert_eq!(written(json, REVEAL), same, "{json}");
}

/// Shining Shoal's spell with the total `most`.
fn shoal(most: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":"AnyTarget"}}],"effects":[{{"Choose":{{"from":{{"Each":[{{"AnyOf":[{{"In":"Battlefield"}},{{"In":"Stack"}},{{"In":"Command"}}]}}]}},"as":"source"}}}},{{"Shield":{{"when":{{"Damage":{{"to":{{"Union":["You",{{"Each":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}]}}]}},"source":[{{"Is":{{"Bound":"source"}}}}]}}}},"by":{{"RedirectCounted":{{"most":{most},"to":{{"Target":0}}}}}},"duration":"UntilEndOfTurn"}}}}]}}}}"#
    )
}

const SHOAL: &str = "The next X damage that a source of your choice would deal to you and/or creatures you control this turn is dealt to any target instead.";

#[rstest]
#[case::x(r#""X""#, true)]
#[case::a_number(r#"2"#, false)]
#[case::another_amount(r#"{"Count":[{"Type":"Creature"}]}"#, false)]
fn r303b_a_counted_redirection_reads_back_with_its_amount(#[case] most: &str, #[case] same: bool) {
    assert_eq!(written(&shoal(most), SHOAL), same, "{most}");
}

#[rstest]
#[case::wither(
    r#"{"Rule":{"rule":"DamageAsWither"}}"#,
    "All damage is dealt as though its source had wither.",
    true
)]
#[case::wither_not_infect(
    r#"{"Rule":{"rule":"DamageAsWither"}}"#,
    "All damage is dealt as though its source had infect.",
    false
)]
#[case::haste(
    r#"{"Rule":{"rule":{"AttackAsThoughHasteAt":"Opponents"}}}"#,
    "All creatures can attack your opponents and planeswalkers your opponents control as though those creatures had haste.",
    true
)]
#[case::haste_for_every_player(
    r#"{"Rule":{"rule":{"AttackAsThoughHasteAt":"EachPlayer"}}}"#,
    "All creatures can attack your opponents and planeswalkers your opponents control as though those creatures had haste.",
    false
)]
#[case::walls(
    r#"{"Rule":{"rule":{"Combat":{"who":"This","quirk":{"MustBeBlockedBy":[{"Subtype":"Wall"}]}}}}}"#,
    "All Walls able to block ~ do so.",
    true
)]
#[case::other_subtype(
    r#"{"Rule":{"rule":{"Combat":{"who":"This","quirk":{"MustBeBlockedBy":[{"Subtype":"Goblin"}]}}}}}"#,
    "All Walls able to block ~ do so.",
    false
)]
#[case::every_creature_is_not_walls(
    r#"{"Rule":{"rule":{"Combat":{"who":"This","quirk":{"MustBeBlocked":"Every"}}}}}"#,
    "All Walls able to block ~ do so.",
    false
)]
#[case::mana_abilities_kept(
    r#"{"Static":{"affected":{"Each":[{"Type":"Land"}]},"mods":["LoseAbilitiesExceptMana"]}}"#,
    "All lands lose all abilities except mana abilities.",
    true
)]
#[case::mana_abilities_lost(
    r#"{"Static":{"affected":{"Each":[{"Type":"Land"}]},"mods":["LoseAbilities"]}}"#,
    "All lands lose all abilities except mana abilities.",
    false
)]
#[case::added_type(
    r#"{"Activated":{"cost":"{1}","effects":[{"ChooseValue":{"kind":"CreatureType","as":"chosen"}},{"Continuous":{"affected":"This","mods":[{"AddCreatureTypeChosen":"chosen"}],"duration":"UntilEndOfTurn"}}]}}"#,
    "{1}: ~ becomes the creature type of your choice in addition to its other types until end of turn.",
    true
)]
#[case::set_type_is_not_added(
    r#"{"Activated":{"cost":"{1}","effects":[{"ChooseValue":{"kind":"CreatureType","as":"chosen"}},{"Continuous":{"affected":"This","mods":[{"SetCreatureTypesChosen":"chosen"}],"duration":"UntilEndOfTurn"}}]}}"#,
    "{1}: ~ becomes the creature type of your choice in addition to its other types until end of turn.",
    false
)]
#[case::equipped(
    r#"{"Replacement":{"when":{"Damage":{"to":"You"}},"by":{"Instead":[{"DealDamage":{"source":"Dealer","to":"Attached","amount":"Triggering"}}]}}}"#,
    "All damage that would be dealt to you is dealt to equipped creature instead.",
    true
)]
fn r303b_rules_read_back_only_as_their_clause(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] same: bool,
) {
    assert_eq!(written(json, clause), same, "{json}");
}

#[rstest]
#[case::absorb("Absorb 2", Some(2))]
#[case::absorb_one("absorb 1", Some(1))]
#[case::no_number("Absorb", None)]
#[case::zero("Absorb 0", None)]
fn r303b_absorb_reads_with_its_number(#[case] line: &str, #[case] amount: Option<u32>) {
    let wanted = amount.map(|amount| crate::keyword::Keyword::Macro {
        name: "absorb".to_owned(),
        amount,
    });
    assert_eq!(crate::compile::keywords::read(line), wanted, "{line}");
}

#[rstest]
#[case::walls(r#"{"Replacement":{"when":{"Damage":{"to":"This","source":[{"Subtype":"Wall"}],"combat":true}},"by":"Prevent"}}"#, true)]
#[case::all_damage(r#"{"Replacement":{"when":{"Damage":{"to":"This","source":[{"Subtype":"Wall"}]}},"by":"Prevent"}}"#, false)]
#[case::other_source(r#"{"Replacement":{"when":{"Damage":{"to":"This","source":[{"Subtype":"Goblin"}],"combat":true}},"by":"Prevent"}}"#, false)]
#[case::other_object(r#"{"Replacement":{"when":{"Damage":{"to":"You","source":[{"Subtype":"Wall"}],"combat":true}},"by":"Prevent"}}"#, false)]
fn r303b_combat_damage_prevented_from_a_kind_of_source(#[case] json: &str, #[case] same: bool) {
    assert_eq!(
        written(
            json,
            "Prevent all combat damage that would be dealt to ~ by Walls."
        ),
        same,
        "{json}"
    );
}

/// Captain's Maneuver with the kinds of its first target.
fn maneuver(first: &str) -> String {
    let three = r#"{"Either":[{"Object":[{"Type":"Creature"}]},{"Object":[{"Type":"Planeswalker"}]},"Player"]}"#;
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{first}}},{{"what":{{"Another":{{"what":{three},"earlier":[0]}}}}}}],"effects":[{{"Shield":{{"when":{{"Damage":{{"to":{{"Target":0}}}}}},"by":{{"RedirectCounted":{{"most":"X","to":{{"Target":1}}}}}},"duration":"UntilEndOfTurn"}}}}]}}}}"#
    )
}

const MANEUVER: &str = "The next X damage that would be dealt to target creature, planeswalker, or player this turn is dealt to another target creature, planeswalker, or player instead.";

#[rstest]
#[case::three_kinds(r#"{"Either":[{"Object":[{"Type":"Creature"}]},{"Object":[{"Type":"Planeswalker"}]},"Player"]}"#, true)]
#[case::any_target(r#""AnyTarget""#, false)]
#[case::no_planeswalker(r#"{"Either":[{"Object":[{"Type":"Creature"}]},"Player"]}"#, false)]
#[case::opponent(r#"{"Either":[{"Object":[{"Type":"Creature"}]},{"Object":[{"Type":"Planeswalker"}]},"Opponent"]}"#, false)]
fn r303b_a_creature_planeswalker_or_player_is_its_own_target(
    #[case] first: &str,
    #[case] same: bool,
) {
    let wordings = written_variants(&ability(&maneuver(first)));
    let found = wordings
        .iter()
        .any(|wording| same_written_text(wording, MANEUVER));
    assert_eq!(found, same, "{first}: {wordings:?}");
}
