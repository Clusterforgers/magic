//! Goal: piece P5 (verbs): an unread static mode stays a gap named after the mode; `Unless` keys and the may wrapper are read
//! in one place and mean what the card prints.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

#[test]
fn unknown_static_mode_is_a_gap() {
    let found = gaps("Name:T\nTypes:Enchantment\nS:Mode$ Nonsense | Affected$ Creature\n");
    assert_eq!(found, ["static:Nonsense"]);
}

fn spell(line: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ {line}\nSVar:Sub:DB$ LoseLife | LifeAmount$ 2\nSVar:Main:DB$ GainLife | LifeAmount$ 3\n"
    )
}

#[rstest]
#[case::not_paid(
    "GainLife | LifeAmount$ 3 | UnlessCost$ 2 | UnlessPayer$ You",
    json!({"Pay": {"cost": "{2}", "unpaid": [{"GainLife": {"amount": 3}}]}})
)]
#[case::switched(
    "GainLife | LifeAmount$ 3 | UnlessCost$ 2 | UnlessPayer$ You | UnlessSwitched$ True",
    json!({"Pay": {"cost": "{2}", "paid": [{"GainLife": {"amount": 3}}]}})
)]
#[case::switched_other_payer(
    "GainLife | LifeAmount$ 3 | UnlessCost$ 2 | UnlessPayer$ Targeted | ValidTgts$ Player | UnlessSwitched$ True",
    json!({"Pay": {"who": {"Target": 0}, "cost": "{2}", "paid": [{"GainLife": {"who": {"Target": 0}, "amount": 3}}]}})
)]
#[case::subs_when_not_paid(
    "GainLife | LifeAmount$ 3 | UnlessCost$ 2 | UnlessPayer$ You | UnlessResolveSubs$ WhenNotPaid | SubAbility$ Sub",
    json!({"Pay": {"cost": "{2}",
        "unpaid": [{"GainLife": {"amount": 3}}, {"LoseLife": {"amount": 2}}]}})
)]
#[case::subs_when_paid(
    "GainLife | LifeAmount$ 3 | UnlessCost$ 2 | UnlessPayer$ You | UnlessResolveSubs$ WhenPaid | SubAbility$ Sub",
    json!({"Pay": {"cost": "{2}",
        "paid": [{"LoseLife": {"amount": 2}}], "unpaid": [{"GainLife": {"amount": 3}}]}})
)]
#[case::switched_subs_when_not_paid(
    "GainLife | LifeAmount$ 3 | UnlessCost$ 2 | UnlessPayer$ You | UnlessSwitched$ True | UnlessResolveSubs$ WhenNotPaid | SubAbility$ Sub",
    json!({"Pay": {"cost": "{2}",
        "paid": [{"GainLife": {"amount": 3}}], "unpaid": [{"LoseLife": {"amount": 2}}]}})
)]
#[case::subs_always(
    "GainLife | LifeAmount$ 3 | UnlessCost$ 2 | UnlessPayer$ You | SubAbility$ Sub",
    json!({"Pay": {"cost": "{2}", "unpaid": [{"GainLife": {"amount": 3}}]}})
)]
fn unless_branches(#[case] line: &str, #[case] pay: Value) {
    let found = abilities(&spell(line));
    assert_eq!(found.len(), 1);
    let effects = &found[0]["Spell"]["effects"];
    assert_eq!(effects[0], pay);
    let subs_always = line.contains("SubAbility$ Sub") && !line.contains("UnlessResolveSubs");
    assert_eq!(
        effects.as_array().map(Vec::len),
        Some(1 + usize::from(subs_always))
    );
}

