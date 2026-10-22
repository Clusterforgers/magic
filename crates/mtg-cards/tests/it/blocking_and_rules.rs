//! Goal: each node the coverage report ranked next (blocking restrictions, hand size, land plays, commander eligibility, looking at the top, choosing a player, counting players and life, and eight keywords) validates, round-trips and names its primitive, is refused where it is wrong, and leaves a card using it Partial until the engine lists it.
#![allow(clippy::unwrap_used)]

use std::collections::BTreeSet;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, Source};
use mtg_cards::validate::used_primitives;
use rstest::rstest;

/// Loads a card whose one face has this ability, or says why the library refused it.
fn load(ability: &str) -> Result<(), String> {
    let text = format!(
        r#"{{"oracle_id":"4457ed35-7c10-48c8-9776-456485fdf070","name":"Test","impl_status":"Full","faces":[{{"name":"Test","oracle_fingerprint":"e3b0c44298fc1c149afbf4c8996fb924","abilities":[{ability}]}}]}}"#
    );
    Library::from_sources(&[Source {
        path: "cards/test.json".into(),
        text,
    }])
    .map(|_| ())
    .map_err(|errors| errors.to_string())
}

fn names(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|name| (*name).to_owned()).collect()
}

const ENTERS: &str = r#""when":{"Enters":{"what":["This"]}}"#;

