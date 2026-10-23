//! Goal: the embedded map reads a Forge shape that our rules state another way as read (an optional trigger that pays or casts, an "unless" payment taken as a choice, an ability a static grants to its own card, a subtype added by a type line, strive, backup and gift, a source named by its nickname, a draw a replacement states, an effect that tracks an exile) and still reports the card whose rules lack it.
use std::collections::BTreeSet;

use mtg_cards::card::Card;
use rstest::rstest;
use serde_json::json;

use super::*;
use crate::forge::script::Script;

fn card(abilities: &serde_json::Value) -> Card {
    serde_json::from_value(json!({
        "oracle_id": "00000000-0000-4000-8000-000000000001",
        "name": "Test",
        "impl_status": "Full",
        "faces": [{ "name": "Test", "abilities": abilities }],
    }))
    .unwrap_or_else(|error| panic!("{error}"))
}

fn stand(script: &str, abilities: &serde_json::Value) -> Vec<Finding> {
    let map = Map::standard().unwrap_or_else(|error| panic!("{error}"));
    let items = Script::parse(&format!("Name:Test\n{script}")).items();
    let ours: BTreeSet<String> = verbs_of(&card(abilities));
    compare(&items, &ours, map)
}

const LEVEL_UP: &str = r#"{"Compare":{"left":{"Counters":{"on":"This","counter":{"Other":"level"}}},"op":"Ge","right":2}}"#;

fn bracket(ability: &str) -> serde_json::Value {
    let text = ability.replace("LEVEL", LEVEL_UP);
    let static_part =
        r#"{"Static":{"affected":"This","mods":[{"SetPT":{"power":2,"toughness":4}}]}}"#;
    serde_json::from_str(&format!("[{static_part},{text}]"))
        .unwrap_or_else(|error| panic!("{error}"))
}

#[rstest]
#[case::a_payment_is_the_choice_of_an_optional_trigger(
    "T:Mode$ SpellCast | OptionalDecider$ You | Execute$ TrigPay\nSVar:TrigPay:AB$ Draw | Cost$ 1",
    json!([{ "Triggered": { "when": { "SpellCast": { "by": "You" } },
        "effects": [{ "Pay": { "cost": "{1}", "paid": [{ "Draw": {} }] } }] } }])
)]
#[case::a_cast_is_the_choice_of_an_optional_trigger(
    "T:Mode$ DamageDone | OptionalDecider$ You | Execute$ TrigPlay\nSVar:TrigPlay:DB$ Play | Optional$ True",
    json!([{ "Triggered": { "when": { "DealsDamage": { "source": ["This"], "to": "EachPlayer", "combat": true } },
        "effects": [{ "CastFromZone": { "what": { "Each": [{ "In": "Exile" }, { "ExiledWith": "This" }] }, "free": true } }] } }])
)]
#[case::an_unless_payment_is_a_choice_to_discard(
    "A:SP$ Draw | UnlessCost$ Discard<1/Card>",
    json!([{ "Spell": { "effects": [
        { "May": { "who": "You", "effects": [{ "Discard": {} }] } },
        { "If": { "condition": "Performed", "then": [{ "Draw": {} }] } }] } }])
)]
#[case::a_bracket_grants_an_activated_ability(
    "S:Mode$ Continuous | AffectedDefined$ Self | SetPower$ 2 | SetToughness$ 4 | AddAbility$ Ab | IsPresent$ Card.Self\nSVar:Ab:AB$ Draw | Cost$ T",
    bracket(r#"{"Activated":{"cost":["Tap"],"condition":LEVEL,"effects":[{"Draw":{}}]}}"#)
)]
#[case::a_bracket_grants_a_triggered_ability(
    "S:Mode$ Continuous | AffectedDefined$ Self | SetPower$ 2 | SetToughness$ 4 | AddTrigger$ Tr\nSVar:Tr:Mode$ SpellCast | Execute$ TrigDraw\nSVar:TrigDraw:DB$ Draw",
    bracket(
        r#"{"Triggered":{"when":{"SpellCast":{"by":"You"}},"if":LEVEL,"effects":[{"Draw":{}}]}}"#
    )
)]
#[case::a_bracket_grants_a_static_ability(
    "S:Mode$ Continuous | AffectedDefined$ Self | SetPower$ 2 | SetToughness$ 4 | AddStaticAbility$ St\nSVar:St:Mode$ Continuous | AffectedDefined$ Self | AddPower$ 1",
    bracket(r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":1,"toughness":0}}],"condition":LEVEL}}"#)
)]
#[case::a_type_line_adds_a_subtype(
    "S:Mode$ Continuous | Affected$ Creature.EnchantedBy | AddType$ Angel",
    json!([{ "Static": { "affected": "Attached", "mods": [{ "AddSubtype": "Angel" }] } }])
)]
#[case::losing_the_creature_types_is_their_replacement_by_none(
    "S:Mode$ Continuous | Affected$ Creature | AddType$ Creature | RemoveCreatureTypes$ True",
    json!([{ "Static": { "affected": "This", "mods": [{ "SetCreatureTypes": [] }] } }])
)]
#[case::strive_is_a_cost_modifier_of_the_spell(
    "K:Strive:2 G",
    json!([{ "CostModifier": { "applies_to": ["This"], "change": "Increase", "mana": "{2}{G}",
        "per": { "Sum": [{ "TargetCount": 0 }, -1] } } }])
)]
#[case::backup_holds_its_ability(
    "K:Backup:1:BackupAb\nSVar:BackupAb:DB$ Pump | KW$ Double Strike",
    json!([{ "Keyword": { "Backup": { "count": 1, "abilities": [{ "Keyword": "DoubleStrike" }] } } }])
)]
#[case::gift_holds_its_ability(
    "K:Gift\nSVar:GiftAbility:DB$ Token | TokenScript$ u_1_1_fish | TokenOwner$ Promised",
    json!([{ "Keyword": { "Gift": "TappedFish" } }])
)]
#[case::the_nickname_is_the_source_of_a_cost(
    "A:AB$ Draw | Cost$ 1 Sac<1/NICKNAME>",
    json!([{ "Activated": { "cost": [{ "Mana": "{1}" }, "SacrificeSelf"], "effects": [{ "Draw": {} }] } }])
)]
#[case::a_draw_a_replacement_states_is_the_multiplier(
    "R:Event$ Draw | ReplaceWith$ Twice\nSVar:Twice:DB$ Draw | NumCards$ 2",
    json!([{ "Replacement": { "when": { "Draw": {} }, "by": { "Multiply": 2 } } }])
)]
#[case::an_effect_that_tracks_an_exile_is_the_duration_of_the_exile(
    "A:SP$ Effect | ForgetOnMoved$ Exile",
    json!([{ "Spell": { "targets": [{ "what": "Player" }],
        "effects": [{ "Exile": { "what": { "Target": 0 }, "duration": "UntilOpponentBecomesMonarch" } }] } }])
)]
fn the_standard_map_reads_a_shape_our_rules_state_another_way(
    #[case] script: &str,
    #[case] abilities: serde_json::Value,
) {
    let found = stand(script, &abilities);
    assert!(!found.is_empty(), "{script}");
    let missing: Vec<_> = found
        .iter()
        .filter(|finding| finding.status != Status::Read)
        .map(|finding| (&finding.name, &finding.key, &finding.expect))
        .collect();
    assert!(missing.is_empty(), "{script}: {missing:?}");
}

