//! Goal: the embedded map reads a Forge shape that our rules state another way as read (an optional trigger that pays or casts, an "unless" payment taken as a choice, an ability a static grants to its own card, a subtype added by a type line, strive, backup and gift, a source named by its nickname, a draw a replacement states, an effect that tracks an exile, the triggers on surveil, the monarch, a crew, counters and tokens, a draw paid as a cost, a free cast, a life lock, damage that costs no life, a skipped draw step, phasing, a class, the return of an exile, a card that enters under your control, a shuffle of cards sent to exile, a pump of the player and a partner text) and still reports the card whose rules lack it; the wider shapes of those names have no verb.
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
#[case::a_surveil_trigger_is_surveilled(
    "T:Mode$ Surveil | ValidPlayer$ You | Execute$ TrigDraw\nSVar:TrigDraw:DB$ Draw",
    json!([{ "Triggered": { "when": { "Surveilled": { "who": "You" } }, "effects": [{ "Draw": {} }] } }])
)]
#[case::a_monarch_trigger_is_becomes_monarch(
    "T:Mode$ BecomeMonarch | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ TrigDraw\nSVar:TrigDraw:DB$ Draw",
    json!([{ "Triggered": { "when": { "BecomesMonarch": { "who": "You" } }, "effects": [{ "Draw": {} }] } }])
)]
#[case::a_monarch_trigger_of_an_effect_is_the_end_of_the_exile(
    "A:SP$ Effect | Triggers$ Back | ForgetOnMoved$ Exile\nSVar:Back:Mode$ BecomeMonarch | TriggerZones$ Command | Execute$ TrigBack\nSVar:TrigBack:DB$ ChangeZoneAll | Origin$ Exile | Destination$ Battlefield",
    json!([{ "Spell": { "targets": [{ "what": "Player" }],
        "effects": [{ "Exile": { "what": { "Target": 0 }, "duration": "UntilOpponentBecomesMonarch" } }] } }])
)]
#[case::a_crew_trigger_is_becomes_crewed(
    "T:Mode$ BecomesCrewed | ValidCard$ Card.Self | Execute$ TrigDraw\nSVar:TrigDraw:DB$ Draw",
    json!([{ "Triggered": { "when": { "BecomesCrewed": { "what": ["This"] } }, "effects": [{ "Draw": {} }] } }])
)]
#[case::the_first_crew_of_a_turn_is_the_nth_crew(
    "T:Mode$ BecomesCrewed | ValidCard$ Card.Self | FirstTimeCrewed$ True | Execute$ TrigDraw\nSVar:TrigDraw:DB$ Draw",
    json!([{ "Triggered": { "when": { "Nth": { "event": { "BecomesCrewed": { "what": ["This"] } }, "nth": 1 } },
        "effects": [{ "Draw": {} }] } }])
)]
#[case::counters_added_together_are_a_batch_of_counters_put(
    "T:Mode$ CounterTypeAddedAll | ValidObject$ Card.Self | Execute$ TrigDraw\nSVar:TrigDraw:DB$ Draw",
    json!([{ "Triggered": { "when": { "Batch": { "CountersPut": { "what": ["This"] } } }, "effects": [{ "Draw": {} }] } }])
)]
#[case::the_first_counters_of_a_turn_are_the_nth_counters_put(
    "T:Mode$ CounterTypeAddedAll | ValidObject$ Card.Self | FirstTime$ True | Execute$ TrigDraw\nSVar:TrigDraw:DB$ Draw",
    json!([{ "Triggered": { "when": { "Nth": { "event": { "CountersPut": { "what": ["This"] } }, "nth": 1 } },
        "effects": [{ "Draw": {} }] } }])
)]
#[case::the_first_tokens_of_a_turn_are_the_nth_token_created(
    "T:Mode$ TokenCreatedOnce | OnlyFirst$ You | Execute$ TrigDraw\nSVar:TrigDraw:DB$ Draw",
    json!([{ "Triggered": { "when": { "Nth": { "event": { "ZoneChange": { "what": ["Token", { "ControlledBy": "You" }], "to": "Battlefield" } }, "nth": 1 } },
        "effects": [{ "Draw": {} }] } }])
)]
#[case::a_draw_that_is_a_cost_is_an_optional_draw(
    "T:Mode$ Phase | Phase$ Upkeep | Execute$ TrigLoot\nSVar:TrigLoot:AB$ Discard | Defined$ You | Mode$ TgtChoose | NumCards$ 1 | Cost$ Draw<1/You>",
    json!([{ "Triggered": { "when": { "StepBegins": { "step": "Upkeep" } },
        "effects": [{ "May": { "effects": [{ "Draw": {} }] } }, { "If": { "condition": "Performed", "then": [{ "Discard": {} }] } }] } }])
)]
#[case::a_cast_for_nothing_is_a_free_cast(
    "S:Mode$ AlternativeCost | ValidSA$ Spell | ValidCard$ Card.Self | Cost$ 0 | EffectZone$ All\nA:SP$ Draw",
    json!([{ "FreeCast": {} }, { "Spell": { "effects": [{ "Draw": {} }] } }])
)]
#[case::a_life_total_that_cannot_change_is_a_rule(
    "S:Mode$ CantChangeLife | ValidPlayer$ You",
    json!([{ "Rule": { "rule": { "LifeTotalCantChange": "You" } } }])
)]
#[case::damage_that_costs_no_life_is_a_rule(
    "R:Event$ LifeReduced | ValidPlayer$ You | IsDamage$ True | Layer$ CantHappen",
    json!([{ "Rule": { "rule": { "NoLifeLossFromDamage": "You" } } }])
)]
#[case::a_skipped_draw_step_is_a_rule(
    "R:Event$ BeginPhase | ValidPlayer$ You | Phase$ Draw | Skip$ True",
    json!([{ "Rule": { "rule": { "SkipDrawStep": "You" } } }])
)]
#[case::phasing_out_is_the_effect_of_that_name(
    "A:AB$ Phases | Cost$ 1 W | Defined$ Self",
    json!([{ "Activated": { "cost": "{1}{W}", "effects": [{ "PhaseOut": { "what": "Attached" } }] } }])
)]
#[case::a_class_is_read_as_its_level_ups(
    "K:Class:2:2 U:AddStaticAbility$ Lv2",
    json!([{ "Activated": { "cost": "{2}{U}", "timing": "Sorcery", "effects": [{ "BecomeLevel": { "level": 2 } }] } }])
)]
#[case::the_return_of_an_exile_is_its_duration(
    "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigOut\nT:Mode$ ChangesZone | Origin$ Battlefield | Destination$ Any | ValidCard$ Card.Self | Execute$ TrigBack\nSVar:TrigOut:DB$ ChangeZone | ValidTgts$ Creature | Origin$ Battlefield | Destination$ Exile\nSVar:TrigBack:DB$ ChangeZone | Defined$ Remembered | Origin$ Exile | Destination$ Battlefield",
    json!([{ "Triggered": { "when": { "ZoneChange": { "what": ["This"], "to": "Battlefield" } },
        "targets": [{ "what": { "Object": [{ "Type": "Creature" }] } }],
        "effects": [{ "Exile": { "what": { "Target": 0 }, "duration": "WhileSourceOnBattlefield" } }] } }])
)]
#[case::the_return_of_the_card_exiled_by_name_is_its_duration(
    "A:SP$ ChangeZone | Origin$ Exile | Destination$ Battlefield | Defined$ DirectRemembered",
    json!([{ "Spell": { "targets": [{ "what": { "Object": [{ "Type": "Creature" }] } }],
        "effects": [{ "Exile": { "what": { "Target": 0 }, "duration": "WhileSourceOnBattlefield" } }] } }])
)]
#[case::a_card_that_enters_under_your_control_is_a_move(
    "A:SP$ ChangeZone | Origin$ Graveyard | Destination$ Battlefield | GainControl$ True | ValidTgts$ Creature",
    json!([{ "Spell": { "targets": [{ "what": { "Object": [{ "Type": "Creature" }, { "In": "Graveyard" }] } }],
        "effects": [{ "Move": { "what": { "Target": 0 }, "to": { "Battlefield": { "under": "You" } } } }] } }])
)]
#[case::a_shuffle_of_cards_sent_to_exile_is_an_exile(
    "A:SP$ ChangeZoneAll | Origin$ Graveyard | Destination$ Exile | ValidTgts$ Player | Shuffle$ True",
    json!([{ "Spell": { "targets": [{ "what": "Player" }],
        "effects": [{ "Exile": { "what": { "Each": [{ "In": "Graveyard" }] } } }] } }])
)]
#[case::a_pump_of_the_player_is_a_rule(
    "A:SP$ Pump | Defined$ You | KW$ Protection from everything | Duration$ UntilYourNextTurn",
    json!([{ "Spell": { "effects": [{ "Rule": { "rule": { "PlayersHave": { "who": "You", "keyword": { "Protection": [] } } },
        "duration": "UntilYourNextTurn" } }] } }])
)]
#[case::a_partner_text_is_a_partner_group(
    "K:Partner:Character select",
    json!([{ "Keyword": { "PartnerGroup": "Character select" } }])
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
#[case::a_surveil_trigger_is_not_a_scry_trigger(
    "T:Mode$ Surveil | ValidPlayer$ You",
    json!([{ "Triggered": { "when": { "SpellCast": { "by": "You" } }, "effects": [] } }])
)]
#[case::a_monarch_trigger_is_not_a_zone_change(
    "T:Mode$ BecomeMonarch | ValidPlayer$ You | TriggerZones$ Battlefield",
    json!([{ "Triggered": { "when": { "ZoneChange": { "what": ["This"], "to": "Battlefield" } }, "effects": [] } }])
)]
#[case::a_monarch_trigger_of_an_effect_needs_the_exile_or_the_trigger(
    "T:Mode$ BecomeMonarch | TriggerZones$ Command",
    json!([{ "Spell": { "effects": [{ "Draw": {} }] } }])
)]
#[case::the_first_crew_of_a_turn_is_not_every_crew(
    "T:Mode$ BecomesCrewed | ValidCard$ Card.Self | FirstTimeCrewed$ True",
    json!([{ "Triggered": { "when": { "BecomesCrewed": { "what": ["This"] } }, "effects": [] } }])
)]
#[case::the_first_counters_of_a_turn_are_not_every_counter(
    "T:Mode$ CounterTypeAddedAll | ValidObject$ Card.Self | FirstTime$ True",
    json!([{ "Triggered": { "when": { "Batch": { "CountersPut": { "what": ["This"] } } }, "effects": [] } }])
)]
#[case::the_first_tokens_of_a_turn_are_not_every_token(
    "T:Mode$ TokenCreatedOnce | OnlyFirst$ You",
    json!([{ "Triggered": { "when": { "Batch": { "ZoneChange": { "what": ["Token"], "to": "Battlefield" } } }, "effects": [] } }])
)]
#[case::a_draw_cost_needs_the_choice_to_pay(
    "A:AB$ Discard | Cost$ Draw<1/You> | Defined$ You | NumCards$ 1",
    json!([{ "Spell": { "effects": [{ "Draw": {} }, { "Discard": {} }] } }])
)]
#[case::a_free_cast_needs_the_free_cast(
    "S:Mode$ AlternativeCost | ValidSA$ Spell | Cost$ 0 | EffectZone$ All",
    json!([{ "Spell": { "effects": [{ "Draw": {} }] } }])
)]
#[case::a_life_lock_needs_the_rule(
    "S:Mode$ CantChangeLife | ValidPlayer$ You",
    json!([{ "Rule": { "rule": { "NoMaxHandSize": "You" } } }])
)]
#[case::damage_without_life_loss_needs_the_rule(
    "R:Event$ LifeReduced | ValidPlayer$ You | IsDamage$ True | Layer$ CantHappen",
    json!([{ "Rule": { "rule": { "NoMaxHandSize": "You" } } }])
)]
#[case::a_skipped_draw_step_needs_the_rule(
    "R:Event$ BeginPhase | Phase$ Draw | Skip$ True",
    json!([{ "Rule": { "rule": { "NoMaxHandSize": "You" } } }])
)]
#[case::phasing_needs_the_effect(
    "A:AB$ Phases | Cost$ 1 W | Defined$ Self",
    json!([{ "Activated": { "cost": "{1}{W}", "effects": [{ "Draw": {} }] } }])
)]
#[case::a_class_needs_its_level_ups(
    "K:Class:2:2 U:AddStaticAbility$ Lv2",
    json!([{ "Spell": { "effects": [{ "Draw": {} }] } }])
)]
#[case::the_return_of_an_exile_needs_a_move_or_an_exile(
    "A:SP$ ChangeZone | Defined$ Remembered | Origin$ Exile | Destination$ Battlefield",
    json!([{ "Spell": { "effects": [{ "Draw": {} }] } }])
)]
#[case::a_card_that_enters_under_your_control_needs_a_move(
    "A:SP$ ChangeZone | Origin$ Graveyard | Destination$ Battlefield | GainControl$ True | ValidTgts$ Creature",
    json!([{ "Spell": { "effects": [{ "Draw": {} }] } }])
)]
#[case::a_shuffle_into_the_library_needs_the_shuffle(
    "A:SP$ ChangeZoneAll | Origin$ Graveyard | Destination$ Library | Shuffle$ True",
    json!([{ "Spell": { "effects": [{ "Exile": { "what": { "Each": [{ "In": "Graveyard" }] } } }] } }])
)]
#[case::a_pump_of_the_player_needs_a_rule(
    "A:SP$ Pump | Defined$ You | KW$ Protection from everything",
    json!([{ "Spell": { "effects": [{ "Draw": {} }] } }])
)]
#[case::a_partner_text_needs_the_partner_keyword(
    "K:Partner:Character select",
    json!([{ "Keyword": "Flying" }])
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

#[rstest]
#[case::an_alternative_cost_that_is_not_free(
    "S:Mode$ AlternativeCost | ValidSA$ Spell.Self | Cost$ W U | EffectZone$ All"
)]
#[case::damage_that_stops_at_one_life(
    "R:Event$ LifeReduced | ValidPlayer$ You.lifeGE1 | Result$ LT1 | IsDamage$ True | ReplaceWith$ Stop"
)]
#[case::a_skipped_untap_step("R:Event$ BeginPhase | Phase$ Untap | Skip$ True")]
fn the_standard_map_has_no_verb_for_the_wider_shapes_of_those_names(#[case] script: &str) {
    let found = stand(script, &json!([]));
    let statuses: Vec<Status> = found.iter().map(|finding| finding.status).collect();
    assert_eq!(statuses, [Status::NoVerb], "{script}");
}