#[rstest]
#[case::cant_be_blocked(
    r#"{"Static":{"affected":"This","mods":["CantBeBlocked"]}}"#,
    &["Ability.Static", "Mod.CantBeBlocked"]
)]
#[case::cant_be_blocked_by_small_creatures(
    r#"{"Static":{"affected":"This","mods":[{"CantBeBlockedBy":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Le","to":2}}]}]}}"#,
    &["Ability.Static", "Mod.CantBeBlockedBy"]
)]
#[case::cant_block_and_cant_be_blocked(
    r#"{"Static":{"affected":"This","mods":["CantBlock","CantBeBlocked"]}}"#,
    &["Ability.Static", "Mod.CantBeBlocked", "Mod.CantBlock"]
)]
#[case::an_aura_that_makes_a_creature_unblockable(
    r#"{"Static":{"affected":"Attached","mods":["CantBeBlocked"]}}"#,
    &["Ability.Static", "Mod.CantBeBlocked"]
)]
#[case::unblockable_until_end_of_turn(
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":["CantBeBlocked"],"duration":"UntilEndOfTurn"}}]}}"#,
    &["Ability.Spell", "Effect.Continuous", "Mod.CantBeBlocked"]
)]
#[case::no_maximum_hand_size(
    r#"{"Rule":{"rule":{"NoMaxHandSize":"You"}}}"#,
    &["Ability.Rule", "Rule.NoMaxHandSize"]
)]
#[case::every_player_has_no_maximum_hand_size(
    r#"{"Rule":{"rule":{"NoMaxHandSize":"EachPlayer"}}}"#,
    &["Ability.Rule", "Rule.NoMaxHandSize"]
)]
#[case::looking_at_the_top_card(
    r#"{"Rule":{"rule":{"LookAtTopAnyTime":"You"}}}"#,
    &["Ability.Rule", "Rule.LookAtTopAnyTime"]
)]
#[case::an_additional_land_play(
    r#"{"Rule":{"rule":{"AdditionalLandPlays":{}}}}"#,
    &["Ability.Rule", "Rule.AdditionalLandPlays"]
)]
#[case::two_additional_land_plays(
    r#"{"Rule":{"rule":{"AdditionalLandPlays":{"count":2}}}}"#,
    &["Ability.Rule", "Rule.AdditionalLandPlays"]
)]
#[case::an_additional_land_play_for_everyone(
    r#"{"Rule":{"rule":{"AdditionalLandPlays":{"who":"EachPlayer"}}}}"#,
    &["Ability.Rule", "Rule.AdditionalLandPlays"]
)]
#[case::an_additional_land_play_this_turn(
    r#"{"Spell":{"effects":[{"Rule":{"rule":{"AdditionalLandPlays":{}},"duration":"UntilEndOfTurn"}}]}}"#,
    &["Ability.Spell", "Effect.Rule", "Rule.AdditionalLandPlays"]
)]
#[case::can_be_your_commander(
    r#"{"Rule":{"rule":"CanBeCommander"}}"#,
    &["Ability.Rule", "Rule.CanBeCommander"]
)]
#[case::choosing_a_player(
    &format!(r#"{{"Replacement":{{{ENTERS},"by":{{"Choose":"Player"}}}}}}"#),
    &["Ability.Replacement", "ReplaceBy.Choose", "ReplaceBy.Choose.Player", "ReplaceWhen.Enters"]
)]
#[case::choosing_an_opponent(
    &format!(r#"{{"Replacement":{{{ENTERS},"by":{{"Choose":"Opponent"}}}}}}"#),
    &["Ability.Replacement", "ReplaceBy.Choose", "ReplaceBy.Choose.Opponent", "ReplaceWhen.Enters"]
)]
#[case::choosing_a_color_is_not_a_gap_of_its_own(
    &format!(r#"{{"Replacement":{{{ENTERS},"by":{{"Choose":"Color"}}}}}}"#),
    &["Ability.Replacement", "ReplaceBy.Choose", "ReplaceWhen.Enters"]
)]
#[case::entering_tapped_unless_two_opponents_or_a_low_life_total(
    &format!(
        r#"{{"Replacement":{{{ENTERS},"by":"Tapped","condition":{{"Not":{{"Any":[{{"Compare":{{"left":{{"Players":"Opponents"}},"op":"Ge","right":2}}}},{{"Compare":{{"left":{{"LowestLife":"EachPlayer"}},"op":"Le","right":13}}}}]}}}}}}}}"#
    ),
    &["Ability.Replacement", "Amount.LowestLife", "Amount.Players", "ReplaceBy.Tapped", "ReplaceWhen.Enters"]
)]
#[case::a_static_ability_while_an_opponent_has_low_life(
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Haste"}],"condition":{"Compare":{"left":{"LowestLife":"Opponents"},"op":"Le","right":10}}}}"#,
    &["Ability.Static", "Amount.LowestLife", "Keyword.Haste", "Mod.AddKeyword"]
)]
#[case::a_static_ability_while_every_opponent_has_low_life(
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Haste"}],"condition":{"Compare":{"left":{"HighestLife":"Opponents"},"op":"Le","right":10}}}}"#,
    &["Ability.Static", "Amount.HighestLife", "Keyword.Haste", "Mod.AddKeyword"]
)]
#[case::changeling(r#"{"Keyword":"Changeling"}"#, &["Keyword.Changeling"])]
#[case::devoid(r#"{"Keyword":"Devoid"}"#, &["Keyword.Devoid"])]
#[case::infect(r#"{"Keyword":"Infect"}"#, &["Keyword.Infect"])]
#[case::myriad(r#"{"Keyword":"Myriad"}"#, &["Keyword.Myriad"])]
#[case::crew(r#"{"Keyword":{"Crew":3}}"#, &["Keyword.Crew"])]
#[case::mutate(r#"{"Keyword":{"Mutate":"{1}{G}{G}"}}"#, &["Cost.Mana", "Keyword.Mutate"])]
#[case::encore(r#"{"Keyword":{"Encore":"{5}{R}"}}"#, &["Cost.Mana", "Keyword.Encore"])]
#[case::squad(r#"{"Keyword":{"Squad":"{2}"}}"#, &["Cost.Mana", "Keyword.Squad"])]
#[case::foretell(r#"{"Keyword":{"Foretell":"{1}{U}"}}"#, &["Cost.Mana", "Keyword.Foretell"])]
#[case::buyback(r#"{"Keyword":{"Buyback":"{3}"}}"#, &["Cost.Mana", "Keyword.Buyback"])]
#[case::entwine(r#"{"Keyword":{"Entwine":"{2}"}}"#, &["Cost.Mana", "Keyword.Entwine"])]
#[case::a_kicker_that_sacrifices(
    r#"{"Keyword":{"Kicker":[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}]}}"#,
    &["Cost.Sacrifice", "Keyword.Kicker"]
)]
#[case::a_buyback_in_life(r#"{"Keyword":{"Buyback":[{"PayLife":4}]}}"#, &["Cost.PayLife", "Keyword.Buyback"])]
#[case::an_entwine_that_discards(
    r#"{"Keyword":{"Entwine":[{"Mana":"{1}"},{"Discard":{"count":2}}]}}"#,
    &["Cost.Discard", "Cost.Mana", "Keyword.Entwine"]
)]
#[case::an_evoke_that_exiles_from_the_hand(
    r#"{"Keyword":{"Evoke":[{"ExileFromHand":{"filter":[{"Color":"Red"}]}}]}}"#,
    &["Cost.ExileFromHand", "Keyword.Evoke"]
)]
#[case::a_flashback_in_mana_and_life(
    r#"{"Keyword":{"Flashback":[{"Mana":"{1}{U}"},{"PayLife":3}]}}"#,
    &["Cost.Mana", "Cost.PayLife", "Keyword.Flashback"]
)]
#[case::a_madness_in_mana_and_life(
    r#"{"Keyword":{"Madness":[{"Mana":"{2}{B}"},{"PayLife":8}]}}"#,
    &["Cost.Mana", "Cost.PayLife", "Keyword.Madness"]
)]
#[case::an_optional_additional_cost(
    r#"{"AdditionalCost":{"cost":[{"Sacrifice":{"filter":[{"Type":"Artifact"}]}}]}}"#,
    &["Ability.AdditionalCost", "Cost.Sacrifice"]
)]
#[case::a_multikicker_with_a_part_is_still_named(
    r#"{"Keyword":{"Multikicker":[{"Sacrifice":{"filter":[{"Type":"Land"}]}}]}}"#,
    &["Cost.Sacrifice", "Keyword.Cost.Sacrifice", "Keyword.Multikicker"]
)]
#[case::a_suspend_with_a_part_is_still_named(
    r#"{"Keyword":{"Suspend":{"time":2,"cost":[{"PayLife":2}]}}}"#,
    &["Cost.PayLife", "Keyword.Cost.PayLife", "Keyword.Suspend"]
)]
#[case::a_flashback_that_counts_x_is_named(
    r#"{"Keyword":{"Flashback":[{"Mana":"{R}{R}"},{"Sacrifice":{"filter":[{"Type":"Land"}],"count":"X"}}]}}"#,
    &["Amount.X.Elsewhere", "Cost.Mana", "Cost.Sacrifice", "Keyword.Cost.Sacrifice.X", "Keyword.Flashback"]
)]
#[case::a_kicker_that_removes_counters_is_named(
    r#"{"Keyword":{"Kicker":[{"RemoveCounters":{"counter":"charge"}}]}}"#,
    &["Cost.RemoveCounters", "Keyword.Cost.RemoveCounters", "Keyword.Kicker"]
)]
#[case::a_granted_changeling(
    r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":"Changeling"}]}}"#,
    &["Ability.Static", "Keyword.Changeling", "Mod.AddKeyword"]
)]
fn a_node_validates_round_trips_and_names_its_primitive(
    #[case] json: &str,
    #[case] expected: &[&str],
) {
    assert_eq!(load(json), Ok(()));
    let ability: Ability = serde_json::from_str(json).unwrap();
    assert_eq!(serde_json::to_string(&ability).unwrap(), json);
    assert_eq!(used_primitives(&ability), names(expected));
}

