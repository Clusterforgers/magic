//! Goal: pack e14f's mechanics read from Forge lines into the engine's one model of each: haunt is the keyword with the card's ability on entering (or as the spell) and from exile when the haunted creature dies, a Case is solved and reads as solved, a card revealed from the opening hand sets up its one trigger, a permanent gets counters as it turns face up or can't be turned face up, the legend rule stops for a selector, the life lost this way is read for Forge's placeholder, and a shape these readers do not know stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

const DESTROY: &str = "SVar:TrigDestroy:DB$ Destroy | ValidTgts$ Enchantment | SpellDescription$ Destroy target enchantment.\n";

#[rstest]
#[case::a_creature("Creature Thrull", "Triggered")]
#[case::a_sorcery("Sorcery", "Spell")]
fn haunt_works_on_entering_or_as_the_spell_and_from_exile(
    #[case] types: &str,
    #[case] first: &str,
) {
    let text = format!("Name:T\nTypes:{types}\nK:Haunt:TrigDestroy\n{DESTROY}");
    assert_eq!(gaps(&text), Vec::<String>::new());
    let found = abilities(&text);
    assert_eq!(found[0], json!({"Keyword": "Haunt"}));
    assert!(found[1].get(first).is_some(), "{:?}", found[1]);
    if first == "Triggered" {
        assert_eq!(
            found[1]["Triggered"]["when"],
            json!({"ZoneChange": {"what": ["This"], "to": "Battlefield"}})
        );
    }
    let haunted = &found[2]["Triggered"];
    assert_eq!(haunted["from"], json!("Exile"));
    assert_eq!(
        haunted["when"],
        json!({"ZoneChange": {"what": [{"HauntedBy": "This"}], "from": "Battlefield", "to": "Graveyard"}})
    );
    assert_eq!(haunted["effects"], found[1][first]["effects"]);
    assert_eq!(
        haunted["targets"],
        json!([{"what": {"Object": [{"Type": "Enchantment"}]}}])
    );
}

#[rstest]
#[case::a_land("Land", "keyword:Haunt-shape")]
#[case::no_part("Creature", "keyword:Haunt-shape")]
fn haunt_of_another_shape_is_a_gap(#[case] types: &str, #[case] gap: &str) {
    let keyword = if types == "Land" {
        "Haunt:TrigDestroy"
    } else {
        "Haunt:"
    };
    let text = format!("Name:T\nTypes:{types}\nK:{keyword}\n{DESTROY}");
    assert!(gaps(&text).contains(&gap.to_owned()), "{:?}", gaps(&text));
}

#[test]
fn a_case_is_solved_and_reads_as_solved() {
    let text = "Name:T\nTypes:Enchantment Case\nT:Mode$ Phase | Phase$ End of Turn | ValidPlayer$ You | Hellbent$ True | IsPresent$ Card.Self+!IsSolved | TriggerZones$ Battlefield | Execute$ TrigSolve | TriggerDescription$ To solve.\nSVar:TrigSolve:DB$ AlterAttribute | Defined$ Self | Attributes$ Solved\nS:Mode$ Continuous | Affected$ Creature.YouCtrl | AddPower$ 1 | IsPresent$ Card.Self+IsSolved | Description$ Solved.\n";
    assert_eq!(gaps(text), Vec::<String>::new());
    let found = abilities(text);
    let printed = serde_json::to_string(&found).unwrap_or_default();
    assert!(
        printed.contains(r#"{"Solve":{"what":"This"}}"#),
        "{printed}"
    );
    assert!(printed.contains(r#"{"Not":"Solved"}"#), "{printed}");
    assert!(printed.contains(r#"["This","Solved"]"#), "{printed}");
}

/// A card that may be revealed from the opening hand: the reveal, its effect and the trigger of the effect.
fn chancellor(effect: &str, trigger: &str) -> String {
    format!(
        "Name:T\nTypes:Creature\nK:MayEffectFromOpeningHand:RevealCard\nSVar:RevealCard:DB$ Reveal | RevealDefined$ Self | SubAbility$ Eff | SpellDescription$ T.\nSVar:Eff:DB$ Effect | Triggers$ Trig{effect}\nSVar:Trig:Mode$ Phase | {trigger} | OneOff$ True | Execute$ Go | TriggerDescription$ T.\nSVar:Go:DB$ Scry | ScryNum$ 3\n"
    )
}

#[rstest]
#[case::the_first_upkeep("", "Phase$ Upkeep", json!({"StepBegins": {"step": "Upkeep"}}))]
#[case::your_first_upkeep(" | Duration$ Permanent", "Phase$ Upkeep | ValidPlayer$ You", json!({"StepBegins": {"step": "Upkeep", "whose": "You"}}))]
fn a_card_revealed_from_the_opening_hand_waits_for_its_step(
    #[case] effect: &str,
    #[case] trigger: &str,
    #[case] when: Value,
) {
    let text = chancellor(effect, trigger);
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(
        abilities(&text)[0],
        json!({"OpeningReveal": {"when": when, "effects": [{"Scry": {"count": 3}}]}})
    );
}

#[rstest]
#[case::owned_by_an_opponent(" | EffectOwner$ Opponent", "Phase$ Upkeep")]
#[case::not_once(" | Duration$ Permanent", "Phase$ Upkeep | ValidPlayer$ You")]
fn an_opening_reveal_of_another_shape_is_a_gap(#[case] effect: &str, #[case] trigger: &str) {
    let mut text = chancellor(effect, trigger);
    if effect.contains("Permanent") {
        text = text.replace(" | OneOff$ True", "");
    }
    assert_ne!(gaps(&text), Vec::<String>::new());
}

fn turned_up(line: &str, part: &str) -> String {
    format!("Name:T\nTypes:Creature\nR:Event$ TurnFaceUp | {line} | Description$ T.\n{part}")
}

#[rstest]
#[case::counters(
    "ValidCard$ Card.Self | ReplaceWith$ Add | ActiveZones$ Battlefield",
    "SVar:Add:DB$ PutCounter | Defined$ Self | CounterNum$ 5 | CounterType$ P1P1\n",
    json!({"Replacement": {"when": {"TurnedFaceUp": {"what": ["This"]}}, "by": {"WithCounters": {"counter": "+1/+1", "count": 5}}}})
)]
#[case::opponents_on_your_turn(
    "ValidCard$ Permanent.OppCtrl | Layer$ CantHappen | ActiveZones$ Battlefield | PlayerTurn$ True",
    "",
    json!({"Static": {"affected": {"Each": [{"ControlledBy": "Opponents"}]}, "mods": ["CantBeTurnedFaceUp"], "condition": "YourTurn"}})
)]
#[case::enchanted_face_down(
    "ValidCard$ Card.EnchantedBy+faceDown | Layer$ CantHappen | ActiveZones$ Battlefield",
    "",
    json!({"Static": {"affected": "Attached", "mods": ["CantBeTurnedFaceUp"]}})
)]
fn turning_face_up_reads_as_counters_or_a_restriction(
    #[case] line: &str,
    #[case] part: &str,
    #[case] expected: Value,
) {
    let text = turned_up(line, part);
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(abilities(&text)[0], expected);
}

