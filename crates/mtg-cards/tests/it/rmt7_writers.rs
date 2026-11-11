//! Goal: the writers rmt-7 adds write the card's words and nothing near them: a filter atom "named Wood" in a cost, the kinds of keyword a change takes away ("loses all landwalk abilities", "lose hexproof, indestructible, protection, shroud, and ward"), a loyalty ability given in quotes with its cost in brackets, an emblem that quotes an ability in single quotes, "if you don't control a creature named Mowu", a cost change "during your turn", a block capacity "for each Equipment attached to this creature", and an ability any player may activate that takes away a printed ability in quotes, "can't attack during extra turns", and "Enchanted player can't activate abilities that aren't mana abilities or loyalty abilities".
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_cards::compile::{same_written_text, written_variants};
use rstest::rstest;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

fn writes(json: &str, text: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|written| same_written_text(written, text))
}

const WOOD: &str = "Sacrifice a token named Wood: Add {R}.";

#[rstest]
#[case::a_token_named(r#"[{"Sacrifice":{"filter":["Token",{"Named":"Wood"}]}}]"#, WOOD, true)]
#[case::another_name(
    r#"[{"Sacrifice":{"filter":["Token",{"Named":"Tree"}]}}]"#,
    WOOD,
    false
)]
#[case::not_only_tokens(r#"[{"Sacrifice":{"filter":[{"Named":"Wood"}]}}]"#, WOOD, false)]
#[case::any_token(r#"[{"Sacrifice":{"filter":["Token"]}}]"#, WOOD, false)]
fn rmt7_a_named_atom_in_a_cost(#[case] cost: &str, #[case] text: &str, #[case] reads: bool) {
    let json = format!(
        r#"{{"Activated":{{"cost":{cost},"effects":[{{"AddMana":{{"mana":{{"Fixed":["Red"]}}}}}}]}}}}"#
    );
    assert_eq!(writes(&json, text), reads);
}

const HAMMERHEIM: &str = "{T}: Target creature loses all landwalk abilities until end of turn.";
const SHAY: &str = "{1}: Permanents your opponents control lose hexproof, indestructible, protection, shroud, and ward until end of turn.";

fn on_target(mods: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":["Tap"],"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{"Continuous":{{"affected":{{"Target":0}},"mods":[{mods}],"duration":"UntilEndOfTurn"}}}}]}}}}"#
    )
}

fn on_opponents(mods: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":"{{1}}","effects":[{{"Continuous":{{"affected":{{"Each":[{{"ControlledBy":"Opponents"}}]}},"mods":[{mods}],"duration":"UntilEndOfTurn"}}}}]}}}}"#
    )
}

const FIVE: &str = r#"{"RemoveKeyword":"Hexproof"},{"RemoveKeyword":"Indestructible"},{"RemoveKeywordKind":"Protection"},{"RemoveKeyword":"Shroud"},{"RemoveKeywordKind":"Ward"}"#;

#[rstest]
#[case::landwalk(on_target(r#"{"RemoveKeywordKind":"Landwalk"}"#), HAMMERHEIM, true)]
#[case::one_landwalk_is_not_all(
    on_target(r#"{"RemoveKeyword":{"Landwalk":[{"Subtype":"Island"}]}}"#),
    HAMMERHEIM,
    false
)]
#[case::ward_is_no_landwalk(on_target(r#"{"RemoveKeywordKind":"Ward"}"#), HAMMERHEIM, false)]
#[case::five(on_opponents(FIVE), SHAY, true)]
#[case::four(
    on_opponents(r#"{"RemoveKeyword":"Hexproof"},{"RemoveKeyword":"Indestructible"},{"RemoveKeywordKind":"Protection"},{"RemoveKeyword":"Shroud"}"#),
    SHAY,
    false
)]
#[case::one_ward_is_not_every_ward(
    on_opponents(r#"{"RemoveKeyword":"Hexproof"},{"RemoveKeyword":"Indestructible"},{"RemoveKeywordKind":"Protection"},{"RemoveKeyword":"Shroud"},{"RemoveKeyword":{"Ward":"{2}"}}"#),
    SHAY,
    false
)]
fn rmt7_kinds_of_keywords_taken_away(
    #[case] json: String,
    #[case] text: &str,
    #[case] reads: bool,
) {
    assert_eq!(writes(&json, text), reads);
}