#[rstest]
#[case::crew_for_nothing(r#"{"Keyword":{"Crew":0}}"#, "crew needs a power of at least 1")]
#[case::no_additional_land(
    r#"{"Rule":{"rule":{"AdditionalLandPlays":{"count":0}}}}"#,
    "an additional land play is at least one"
)]
#[case::a_negative_number_of_lands(
    r#"{"Rule":{"rule":{"AdditionalLandPlays":{"count":-1}}}}"#,
    "an additional land play is at least one"
)]
#[case::a_commander_in_one_zone(
    r#"{"Rule":{"rule":"CanBeCommander","from":"Hand"}}"#,
    "a card can be a commander in every zone"
)]
#[case::an_object_has_no_hand_size(
    r#"{"Rule":{"rule":{"NoMaxHandSize":"This"}}}"#,
    "expected a player"
)]
#[case::an_object_does_not_look_at_the_top(
    r#"{"Rule":{"rule":{"LookAtTopAnyTime":"This"}}}"#,
    "expected a player"
)]
#[case::an_object_plays_no_land(
    r#"{"Rule":{"rule":{"AdditionalLandPlays":{"who":"This"}}}}"#,
    "expected a player"
)]
#[case::an_object_is_not_counted_as_a_player(
    r#"{"Spell":{"effects":[{"If":{"condition":{"Compare":{"left":{"Players":"This"},"op":"Ge","right":2}},"then":[{"Draw":{}}]}}]}}"#,
    "expected a player"
)]
#[case::an_object_has_no_life_total(
    r#"{"Spell":{"effects":[{"If":{"condition":{"Compare":{"left":{"LowestLife":"This"},"op":"Le","right":13}},"then":[{"Draw":{}}]}}]}}"#,
    "expected a player"
)]
#[case::a_target_that_is_not_there(
    r#"{"Static":{"affected":"This","mods":[{"CantBeBlockedBy":[{"ControlledBy":{"Target":0}}]}]}}"#,
    "target 0 does not exist"
)]
#[case::a_cost_is_checked(
    r#"{"Keyword":{"Squad":[{"PayEnergy":{"Life":"This"}}]}}"#,
    "expected a player"
)]
fn a_node_used_wrongly_is_refused_where_it_stands(#[case] json: &str, #[case] message: &str) {
    let error = load(json).unwrap_err();
    assert!(error.contains(message), "{message:?} is not in {error}");
}