#[rstest]
#[case::group(
    "GainLife | LifeAmount$ 3 | UnlessCost$ 2 | UnlessPayer$ Player",
    "key:Unless.UnlessPayer=Player"
)]
#[case::group_opponents(
    "GainLife | LifeAmount$ 3 | UnlessCost$ 2 | UnlessPayer$ Opponent",
    "key:Unless.UnlessPayer=Opponent"
)]
#[case::resolve_value(
    "GainLife | LifeAmount$ 3 | UnlessCost$ 2 | UnlessPayer$ You | UnlessResolveSubs$ Sometimes",
    "key:Unless.UnlessResolveSubs=Sometimes"
)]
#[case::switch_without_cost(
    "GainLife | LifeAmount$ 3 | UnlessSwitched$ True",
    "key:Unless.without-UnlessCost"
)]
fn unless_gaps(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&spell(line)), [gap]);
}

#[rstest]
#[case::decider(
    "GainLife | LifeAmount$ 3 | OptionalDecider$ You",
    json!({"May": {"effects": [{"GainLife": {"amount": 3}}]}})
)]
#[case::optional_draw(
    "Draw | NumCards$ 1 | Optional$ True",
    json!({"May": {"effects": [{"Draw": {}}]}})
)]
#[case::decider_target(
    "GainLife | ValidTgts$ Player | LifeAmount$ 3 | OptionalDecider$ Targeted",
    json!({"May": {"who": {"Target": 0}, "effects": [{"GainLife": {"who": {"Target": 0}, "amount": 3}}]}})
)]
fn may_wrapper(#[case] line: &str, #[case] node: Value) {
    let found = abilities(&spell(line));
    assert_eq!(found[0]["Spell"]["effects"][0], node);
}

#[rstest]
#[case::group_decider(
    "GainLife | LifeAmount$ 3 | OptionalDecider$ Opponent",
    "key:May.OptionalDecider=Opponent"
)]
#[case::targeted_optional(
    "Draw | Optional$ True | ValidTgts$ Player",
    "key:May.Optional-decider"
)]
#[case::other_player(
    "Draw | Optional$ True | Defined$ Opponent",
    "key:May.Optional-decider"
)]
#[case::not_a_may_api(
    "Destroy | ValidTgts$ Creature | Optional$ True",
    "key:Destroy.Optional"
)]
fn may_gaps(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&spell(line)), [gap]);
}

fn plain(line: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {line}\n")
}

#[rstest]
#[case::attach_self_to_target(
    "Attach | ValidTgts$ Creature",
    json!({"Attach": {"what": "This", "to": {"Target": 0}}})
)]
#[case::attach_defined_host("Attach | Defined$ Enchanted", json!({"Attach": {"what": "This", "to": "Attached"}}))]
#[case::attach_object(
    "Attach | Object$ Self | ValidTgts$ Creature",
    json!({"Attach": {"what": "This", "to": {"Target": 0}}})
)]
#[case::attach_target_to_host(
    "Attach | ValidTgts$ Equipment | Object$ Targeted | Defined$ Self",
    json!({"Attach": {"what": {"Target": 0}, "to": "This"}})
)]
#[case::attach_optional(
    "Attach | ValidTgts$ Creature | Optional$ True",
    json!({"May": {"effects": [{"Attach": {"what": "This", "to": {"Target": 0}}}]}})
)]
fn attach_words(#[case] line: &str, #[case] node: Value) {
    let found = abilities(&plain(line));
    assert_eq!(found[0]["Spell"]["effects"][0], node);
}

#[rstest]
#[case::all(
    "Attach | Object$ Valid Equipment | Defined$ Self",
    "key:Attach.Object-all"
)]
#[case::both_hosts(
    "Attach | Object$ Self | ValidTgts$ Creature | Defined$ Self",
    "key:Attach.Object-shape"
)]
#[case::nothing("Attach", "key:Attach.no-ValidTgts")]
fn attach_gaps(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&plain(line)), [gap]);
}

#[rstest]
#[case::you("GainControl | ValidTgts$ Creature | NewController$ You", "You")]
#[case::default("GainControl | ValidTgts$ Creature", "You")]
#[case::other(
    "GainControl | ValidTgts$ Creature | NewController$ DefendingPlayer",
    "Defending"
)]
fn gain_control_new_controller(#[case] line: &str, #[case] who: &str) {
    let found = abilities(&plain(line));
    assert_eq!(
        found[0]["Spell"]["effects"][0]["Continuous"]["mods"][0]["SetController"],
        who
    );
}