#[test]
fn rmt7_plain_keywords_taken_away_keep_their_words() {
    let json = on_target(r#"{"RemoveKeyword":"Flying"},{"RemoveKeyword":"Trample"}"#);
    assert!(writes(
        &json,
        "{T}: Target creature loses flying and trample until end of turn."
    ));
}

const KIORA: &str = r#"Planeswalkers you control have "[−8]: Create an 8/8 blue Leviathan creature token with hexproof.""#;
const LEVIATHAN: &str = r#"{"CreateToken":{"token":{"Custom":{"name":"Leviathan","colors":["Blue"],"types":["Creature"],"subtypes":["Leviathan"],"power":8,"toughness":8,"abilities":[{"Keyword":"Hexproof"}]}}}}"#;

fn given(ability: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":{{"Each":[{{"Type":"Planeswalker"}},{{"ControlledBy":"You"}}]}},"mods":[{{"GrantAbility":{ability}}}]}}}}"#
    )
}

#[rstest]
#[case::loyalty(given(&format!(r#"{{"Loyalty":{{"cost":{{"Minus":8}},"effects":[{LEVIATHAN}]}}}}"#)), true)]
#[case::another_cost(given(&format!(r#"{{"Loyalty":{{"cost":{{"Minus":7}},"effects":[{LEVIATHAN}]}}}}"#)), false)]
#[case::an_activated_ability(
    given(&format!(r#"{{"Activated":{{"cost":"{{8}}","effects":[{LEVIATHAN}]}}}}"#)),
    false
)]
fn rmt7_a_loyalty_ability_given_has_its_cost_in_brackets(
    #[case] json: String,
    #[case] reads: bool,
) {
    assert_eq!(writes(&json, KIORA), reads);
}

const LILIANA: &str =
    r#"−6: You get an emblem with "Swamps you control have '{T}: Add {B}{B}{B}{B}.'""#;

fn emblem(mana: &str) -> String {
    format!(
        r#"{{"Loyalty":{{"cost":{{"Minus":6}},"effects":[{{"CreateEmblem":{{"abilities":[{{"Static":{{"affected":{{"Each":[{{"Subtype":"Swamp"}},{{"ControlledBy":"You"}}]}},"mods":[{{"GrantAbility":{{"Activated":{{"cost":["Tap"],"effects":[{{"AddMana":{{"mana":{{"Fixed":{mana}}}}}}}]}}}}}}]}}}}]}}}}]}}}}"#
    )
}

#[rstest]
#[case::four(emblem(r#"["Black","Black","Black","Black"]"#), LILIANA, true)]
#[case::three(emblem(r#"["Black","Black","Black"]"#), LILIANA, false)]
#[case::double_quotes_inside(
    emblem(r#"["Black","Black","Black","Black"]"#),
    r#"−6: You get an emblem with "Swamps you control have "{T}: Add {B}{B}{B}{B}."""#,
    false
)]
fn rmt7_an_emblem_quotes_an_ability_in_single_quotes(
    #[case] json: String,
    #[case] text: &str,
    #[case] reads: bool,
) {
    assert_eq!(writes(&json, text), reads);
}

const JIANG: &str = "−1: If you don't control a creature named Mowu, create Mowu, a legendary 3/3 green Dog creature token.";

fn unless(condition: &str) -> String {
    format!(
        r#"{{"Loyalty":{{"cost":{{"Minus":1}},"effects":[{{"If":{{"condition":{condition},"then":[{{"CreateToken":{{"token":{{"Custom":{{"name":"Mowu","supertypes":["Legendary"],"colors":["Green"],"types":["Creature"],"subtypes":["Dog"],"power":3,"toughness":3}}}}}}}}]}}}}]}}}}"#
    )
}

#[rstest]
#[case::none(r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"Named":"Mowu"}]},"op":"Lt","right":1}}"#, true)]
#[case::not_one(r#"{"Not":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"Named":"Mowu"}]},"op":"Ge","right":1}}}"#, true)]
#[case::one(r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"Named":"Mowu"}]},"op":"Ge","right":1}}"#, false)]
#[case::an_opponents(r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"Opponents"},{"Named":"Mowu"}]},"op":"Lt","right":1}}"#, false)]
#[case::fewer_than_two(r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"Named":"Mowu"}]},"op":"Lt","right":2}}"#, false)]
fn rmt7_if_you_dont_control_a_named_creature(#[case] condition: &str, #[case] reads: bool) {
    assert_eq!(writes(&unless(condition), JIANG), reads);
}

