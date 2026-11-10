//! Goal: the writers of ep22 write the card's words and nothing near them: flips in a row with one win list ("Flip five coins. Take an extra turn after this one for each coin that comes up heads.", also "For each flip you win"), "If you would flip a coin, instead flip two coins and ignore one.", and a required cost of X things beside the spell's reduction for each ("you may sacrifice any number of creatures. This spell costs {2} less to cast for each creature sacrificed this way."); another count of flips, a list on a loss, another player's coins, another reduction or another noun do not read back.
use super::*;

fn flips(times: usize, win: &str, lose: &str) -> String {
    let flip = format!(r#"{{"FlipCoin":{{"win":[{win}]{lose}}}}}"#);
    let all = vec![flip; times].join(",");
    format!(r#"{{"Loyalty":{{"cost":{{"Minus":7}},"effects":[{all}]}}}}"#)
}

const TURN: &str = r#"{"ExtraTurn":{}}"#;
const RAL: &str =
    "−7: Flip five coins. Take an extra turn after this one for each coin that comes up heads.";
const RAL_WON: &str =
    "−7: Flip five coins. For each flip you win, take an extra turn after this one.";

#[rstest]
#[case::five_heads(flips(5, TURN, ""), RAL, true)]
#[case::five_won(flips(5, TURN, ""), RAL_WON, true)]
#[case::four_is_not_five(flips(4, TURN, ""), RAL, false)]
#[case::a_list_on_a_loss(flips(5, TURN, r#","lose":[{"Draw":{}}]"#), RAL, false)]
#[case::another_win_list(flips(5, r#"{"Draw":{}}"#, ""), RAL, false)]
fn ep22_flips_in_a_row_write_as_printed(
    #[case] json: String,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability(&json));
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

const THUMB: &str = "If you would flip a coin, instead flip two coins and ignore one.";

#[rstest]
#[case::yours(r#"{"Rule":{"rule":{"FlipTwoCoins":"You"}}}"#, true)]
#[case::an_opponent_s(r#"{"Rule":{"rule":{"FlipTwoCoins":"Opponents"}}}"#, false)]
fn ep22_two_coins_write_as_printed(#[case] json: &str, #[case] reads: bool) {
    let found = written_variants(&ability(json));
    assert_eq!(
        found.iter().any(|w| same_text(w, THUMB)),
        reads,
        "{found:?}"
    );
}

#[test]
fn ep22_two_coins_read_back() {
    let json = serde_json::to_string(&golden(THUMB, ClauseKind::Static, false)).unwrap();
    assert!(json.contains("FlipTwoCoins"), "{json}");
}

fn paid(cost: &str) -> Ability {
    ability(&format!(
        r#"{{"AdditionalCost":{{"cost":[{cost}],"required":true}}}}"#
    ))
}

fn less(mana: &str, per: &str, applies: &str) -> Ability {
    ability(&format!(
        r#"{{"CostModifier":{{"applies_to":{applies},"change":"Reduce","mana":"{mana}","per":{per}}}}}"#
    ))
}

const SAC_CREATURES: &str = r#"{"Sacrifice":{"count":"X","filter":[{"Type":"Creature"}]}}"#;
const EXILE_CARDS: &str = r#"{"ExileFromGraveyard":{"count":"X","filter":[{"Type":"Creature"}]}}"#;
const EXTUS: &str = "As an additional cost to cast this spell, you may sacrifice any number of creatures. This spell costs {2} less to cast for each creature sacrificed this way.";
const GOREX: &str = "As an additional cost to cast this spell, you may exile any number of creature cards from your graveyard. This spell costs {2} less to cast for each card exiled this way.";
const THIS: &str = r#"["This"]"#;

#[rstest]
#[case::sacrificed(SAC_CREATURES, "{2}", r#""X""#, THIS, EXTUS, true)]
#[case::twice_one(
    SAC_CREATURES,
    "{1}",
    r#"{"Times":{"by":2,"of":"X"}}"#,
    THIS,
    EXTUS,
    true
)]
#[case::exiled(EXILE_CARDS, "{2}", r#""X""#, THIS, GOREX, true)]
#[case::one_less_is_not_two(SAC_CREATURES, "{1}", r#""X""#, THIS, EXTUS, false)]
#[case::other_spells(
    SAC_CREATURES,
    "{2}",
    r#""X""#,
    r#"[{"Type":"Creature"}]"#,
    EXTUS,
    false
)]
#[case::exiled_is_not_sacrificed(EXILE_CARDS, "{2}", r#""X""#, THIS, EXTUS, false)]
#[case::a_fixed_count(
    r#"{"Sacrifice":{"count":2,"filter":[{"Type":"Creature"}]}}"#,
    "{2}",
    r#""X""#,
    THIS,
    EXTUS,
    false
)]
fn ep22_a_cost_and_its_reduction_write_as_one_clause(
    #[case] cost: &str,
    #[case] mana: &str,
    #[case] per: &str,
    #[case] applies: &str,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = static_pair_texts(&paid(cost), &less(mana, per, applies));
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

const ALL_ZONES: &str = r#"{"AnyOf":[{"In":"Battlefield"},{"In":"Stack"},{"In":"Hand"},{"In":"Library"},{"In":"Graveyard"},{"In":"Exile"},{"In":"Command"}]}"#;
const SOME_ZONES: &str = r#"{"AnyOf":[{"In":"Battlefield"},{"In":"Hand"}]}"#;
const ARTIFACTS: &str = "Creatures you control are artifacts in addition to their other types. The same is true for creature spells you control and creature cards you own that aren't on the battlefield.";

fn everywhere(kind: &str, who: &str, zones: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":{{"Each":[{{"Type":"{kind}"}},{{"ControlledBy":"{who}"}},{zones}]}},"mods":[{{"AddType":"Artifact"}}]}}}}"#
    )
}

