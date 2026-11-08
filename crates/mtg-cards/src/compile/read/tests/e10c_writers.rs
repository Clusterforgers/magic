//! Goal: a rule, replacement or cost modifier a permanent is given in quotes, two quoted grants, the untap rule of the other players' untap steps and the move from the battlefield to exile "instead of putting it anywhere else" are written as cards word them, and another player, zone, quirk, subject or rule is not their text.
use super::*;

/// Whether one of the wordings the checker accepts for the ability is the text.
fn writes(json: &str, text: &str) -> bool {
    super::super::render::written_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, text))
}

const UNTAP_THIS: &str =
    r#"{"Rule":{"rule":{"UntapDuring":{"what":["This"],"whose":"Opponents"}}}}"#;
const UNBLOCKED: &str =
    r#"{"Rule":{"rule":{"Combat":{"quirk":"AssignsAsUnblocked","who":"This"}}}}"#;
const LOOK: &str = r#"{"Rule":{"rule":{"LookAtTopAnyTime":"You"}}}"#;
const EXPLORES: &str = r#"{"Triggered":{"effects":[{"Explore":{"what":"This"}}],"when":{"Attacks":{"what":["This"]}}}}"#;

fn granted(affected: &str, abilities: &[&str]) -> String {
    let mods: Vec<String> = abilities
        .iter()
        .map(|given| format!(r#"{{"GrantAbility":{given}}}"#))
        .collect();
    format!(
        r#"{{"Static":{{"affected":{affected},"mods":[{}]}}}}"#,
        mods.join(",")
    )
}

fn leaves(by: &str, to: Option<&str>) -> String {
    let to = to.map_or(String::new(), |zone| format!(r#","to":"{zone}""#));
    format!(
        r#"{{"Replacement":{{"when":{{"ZoneChange":{{"what":["This"],"from":"Battlefield"{to}}}}},"by":{{"MoveTo":"{by}"}}}}}}"#
    )
}

#[rstest]
#[case::urban_burgeoning(
    granted(r#""Attached""#, &[UNTAP_THIS]),
    "Enchanted land has \"Untap this land during each other player's untap step.\""
)]
#[case::proud_wildbonder(
    granted(r#"{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasKeyword":"Trample"}]}"#, &[UNBLOCKED]),
    "Creatures you control with trample have \"You may have this creature assign its combat damage as though it weren't blocked.\""
)]
#[case::glowcap_lantern(
    granted(r#""Attached""#, &[EXPLORES, LOOK]),
    "Equipped creature has \"You may look at the top card of your library any time\" and \"Whenever this creature attacks, it explores.\""
)]
#[case::thorn_elemental(
    UNBLOCKED.to_owned(),
    "You may have this creature assign its combat damage as though it weren't blocked."
)]
#[case::endbringer(UNTAP_THIS.to_owned(), "Untap this creature during each other player's untap step.")]
#[case::seedborn_muse(
    r#"{"Rule":{"rule":{"UntapDuring":{"what":[{"ControlledBy":"You"}],"whose":"Opponents"}}}}"#.to_owned(),
    "Untap all permanents you control during each other player's untap step."
)]
#[case::geth_leaves(
    leaves("Exile", None),
    "If this creature would leave the battlefield, exile it instead of putting it anywhere else."
)]
#[case::leaves_plain(
    leaves("Exile", None),
    "If this creature would leave the battlefield, exile it instead."
)]
fn a_granted_or_untap_rule_is_written_as_the_card_words_it(
    #[case] json: String,
    #[case] text: &str,
) {
    assert!(
        writes(&json, text),
        "{text}: {:?}",
        super::super::render::written_variants(&ability(&json))
    );
}

#[rstest]
#[case::untap_in_your_own_step(
    r#"{"Rule":{"rule":{"UntapDuring":{"what":["This"],"whose":"You"}}}}"#.to_owned(),
    "Untap this creature during each other player's untap step."
)]
#[case::untap_what_opponents_control(
    r#"{"Rule":{"rule":{"UntapDuring":{"what":[{"ControlledBy":"Opponents"}],"whose":"Opponents"}}}}"#.to_owned(),
    "Untap all permanents you control during each other player's untap step."
)]
#[case::other_quirk(
    r#"{"Rule":{"rule":{"Combat":{"quirk":"AssignsNoCombatDamage","who":"This"}}}}"#.to_owned(),
    "You may have this creature assign its combat damage as though it weren't blocked."
)]
#[case::leaves_to_a_hand(
    leaves("Hand", None),
    "If this creature would leave the battlefield, exile it instead of putting it anywhere else."
)]
#[case::dies_is_not_leaves(
    leaves("Exile", Some("Graveyard")),
    "If this creature would leave the battlefield, exile it instead of putting it anywhere else."
)]
#[case::without_trample(
    granted(r#"{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#, &[UNBLOCKED]),
    "Creatures you control with trample have \"You may have this creature assign its combat damage as though it weren't blocked.\""
)]
#[case::another_granted_rule(
    granted(r#""Attached""#, &[r#"{"Rule":{"rule":{"NoMaxHandSize":"You"}}}"#]),
    "Enchanted land has \"Untap this land during each other player's untap step.\""
)]
#[case::one_of_two_grants_differs(
    granted(r#""Attached""#, &[EXPLORES, r#"{"Rule":{"rule":{"NoMaxHandSize":"You"}}}"#]),
    "Equipped creature has \"You may look at the top card of your library any time\" and \"Whenever this creature attacks, it explores.\""
)]
fn another_rule_is_not_the_text(#[case] json: String, #[case] text: &str) {
    assert!(!writes(&json, text), "{text}");
}