#[rstest]
#[case::keyword("PutCounter | ValidTgts$ Creature | CounterType$ Flying", "flying")]
#[case::spaced(
    "PutCounter | ValidTgts$ Creature | CounterType$ First Strike",
    "first strike"
)]
#[case::experience("PutCounter | Defined$ You | CounterType$ Experience", "experience")]
fn keyword_and_player_counters(#[case] line: &str, #[case] name: &str) {
    let found = abilities(&plain(line));
    assert_eq!(
        found[0]["Spell"]["effects"][0]["PutCounters"]["counter"],
        json!({"Other": name})
    );
}

#[rstest]
#[case::unknown(
    "PutCounter | ValidTgts$ Creature | CounterType$ Wibble",
    "key:PutCounter.CounterType"
)]
#[case::any(
    "RemoveCounter | ValidTgts$ Creature | CounterType$ Any",
    "key:RemoveCounter.CounterType"
)]
fn unknown_counter_words(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&plain(line)), [gap]);
}

#[test]
fn a_group_cannot_take_control() {
    let line = "GainControl | ValidTgts$ Creature | NewController$ Player.Opponent";
    assert_eq!(
        gaps(&plain(line)),
        ["key:GainControl.NewController=Player.Opponent"]
    );
}

#[test]
fn the_sacrifice_prompt_is_text_only() {
    let line = "Sacrifice | SacValid$ Creature | SacMessage$ Creature";
    assert_eq!(gaps(&plain(line)), Vec::<String>::new());
}

#[rstest]
#[case::you("Poison | Defined$ You | Num$ 2", json!({"PutCounters": {"on": "You", "counter": "poison", "count": 2}}))]
#[case::target(
    "Poison | ValidTgts$ Opponent | Num$ 1",
    json!({"PutCounters": {"on": {"Target": 0}, "counter": "poison"}})
)]
fn poison_counters(#[case] line: &str, #[case] node: Value) {
    let found = abilities(&plain(line));
    assert_eq!(found[0]["Spell"]["effects"][0], node);
}

#[test]
fn poison_without_a_number_is_a_gap() {
    assert_eq!(gaps(&plain("Poison | Defined$ You")), ["key:Poison.no-Num"]);
}

#[test]
fn fog_prevents_combat_damage_this_turn() {
    let found = abilities(&plain("Fog"));
    assert_eq!(
        found[0]["Spell"]["effects"][0],
        json!({"Shield": {"when": {"Damage": {"combat": true}}, "by": "Prevent",
            "duration": "UntilEndOfTurn"}})
    );
}

#[rstest]
#[case::draw_this_turn("SkipPhase | Defined$ You | Step$ Draw | Duration$ EndOfTurn", &[])]
#[case::next_draw("SkipPhase | ValidTgts$ Player | Step$ Draw", &["key:SkipPhase.next-occurrence"])]
#[case::combat("SkipPhase | Defined$ You | Phase$ BeginCombat | Duration$ EndOfTurn", &["key:SkipPhase.Phase"])]
fn skipped_steps(#[case] line: &str, #[case] expected: &[&str]) {
    assert_eq!(gaps(&plain(line)), expected);
}

#[rstest]
#[case::itself("SacrificeAll | Defined$ Self | Controller$ You", None)]
#[case::its_controller("SacrificeAll | Defined$ Self", Some(json!({"ControllerOf": "This"})))]
fn sacrifice_named_objects(#[case] line: &str, #[case] who: Option<Value>) {
    let found = abilities(&plain(line));
    let mut node = json!({"Sacrifice": {"filter": [{"Is": "This"}]}});
    if let Some(who) = who {
        node["Sacrifice"]["who"] = who;
    }
    assert_eq!(found[0]["Spell"]["effects"][0], node);
}

#[test]
fn sacrifice_all_of_a_valid_string_with_a_controller_is_a_gap() {
    assert_eq!(
        gaps(&plain(
            "SacrificeAll | ValidCards$ Creature.YouCtrl | Controller$ You"
        )),
        ["key:SacrificeAll.Controller"]
    );
}