#[rstest]
#[case::an_optional_trigger_with_no_choice(
    "T:Mode$ SpellCast | OptionalDecider$ You | Execute$ TrigDraw\nSVar:TrigDraw:DB$ Draw",
    json!([{ "Triggered": { "when": { "SpellCast": { "by": "You" } }, "effects": [{ "Draw": {} }] } }])
)]
#[case::a_grant_to_other_cards_is_a_grant(
    "S:Mode$ Continuous | Affected$ Creature.YouCtrl | AddAbility$ Ab\nSVar:Ab:AB$ Draw | Cost$ T",
    json!([{ "Static": { "affected": "This", "mods": [{ "AddKeyword": "Flying" }] } },
        { "Activated": { "cost": ["Tap"], "effects": [{ "Draw": {} }] } }])
)]
#[case::a_draw_of_an_ability_is_not_a_replacement(
    "A:SP$ Draw | NumCards$ 2",
    json!([{ "Replacement": { "when": { "Draw": {} }, "by": { "Multiply": 2 } } }])
)]
#[case::a_pump_beside_a_backup_keyword_is_an_ability_of_its_own(
    "A:SP$ Pump | KW$ Double Strike\nK:Backup:1:BackupAb\nSVar:BackupAb:DB$ Pump | KW$ Double Strike",
    json!([{ "Keyword": { "Backup": { "count": 1, "abilities": [{ "Keyword": "DoubleStrike" }] } } }])
)]
#[case::a_sacrifice_of_another_permanent_is_not_the_source(
    "A:AB$ Draw | Cost$ 1 Sac<1/Creature>",
    json!([{ "Activated": { "cost": [{ "Mana": "{1}" }, "SacrificeSelf"], "effects": [{ "Draw": {} }] } }])
)]
fn the_standard_map_still_reports_a_card_whose_rules_lack_the_shape(
    #[case] script: &str,
    #[case] abilities: serde_json::Value,
) {
    let found = stand(script, &abilities);
    assert!(
        found
            .iter()
            .any(|finding| finding.status == Status::Missing),
        "{script}: {found:?}"
    );
}