#[rstest]
#[case::cant_be_blocked("Mod.CantBeBlocked")]
#[case::cant_be_blocked_by("Mod.CantBeBlockedBy")]
#[case::no_maximum_hand_size("Rule.NoMaxHandSize")]
#[case::looking_at_the_top("Rule.LookAtTopAnyTime")]
#[case::additional_land_plays("Rule.AdditionalLandPlays")]
#[case::commander_eligibility("Rule.CanBeCommander")]
#[case::choosing_a_player("ReplaceBy.Choose.Player")]
#[case::choosing_an_opponent("ReplaceBy.Choose.Opponent")]
#[case::counting_players("Amount.Players")]
#[case::the_lowest_life_total("Amount.LowestLife")]
#[case::the_highest_life_total("Amount.HighestLife")]
#[case::changeling("Keyword.Changeling")]
#[case::devoid("Keyword.Devoid")]
#[case::infect("Keyword.Infect")]
#[case::myriad("Keyword.Myriad")]
#[case::crew("Keyword.Crew")]
#[case::mutate("Keyword.Mutate")]
#[case::encore("Keyword.Encore")]
#[case::squad("Keyword.Squad")]
#[case::foretell("Keyword.Foretell")]
fn a_caps_that_lacks_the_primitive_reports_it_missing(#[case] name: &str) {
    let lacking = Caps::none();
    assert!(!lacking.runs(name));
    assert_eq!(lacking.missing(&names(&[name])), vec![name.to_owned()]);
    assert!(Caps::everything().runs(name));
}