#[rstest]
#[case::every_zone(everywhere("Creature", "You", ALL_ZONES), true)]
#[case::some_zones(everywhere("Creature", "You", SOME_ZONES), false)]
#[case::an_opponent_s(everywhere("Creature", "Opponents", ALL_ZONES), false)]
#[case::another_type(everywhere("Enchantment", "You", ALL_ZONES), false)]
fn ep22_every_zone_writes_as_printed(#[case] json: String, #[case] reads: bool) {
    let found = written_variants(&ability(&json));
    assert_eq!(
        found.iter().any(|w| same_text(w, ARTIFACTS)),
        reads,
        "{found:?}"
    );
}

const MOROPHON: &str = "Creature spells you cast cost {W}{U}{B}{R}{G} less to cast. This effect reduces only the amount of colored mana you pay.";

#[rstest]
#[case::colored(r#"{"CostModifier":{"applies_to":["Spell",{"Type":"Creature"},{"ControlledBy":"You"}],"change":"Reduce","mana":"{W}{U}{B}{R}{G}"}}"#, true)]
#[case::with_generic(r#"{"CostModifier":{"applies_to":["Spell",{"Type":"Creature"},{"ControlledBy":"You"}],"change":"Reduce","mana":"{1}{W}{U}{B}{R}{G}"}}"#, false)]
#[case::an_increase(r#"{"CostModifier":{"applies_to":["Spell",{"Type":"Creature"},{"ControlledBy":"You"}],"change":"Increase","mana":"{W}{U}{B}{R}{G}"}}"#, false)]
fn ep22_a_colored_reduction_says_so(#[case] json: &str, #[case] reads: bool) {
    let found = written_variants(&ability(json));
    let wanted = if json.contains("{1}") {
        MOROPHON.replace("{W}{U}{B}{R}{G} less", "{1}{W}{U}{B}{R}{G} less")
    } else if json.contains("Increase") {
        MOROPHON.replace("less", "more")
    } else {
        MOROPHON.to_owned()
    };
    assert_eq!(
        found.iter().any(|w| same_text(w, &wanted)),
        reads,
        "{found:?}"
    );
}
