//! Goal: the hand ability "until this card is cast from exile" with its permission, the spells that count "as you cast
//! this spell" and a trigger whose one effect lasts until your next turn read back as the card prints them; another zone,
//! duration, cost, target, count or amount never reads back.
use rstest::rstest;

use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const HIDE: &str = r#"{"Activated":{"cost":[{"Mana":"{2}"},"ExileSelf"],"from":"Hand","targets":[{"what":{"Object":[{"Type":"Land"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilCastFromExile","mods":[{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"OneOf":["Blue","Black","Red"]}}}]}}}]}},{"Rule":{"duration":"Forever","rule":{"MayCast":{"from":"Exile","only":{"Arrived":{"of":"This","zone":"Exile"}}}}}}]}}"#;

const HIDE_CLAUSE: &str = "{2}, Exile ~ from your hand: Target land gains \"{T}: Add {U}, {B}, or {R}\" until ~ is cast from exile. You may cast ~ for as long as it remains exiled.";

#[rstest]
#[case::named(HIDE_CLAUSE.to_owned())]
#[case::this_card(HIDE_CLAUSE.replace('~', "this card"))]
fn the_hand_ability_reads_back(#[case] clause: String) {
    assert!(written(HIDE, &clause), "{clause}");
}

#[rstest]
#[case::another_mana(r#"{"Mana":"{2}"}"#, r#"{"Mana":"{3}"}"#)]
#[case::without_the_exile(r#","ExileSelf""#, "")]
#[case::from_the_battlefield(r#""from":"Hand","#, "")]
#[case::a_creature_target(r#"{"Type":"Land"}"#, r#"{"Type":"Creature"}"#)]
#[case::for_the_turn(r#""UntilCastFromExile""#, r#""UntilEndOfTurn""#)]
#[case::from_the_graveyard(r#"{"from":"Exile","only""#, r#"{"from":"Graveyard","only""#)]
#[case::for_the_turn_only(r#""duration":"Forever""#, r#""duration":"UntilEndOfTurn""#)]
#[case::another_color(r#""Red""#, r#""Green""#)]
fn another_hand_ability_differs(#[case] from: &str, #[case] to: &str) {
    let json = HIDE.replacen(from, to, 1);
    assert_ne!(json, HIDE, "{from}");
    assert!(!written(&json, HIDE_CLAUSE), "{from}");
}

const STEER: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"AnyOf":["Attacking","Blocking"]}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"When":{"condition":{"Compare":{"left":{"CountedAsCast":{"Count":[{"Subtype":"Mount"},{"ControlledBy":"You"}]}},"op":"Ge","right":1}},"then":4,"otherwise":2}}}}]}}"#;
const STEER_CLAUSE: &str = "~ deals 2 damage to target attacking or blocking creature. ~ deals 4 damage to that creature instead if you controlled a Mount as you cast this spell.";
const BLESSING: &str = r#"{"Spell":{"effects":[{"Draw":{"count":{"CountedAsCast":{"Greatest":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Power"}}}}},{"GainLife":{"amount":{"Product":[{"CountedAsCast":{"Greatest":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Power"}}},{"When":{"condition":{"Matches":{"selector":[{"WasCast":{"from":"Exile"}}],"subject":"This"}},"then":2,"otherwise":0}}]}}}]}}"#;
const BLESSING_CLAUSE: &str = "Draw X cards, where X is the greatest power among creatures you controlled as you cast this spell. If this spell was cast from exile, you gain twice X life.";

#[rstest]
#[case::steer_clear(STEER, STEER_CLAUSE)]
#[case::lifestreams_blessing(BLESSING, BLESSING_CLAUSE)]
fn a_count_as_you_cast_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::steer_counted_now(
    STEER,
    STEER_CLAUSE,
    r#"{"CountedAsCast":{"Count":[{"Subtype":"Mount"},{"ControlledBy":"You"}]}}"#,
    r#"{"Count":[{"Subtype":"Mount"},{"ControlledBy":"You"}]}"#
)]
#[case::steer_another_subtype(STEER, STEER_CLAUSE, "Mount", "Horse")]
#[case::steer_more_damage(STEER, STEER_CLAUSE, r#""then":4"#, r#""then":5"#)]
#[case::steer_an_opponents(
    STEER,
    STEER_CLAUSE,
    r#"{"ControlledBy":"You"}"#,
    r#"{"ControlledBy":"Opponents"}"#
)]
#[case::blessing_toughness(
    BLESSING,
    BLESSING_CLAUSE,
    r#""stat":"Power""#,
    r#""stat":"Toughness""#
)]
#[case::blessing_from_the_graveyard(
    BLESSING,
    BLESSING_CLAUSE,
    r#""from":"Exile""#,
    r#""from":"Graveyard""#
)]
#[case::blessing_thrice(BLESSING, BLESSING_CLAUSE, r#""then":2"#, r#""then":3"#)]
fn another_count_differs(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] from: &str,
    #[case] to: &str,
) {
    let near = json.replacen(from, to, 1);
    assert_ne!(near, json, "{from}");
    assert!(!written(&near, clause), "{from}");
}

const ADJUDICATE: &str = r#"{"Triggered":{"when":{"ZoneChange":{"to":"Battlefield","what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilYourNextTurn","mods":["CantAttack","CantBlock"]}}]}}"#;
const ADJUDICATE_CLAUSE: &str = "When this creature enters, target creature an opponent controls can't attack or block until your next turn.";

#[test]
fn a_trigger_lasting_until_your_next_turn_reads_back_with_the_duration_last() {
    assert!(written(ADJUDICATE, ADJUDICATE_CLAUSE));
}

#[rstest]
#[case::this_turn(r#""UntilYourNextTurn""#, r#""UntilEndOfTurn""#)]
#[case::attack_only(r#"["CantAttack","CantBlock"]"#, r#"["CantAttack"]"#)]
#[case::your_creature(r#"{"ControlledBy":"Opponents"}"#, r#"{"ControlledBy":"You"}"#)]
fn another_trigger_lasting_until_your_next_turn_differs(#[case] from: &str, #[case] to: &str) {
    let near = ADJUDICATE.replacen(from, to, 1);
    assert_ne!(near, ADJUDICATE, "{from}");
    assert!(!written(&near, ADJUDICATE_CLAUSE), "{from}");
}