#[test]
fn tap_any_number_picks_then_taps_and_binds_the_tapped() {
    let line = "Tap | CardChoices$ Creature.YouCtrl+untapped | AnyNumber$ True | ChoiceAmount$ Count$Valid Creature.YouCtrl+untapped | RememberTapped$ True | SubAbility$ Main";
    assert_eq!(gaps(&spell(line)), Vec::<String>::new());
    let found = abilities(&spell(line));
    let effects = &found[0]["Spell"]["effects"];
    let bind = effects[0]["Choose"]["as"].clone();
    for effect in effects.as_array().into_iter().flatten() {
        let parsed = serde_json::from_value::<mtg_cards::effect::Effect>(effect.clone());
        assert!(parsed.is_ok(), "{effect}");
    }
    assert_eq!(effects[0]["Choose"]["count"]["min"], 0);
    assert_eq!(effects[1]["Tap"]["what"], json!({"Bound": bind}));
    assert!(effects[1]["Tap"]["as"].is_string());
}

#[rstest]
#[case::cap(
    "Tap | CardChoices$ Creature | AnyNumber$ True | ChoiceAmount$ 2",
    "key:Tap.CardChoices-amount"
)]
#[case::fixed(
    "Tap | CardChoices$ Creature | ChoiceAmount$ 1",
    "key:Tap.CardChoices-amount"
)]
fn tap_choices_with_a_cap_are_gaps(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&plain(line)), [gap]);
}

#[rstest]
#[case::poison("Poison | Defined$ Player.Opponent | Num$ 2")]
#[case::fog("Fog")]
#[case::skip("SkipPhase | Defined$ You | Step$ Draw | Duration$ EndOfTurn")]
#[case::sacrifice("SacrificeAll | Defined$ Self")]
#[case::earthbend("Earthbend | Num$ 2")]
#[case::discover("Discover | Num$ 3")]
fn new_verbs_are_nodes_of_the_language(#[case] line: &str) {
    let found = abilities(&plain(line));
    for effect in found[0]["Spell"]["effects"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let parsed = serde_json::from_value::<mtg_cards::effect::Effect>(effect.clone());
        assert!(parsed.is_ok(), "{effect}");
    }
}

#[rstest]
#[case::earthbend("Earthbend | Num$ 2")]
#[case::discover("Discover | Num$ 4")]
fn keyword_actions_expand(#[case] line: &str) {
    assert_eq!(gaps(&plain(line)), Vec::<String>::new());
    let found = abilities(&plain(line));
    let effects = found[0]["Spell"]["effects"].as_array().map_or(0, Vec::len);
    assert!(effects >= 3, "{effects}");
}

