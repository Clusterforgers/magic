//! Goal: the validator names a gap for exactly the shapes the engine refuses at run time: a shield `ShieldShape` does not read (a named object moving other than to a graveyard or off the battlefield, a library shield, a named entry), a payment that loses its source more than once, and mana read from one player's game added for many.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::trigger::ShieldShape;
use mtg_cards::validate::used_primitives;
use rstest::rstest;

/// The primitives of an activated ability with this one effect.
fn uses(effect: &str) -> Vec<String> {
    let ability: Ability = serde_json::from_str(&format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{1}}"}}],"effects":[{effect}]}}}}"#
    ))
    .unwrap();
    used_primitives(&ability).into_iter().collect()
}

#[rstest]
#[case::leaves(r#"{"ZoneChange":{"from":"Battlefield","what":[{"Is":"This"}]}}"#, r#"{"MoveTo":"Exile"}"#, Some(ShieldShape::Named { leaves: true }))]
#[case::dies(r#"{"ZoneChange":{"from":"Battlefield","to":"Graveyard","what":[{"Is":{"Target":0}}]}}"#, r#"{"MoveTo":"Exile"}"#, Some(ShieldShape::Named { leaves: false }))]
#[case::spell(r#"{"ZoneChange":{"from":"Stack","to":"Graveyard","what":[{"Is":{"Bound":"cast"}}]}}"#, r#"{"MoveTo":"Exile"}"#, Some(ShieldShape::Named { leaves: false }))]
#[case::named_to_hand(
    r#"{"ZoneChange":{"from":"Battlefield","to":"Hand","what":[{"Is":"This"}]}}"#,
    r#"{"MoveTo":"Exile"}"#,
    None
)]
#[case::named_from_graveyard(
    r#"{"ZoneChange":{"from":"Graveyard","what":[{"Is":"This"}]}}"#,
    r#"{"MoveTo":"Exile"}"#,
    None
)]
#[case::named_leaves_the_stack(
    r#"{"ZoneChange":{"from":"Stack","what":[{"Is":"This"}]}}"#,
    r#"{"MoveTo":"Exile"}"#,
    None
)]
#[case::class(
    r#"{"ZoneChange":{"from":"Battlefield","to":"Graveyard","what":[{"Type":"Creature"}]}}"#,
    r#"{"MoveTo":"Exile"}"#,
    Some(ShieldShape::Class)
)]
#[case::class_into_library(
    r#"{"ZoneChange":{"to":"Graveyard","what":[{"Type":"Creature"}]}}"#,
    r#"{"IntoLibrary":{"place":"Bottom"}}"#,
    None
)]
#[case::countered(
    r#"{"Countered":{"what":[{"Is":"This"}]}}"#,
    r#"{"MoveTo":"Hand"}"#,
    Some(ShieldShape::Countered)
)]
#[case::countered_into_library(
    r#"{"Countered":{"what":[{"Is":"This"}]}}"#,
    r#"{"IntoLibrary":{"place":"Top"}}"#,
    None
)]
#[case::entries(
    r#"{"Enters":{"what":[{"Type":"Creature"}]}}"#,
    r#""Tapped""#,
    Some(ShieldShape::Class)
)]
#[case::named_entry(r#"{"Enters":{"what":[{"Is":"This"}]}}"#, r#""Tapped""#, None)]
#[case::damage(
    r#"{"Damage":{"to":"You"}}"#,
    r#""Prevent""#,
    Some(ShieldShape::Damage)
)]
fn fix19_a_shield_is_a_gap_exactly_when_the_engine_has_no_shape_for_it(
    #[case] when: &str,
    #[case] by: &str,
    #[case] shape: Option<ShieldShape>,
) {
    let effect = format!(r#"{{"Shield":{{"when":{when},"by":{by},"duration":"UntilEndOfTurn"}}}}"#);
    let Effect::Shield { when, by, .. } = serde_json::from_str(&effect).unwrap() else {
        panic!("a shield");
    };
    assert_eq!(ShieldShape::of(&when, &by), shape);
    let gap = uses(&effect)
        .iter()
        .any(|name| name.starts_with("Effect.Shield."));
    assert_eq!(gap, shape.is_none(), "{effect}");
}

#[rstest]
#[case::sacrifice_twice(r#"{"Pay":{"cost":["SacrificeSelf"],"times":2}}"#, true)]
#[case::exile_x_times(r#"{"Pay":{"cost":["ExileSelf"],"times":"X"}}"#, true)]
#[case::sacrifice_once(r#"{"Pay":{"cost":["SacrificeSelf"]}}"#, false)]
#[case::mana_twice(r#"{"Pay":{"cost":[{"Mana":"{1}"}],"times":2}}"#, false)]
fn fix19_a_payment_that_loses_its_source_more_than_once_is_a_gap(
    #[case] effect: &str,
    #[case] gap: bool,
) {
    let named = uses(effect).contains(&"Effect.Pay.Times.SourceLeaves".to_owned());
    assert_eq!(named, gap, "{effect}");
}

#[rstest]
#[case::identity_for_each(
    r#"{"AddMana":{"who":"EachPlayer","mana":{"CommanderIdentity":1}}}"#,
    Some("Effect.AddMana.CommanderIdentity.Players")
)]
#[case::identity_for_you(r#"{"AddMana":{"mana":{"CommanderIdentity":1}}}"#, None)]
#[case::lands_for_opponents(r#"{"AddMana":{"who":"Opponents","mana":{"ProducedBy":{"among":[{"Type":"Land"}],"count":1}}}}"#, Some("Effect.AddMana.ProducedBy.Players"))]
#[case::fixed_for_each(r#"{"AddMana":{"who":"EachPlayer","mana":{"Fixed":["Green"]}}}"#, None)]
fn fix19_mana_read_from_one_game_for_many_players_is_a_gap(
    #[case] effect: &str,
    #[case] gap: Option<&str>,
) {
    let named: Vec<String> = uses(effect)
        .into_iter()
        .filter(|name| name.ends_with(".Players"))
        .collect();
    assert_eq!(
        named,
        gap.into_iter().map(str::to_owned).collect::<Vec<_>>(),
        "{effect}"
    );
}