const HYLDA: &str =
    "{1}, {T}: Tap target creature. This ability costs {1} less to activate during your turn.";

fn hylda(condition: &str, mana: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{1}}"}},"Tap",{{"Changed":{{"change":"Reduce","mana":"{mana}","condition":{condition}}}}}],"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{"Tap":{{"what":{{"Target":0}}}}}}]}}}}"#
    )
}

#[rstest]
#[case::your_turn(hylda(r#""YourTurn""#, "{1}"), true)]
#[case::two_less(hylda(r#""YourTurn""#, "{2}"), false)]
#[case::not_your_turn(hylda(r#"{"Not":"YourTurn"}"#, "{1}"), false)]
fn rmt7_an_ability_costs_less_during_your_turn(#[case] json: String, #[case] reads: bool) {
    assert_eq!(writes(&json, HYLDA), reads);
}

const KEMBA: &str = "This creature can block an additional creature each combat for each Equipment attached to this creature.";

fn kemba(per: &str) -> String {
    format!(r#"{{"Static":{{"affected":"This","mods":[{{"BlocksAdditionalPer":{per}}}]}}}}"#)
}

#[rstest]
#[case::equipment(
    kemba(r#"{"Count":[{"Subtype":"Equipment"},{"AttachedTo":"This"}]}"#),
    true
)]
#[case::auras(
    kemba(r#"{"Count":[{"Subtype":"Aura"},{"AttachedTo":"This"}]}"#),
    false
)]
#[case::every_equipment(kemba(r#"{"Count":[{"Subtype":"Equipment"}]}"#), false)]
fn rmt7_a_block_capacity_counted(#[case] json: String, #[case] reads: bool) {
    assert_eq!(writes(&json, KEMBA), reads);
}

const LION: &str = r#"{3}: Until end of turn, this creature loses "Prevent all damage that would be dealt to this creature." Any player may activate this ability."#;

fn lion(by: &str, lost: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":"{{3}}","by":"{by}","effects":[{{"Continuous":{{"affected":"This","mods":[{{"LosePrinted":{lost}}}],"duration":"UntilEndOfTurn"}}}}]}}}}"#
    )
}

const PREVENT: &str = r#"{"Replacement":{"when":{"Damage":{"to":"This"}},"by":"Prevent"}}"#;

#[rstest]
#[case::any_player(lion("Anyone", PREVENT), true)]
#[case::its_controller(lion("Controller", PREVENT), false)]
#[case::an_opponent(lion("Opponents", PREVENT), false)]
#[case::another_ability(lion("Anyone", r#"{"Keyword":"Flying"}"#), false)]
fn rmt7_any_player_takes_away_a_printed_ability(#[case] json: String, #[case] reads: bool) {
    assert_eq!(writes(&json, LION), reads);
}

const MEDOMAI: &str = "This creature can't attack during extra turns.";

#[rstest]
#[case::extra_turns(
    r#"{"Static":{"affected":"This","mods":["CantAttack"],"condition":"ExtraTurn"}}"#,
    true
)]
#[case::your_turn(
    r#"{"Static":{"affected":"This","mods":["CantAttack"],"condition":"YourTurn"}}"#,
    false
)]
#[case::always(r#"{"Static":{"affected":"This","mods":["CantAttack"]}}"#, false)]
#[case::cant_block(
    r#"{"Static":{"affected":"This","mods":["CantBlock"],"condition":"ExtraTurn"}}"#,
    false
)]
fn rmt7_no_attack_during_extra_turns(#[case] json: &str, #[case] reads: bool) {
    assert_eq!(writes(json, MEDOMAI), reads);
}

const SPLENDOR: &str =
    "Enchanted player can't activate abilities that aren't mana abilities or loyalty abilities.";

fn cant(who: &str, kind: &str) -> String {
    format!(
        r#"{{"Rule":{{"rule":{{"Cant":{{"who":"{who}","action":{{"ActivateKind":{{"kind":"{kind}"}}}}}}}}}}}}"#
    )
}

#[rstest]
#[case::enchanted_player(cant("AttachedPlayer", "AnyButLoyalty"), true)]
#[case::any_but_mana(cant("AttachedPlayer", "Any"), false)]
#[case::opponents(cant("Opponents", "AnyButLoyalty"), false)]
fn rmt7_the_enchanted_player_keeps_mana_and_loyalty_abilities(
    #[case] json: String,
    #[case] reads: bool,
) {
    assert_eq!(writes(&json, SPLENDOR), reads);
}