/// The effects of a copy line, each checked to be a node of the language.
fn copy_nodes(words: &str) -> Vec<Value> {
    let line = format!("CopyPermanent | ValidTgts$ Creature.YouCtrl | {words}");
    assert_eq!(gaps(&plain(&line)), Vec::<String>::new(), "{words}");
    let found = abilities(&plain(&line));
    let effects = found[0]["Spell"]["effects"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for effect in &effects {
        let parsed = serde_json::from_value::<mtg_cards::effect::Effect>(effect.clone());
        assert!(parsed.is_ok(), "{effect}");
    }
    effects
}

#[rstest]
#[case::stats("SetPower$ 4 | SetToughness$ 4", json!({"SetPT": {"power": 4, "toughness": 4}}))]
#[case::color("SetColor$ Black", json!({"SetColors": ["Black"]}))]
#[case::creature_type("SetCreatureTypes$ Soldier", json!({"SetCreatureTypes": ["Soldier"]}))]
#[case::artifact("AddTypes$ Artifact", json!({"AddType": "Artifact"}))]
#[case::subtype("AddTypes$ Soldier", json!({"AddSubtype": "Soldier"}))]
#[case::legendary("AddTypes$ Legendary", json!({"AddSupertype": "Legendary"}))]
#[case::non_legendary("NonLegendary$ True", json!({"RemoveSupertype": "Legendary"}))]
#[case::keyword("AddKeywords$ Haste", json!({"AddKeyword": "Haste"}))]
#[case::pumped_keyword("PumpKeywords$ Haste", json!({"AddKeyword": "Haste"}))]
fn copy_exceptions_are_written_into_the_copy(#[case] words: &str, #[case] change: Value) {
    let effects = copy_nodes(words);
    assert_eq!(effects.len(), 1);
    assert_eq!(
        effects[0]["CreateToken"]["token"]["CopyOf"]["except"][0],
        change
    );
}

#[test]
fn a_pump_duration_makes_the_keyword_a_continuous_effect_on_the_copies() {
    let effects = copy_nodes("PumpKeywords$ Haste | PumpDuration$ EOT");
    let bind = effects[0]["CreateToken"]["as"].clone();
    assert!(
        effects[0]["CreateToken"]["token"]["CopyOf"]
            .get("except")
            .is_none()
    );
    assert_eq!(effects[1]["Continuous"]["affected"], json!({"Bound": bind}));
    assert_eq!(effects[1]["Continuous"]["duration"], "UntilEndOfTurn");
}

#[rstest]
#[case::sacrifice("Sacrifice", "End", "", "Sacrifice")]
#[case::exile("Exile", "End", "", "Exile")]
#[case::your_exile("YourExile", "End", "You", "Exile")]
#[case::combat("ExileCombat", "EndCombat", "", "Exile")]
#[case::destroy("Destroy", "End", "", "Destroy")]
fn the_copies_leave_at_the_end_step(
    #[case] word: &str,
    #[case] step: &str,
    #[case] whose: &str,
    #[case] verb: &str,
) {
    let effects = copy_nodes(&format!("AtEOT$ {word}"));
    let bind = effects[0]["CreateToken"]["as"].clone();
    let when = &effects[1]["DelayedTrigger"]["when"]["StepBegins"];
    // Every player's step is the default and is not written.
    let whose = if whose.is_empty() {
        Value::Null
    } else {
        json!(whose)
    };
    assert_eq!((&when["step"], &when["whose"]), (&json!(step), &whose));
    let effect = &effects[1]["DelayedTrigger"]["effects"][0][verb];
    assert!(effect.to_string().contains(&bind.to_string()), "{effect}");
}

#[rstest]
#[case::half_stats("SetPower$ 4", "key:CopyPermanent.half-stats")]
#[case::unknown_type("AddTypes$ Wibble", "key:CopyPermanent.AddTypes=Wibble")]
#[case::later_upkeep("AtEOT$ SacrificeUpkeep", "key:AtEOT=SacrificeUpkeep")]
#[case::pump_for_ever(
    "PumpKeywords$ Haste | PumpDuration$ Permanent",
    "key:CopyPermanent.PumpDuration=Permanent"
)]
fn copy_words_that_are_not_read_are_gaps(#[case] words: &str, #[case] gap: &str) {
    let line = format!("CopyPermanent | ValidTgts$ Creature | {words}");
    assert_eq!(gaps(&plain(&line)), [gap]);
}

#[test]
fn a_villainous_choice_is_made_by_one_player() {
    let line = "VillainousChoice | Defined$ You | Choices$ Sub,Main";
    assert_eq!(gaps(&spell(line)), Vec::<String>::new());
    let found = abilities(&spell(line));
    let node = &found[0]["Spell"]["effects"][0]["VillainousChoice"];
    assert_eq!(node["who"], "You");
    assert_eq!(node["first"], json!([{"LoseLife": {"amount": 2}}]));
    assert_eq!(node["second"], json!([{"GainLife": {"amount": 3}}]));
}

#[rstest]
#[case::opponents(
    "VillainousChoice | Defined$ Opponent | Choices$ Sub,Main",
    "key:VillainousChoice.Defined=Opponent"
)]
#[case::three(
    "VillainousChoice | Defined$ You | Choices$ Sub,Main,Sub",
    "key:VillainousChoice.Choices"
)]
fn villainous_choice_gaps(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&spell(line)), [gap]);
}