#[rstest]
#[case::attach(
    "ValidCard$ Card.Self | ReplaceWith$ Add",
    "SVar:Add:DB$ Attach | Choices$ Creature | Object$ Self\n"
)]
#[case::another_card(
    "ValidCard$ Creature.Other | ReplaceWith$ Add",
    "SVar:Add:DB$ PutCounter | Defined$ Self | CounterNum$ 1 | CounterType$ P1P1\n"
)]
#[case::other_zone(
    "ValidCard$ Card.Self | ReplaceWith$ Add | ActiveZones$ Exile",
    "SVar:Add:DB$ PutCounter | Defined$ Self | CounterNum$ 1 | CounterType$ P1P1\n"
)]
fn turning_face_up_of_another_shape_is_a_gap(#[case] line: &str, #[case] part: &str) {
    assert_ne!(gaps(&turned_up(line, part)), Vec::<String>::new());
}

#[rstest]
#[case::every_player(None, json!({"NoLegendRule": "EachPlayer"}))]
#[case::yours(Some("Permanent.YouCtrl"), json!({"NoLegendRule": "You"}))]
#[case::your_creatures(Some("Creature.YouCtrl"), json!({"NoLegendRuleFor": [{"Type": "Creature"}, {"ControlledBy": "You"}]}))]
#[case::your_tokens(Some("Permanent.token+YouCtrl"), json!({"NoLegendRuleFor": ["Token", {"ControlledBy": "You"}]}))]
fn the_legend_rule_stops_for_a_selector(#[case] valid: Option<&str>, #[case] rule: Value) {
    let valid = valid.map_or_else(String::new, |valid| format!(" | ValidCard$ {valid}"));
    let text =
        format!("Name:T\nTypes:Creature\nS:Mode$ IgnoreLegendRule{valid} | Description$ T.\n");
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(abilities(&text)[0], json!({"Rule": {"rule": rule}}));
}

#[rstest]
#[case::the_placeholder("Number$0", Some(json!("LifeLostThisWay")))]
#[case::a_real_amount("TargetedPlayer$LifeTotal/HalfUp", None)]
fn the_life_lost_this_way_reads_the_placeholder_only(
    #[case] value: &str,
    #[case] expected: Option<Value>,
) {
    let text = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ LoseLife | Defined$ Player.Opponent | LifeAmount$ 2 | SubAbility$ Gain\nSVar:Gain:DB$ GainLife | Defined$ You | LifeAmount$ AFLifeLost\nSVar:AFLifeLost:{value}\n"
    );
    let found = abilities(&text);
    let gained = found
        .first()
        .and_then(|spell| spell["Spell"]["effects"].get(1).cloned());
    match expected {
        Some(amount) => {
            assert_eq!(gaps(&text), Vec::<String>::new());
            assert_eq!(gained, Some(json!({"GainLife": {"amount": amount}})));
        }
        None => assert_ne!(
            gained,
            Some(json!({"GainLife": {"amount": "LifeLostThisWay"}}))
        ),
    }
}

#[test]
fn encore_with_the_cards_own_x_is_a_gap() {
    let text = "Name:T\nTypes:Creature\nS:Mode$ Continuous | EffectZone$ Battlefield | AffectedZone$ Graveyard | Affected$ Creature.YouCtrl | AddKeyword$ Encore:X:XAlternative$ Number$ConvertedManaCost | Description$ T.\n";
    assert!(
        gaps(text).contains(&"keyword:Encore-with-fixed-x".to_owned()),
        "{:?}",
        gaps(text)
    );
}