#[test]
fn a_delayed_trigger_reads_any_trigger_mode() {
    let line = "DelayedTrigger | Mode$ SpellCast | ValidActivatingPlayer$ You | ThisTurn$ True | Execute$ Main";
    assert_eq!(gaps(&spell(line)), Vec::<String>::new());
    let found = abilities(&spell(line));
    let node = &found[0]["Spell"]["effects"][0]["DelayedTrigger"];
    assert_eq!(node["when"], json!({"SpellCast": {"by": "You"}}));
    assert_eq!(node["this_turn"], true);
    assert_eq!(node["effects"], json!([{"GainLife": {"amount": 3}}]));
}

#[test]
fn created_tokens_leave_at_the_end_step_by_name() {
    let line = "Token | TokenScript$ w_1_1_soldier | AtEOT$ Sacrifice";
    assert_eq!(gaps(&plain(line)), Vec::<String>::new());
    let found = abilities(&plain(line));
    let effects = &found[0]["Spell"]["effects"];
    let bind = effects[0]["CreateToken"]["as"].clone();
    assert!(bind.is_string());
    let gone = &effects[1]["DelayedTrigger"]["effects"][0]["Sacrifice"];
    assert_eq!(gone["filter"], json!([{"Is": {"Bound": bind}}]));
}

/// The modifications an animate line gives.
fn animate_mods(words: &str) -> Vec<Value> {
    let line = format!("Animate | ValidTgts$ Permanent | Duration$ Permanent | {words}");
    assert_eq!(gaps(&plain(&line)), Vec::<String>::new(), "{words}");
    let found = abilities(&plain(&line));
    let effect = &found[0]["Spell"]["effects"][0];
    let parsed = serde_json::from_value::<mtg_cards::effect::Effect>(effect.clone());
    assert!(parsed.is_ok(), "{effect}");
    effect["Continuous"]["mods"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

#[rstest]
#[case::becomes_only_a_creature(
    "Power$ 3 | Toughness$ 3 | Types$ Creature,Soldier | RemoveCardTypes$ True | RemoveCreatureTypes$ True",
    &[json!({"SetTypes": ["Creature"]}), json!({"SetCreatureTypes": ["Soldier"]})]
)]
#[case::becomes_an_island(
    "Types$ Island | RemoveLandTypes$ True",
    &[json!({"SetLandTypes": ["Island"]})]
)]
#[case::all_creature_types("AddAllCreatureTypes$ True", &[json!("AddAllCreatureTypes")])]
#[case::legendary("Types$ Legendary", &[json!({"AddSupertype": "Legendary"})])]
fn animate_type_words(#[case] words: &str, #[case] wanted: &[Value]) {
    let mods = animate_mods(words);
    for change in wanted {
        assert!(mods.contains(change), "{change} in {mods:?}");
    }
}

#[rstest]
#[case::no_types(
    "RemoveCardTypes$ True | Power$ 1 | Toughness$ 1",
    "key:Animate.RemoveCardTypes"
)]
#[case::only_subtypes(
    "Types$ Soldier | RemoveCardTypes$ True",
    "key:Animate.RemoveCardTypes"
)]
#[case::lands_without_types(
    "RemoveLandTypes$ True | Power$ 1 | Toughness$ 1",
    "key:Animate.RemoveLandTypes"
)]
#[case::not_a_basic_land_type("Types$ Soldier | RemoveLandTypes$ True", "")]
fn animate_type_gaps(#[case] words: &str, #[case] gap: &str) {
    let line = format!("Animate | ValidTgts$ Permanent | {words}");
    let found = gaps(&plain(&line));
    if gap.is_empty() {
        // A creature type with `RemoveLandTypes$` is an added subtype, so nothing is refused.
        assert!(found.is_empty(), "{found:?}");
    } else {
        assert_eq!(found, [gap]);
    }
}
