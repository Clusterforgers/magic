//! Goal: the other creature of a block filters a trigger once per pair ("blocks a creature with flying", "becomes
//! blocked by an artifact creature") or once as a filter of the block itself ("one or more black creatures"); a printed
//! "you may search" opening the effect asks for the whole effect; a decider of the event's player asks that player; a
//! "your graveyard" name pool of another player's cast is the caster's; other shapes stay named gaps.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn card(lines: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Bear\nPT:2/2\n{lines}SVar:TrigPump:DB$ Pump | Defined$ Self | NumAtt$ +2\n"
    )
}

fn when_and_if(script: &str) -> (Value, Value) {
    let found = abilities(script);
    let trigger = found
        .iter()
        .find_map(|ability| ability.get("Triggered"))
        .unwrap_or_else(|| panic!("no trigger: {:?}", gaps(script)));
    (
        trigger["when"].clone(),
        trigger.get("if").cloned().unwrap_or(Value::Null),
    )
}

#[rstest]
#[case::blocks_a_flyer(
    "T:Mode$ Blocks | ValidCard$ Card.Self | ValidBlocked$ Creature.withFlying | Execute$ TrigPump | TriggerDescription$ Whenever CARDNAME blocks a creature with flying, CARDNAME gets +2/+0 until end of turn.\n",
    json!({"BecomesBlocked": {"what": [{"Type": "Creature"}, {"HasKeyword": "Flying"}], "by": ["This"]}}),
    Value::Null
)]
#[case::blocked_by_an_artifact(
    "T:Mode$ AttackerBlocked | ValidCard$ Card.Self | ValidBlocker$ Creature.Artifact | Execute$ TrigPump | TriggerDescription$ Whenever CARDNAME becomes blocked by an artifact creature, CARDNAME gets +2/+0 until end of turn.\n",
    json!({"BecomesBlocked": {"what": ["This"], "by": [{"Type": "Creature"}, {"Type": "Artifact"}]}}),
    Value::Null
)]
#[case::blocks_one_or_more(
    "T:Mode$ Blocks | ValidCard$ Card.Self | ValidBlocked$ Creature.Black | Execute$ TrigPump | TriggerDescription$ Whenever CARDNAME blocks one or more black creatures, CARDNAME gets +2/+0 until end of turn.\n",
    json!({"Blocks": {"what": ["This"], "blocking": [{"Type": "Creature"}, {"Color": "Black"}]}}),
    Value::Null
)]
#[case::either_side(
    "T:Mode$ Blocks | ValidCard$ Card.Self | ValidBlocked$ Creature.Black | Execute$ TrigPump | TriggerDescription$ Whenever CARDNAME blocks or becomes blocked by one or more black creatures, CARDNAME gets +2/+0 until end of turn.\nT:Mode$ AttackerBlocked | ValidCard$ Card.Self | ValidBlocker$ Creature.Black | Execute$ TrigPump | Secondary$ True | TriggerDescription$ Whenever CARDNAME blocks or becomes blocked by one or more black creatures, CARDNAME gets +2/+0 until end of turn.\n",
    json!({"AnyOf": [
        {"Blocks": {"what": ["This"], "blocking": [{"Type": "Creature"}, {"Color": "Black"}]}},
        {"BecomesBlocked": {"what": ["This"], "by": [{"Type": "Creature"}, {"Color": "Black"}], "once": true}}
    ]}),
    Value::Null
)]
#[case::blocked_from_the_blockers_side(
    "T:Mode$ Blocks | ValidCard$ Creature | ValidBlocked$ Card.Self | Execute$ TrigPump | TriggerDescription$ Whenever CARDNAME becomes blocked, CARDNAME gets +2/+0 until end of turn.\n",
    json!({"BecomesBlocked": {"what": ["This"]}}),
    Value::Null
)]
#[case::any_creature(
    "T:Mode$ Blocks | ValidCard$ Card.Self | ValidBlocked$ Creature | Execute$ TrigPump | TriggerDescription$ Whenever CARDNAME blocks, CARDNAME gets +2/+0 until end of turn.\n",
    json!({"Blocks": {"what": ["This"]}}),
    Value::Null
)]
#[case::blocked_by_any_creature(
    "T:Mode$ AttackerBlocked | ValidCard$ Card.Self | ValidBlocker$ Creature | Execute$ TrigPump | TriggerDescription$ Whenever CARDNAME becomes blocked, CARDNAME gets +2/+0 until end of turn.\n",
    json!({"BecomesBlocked": {"what": ["This"]}}),
    Value::Null
)]
fn the_other_creature_of_a_block_filters_the_trigger(
    #[case] lines: &str,
    #[case] when: Value,
    #[case] condition: Value,
) {
    assert_eq!(when_and_if(&card(lines)), (when, condition));
}

#[rstest]
#[case::another_subject(
    "T:Mode$ Blocks | ValidCard$ Creature.YouCtrl | ValidBlocked$ Creature.Black | Execute$ TrigPump | TriggerDescription$ Whenever a creature you control blocks one or more black creatures, CARDNAME gets +2/+0 until end of turn.\n",
    "key:Trigger.Blocks.ValidBlocked"
)]
#[case::unprinted_count(
    "T:Mode$ AttackerBlocked | ValidCard$ Card.Self | ValidBlocker$ Creature.Black | Execute$ TrigPump | TriggerDescription$ Whenever CARDNAME becomes blocked by black creatures, CARDNAME gets +2/+0 until end of turn.\n",
    "key:Trigger.BecomesBlocked.ValidBlocker"
)]
fn an_unknown_block_filter_stays_a_gap(#[case] lines: &str, #[case] gap: &str) {
    assert!(
        gaps(&card(lines)).iter().any(|found| found == gap),
        "{:?}",
        gaps(&card(lines))
    );
}

const SEARCH: &str = "SVar:TrigSearch:DB$ ChangeZone | Origin$ Library | Destination$ Hand | ChangeType$ Land.Basic | ChangeNum$ 1\n";

#[rstest]
#[case::opens_the_effect(
    "When CARDNAME enters, you may search your library for a basic land card, reveal it, put it into your hand, then shuffle.",
    true
)]
#[case::after_another_step(
    "When CARDNAME enters, draw a card. Then you may search your library for a basic land card, reveal it, put it into your hand, then shuffle.",
    false
)]
#[case::after_a_condition(
    "When CARDNAME enters, if you control a Forest, you may search your library for a basic land card, reveal it, put it into your hand, then shuffle.",
    false
)]
fn a_printed_may_search_opening_the_effect_asks(#[case] printed: &str, #[case] asks: bool) {
    let script = format!(
        "Name:T\nTypes:Creature Bear\nPT:2/2\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigSearch | TriggerDescription$ {printed}\n{SEARCH}"
    );
    if asks {
        let found = abilities(&script);
        let effects = &found[0]["Triggered"]["effects"];
        assert_eq!(effects.as_array().map(Vec::len), Some(1));
        assert!(effects[0].get("May").is_some(), "{effects}");
    } else {
        assert!(
            gaps(&script)
                .iter()
                .any(|gap| gap == "trigger:may-search-without-decider")
        );
    }
}

#[rstest]
#[case::dealer_controller(
    "T:Mode$ DamageDone | ValidSource$ Creature | ValidTarget$ Opponent | CombatDamage$ True | OptionalDecider$ TriggeredSourceController | TriggerZones$ Battlefield | Execute$ TrigDraw | TriggerDescription$ Whenever a creature deals combat damage to one of your opponents, its controller may draw a card.\nSVar:TrigDraw:DB$ Draw | Defined$ TriggeredSourceController | NumCards$ 1\n",
    json!({"ControllerOf": "Dealer"})
)]
#[case::dying_creatures_controller(
    "T:Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Creature | OptionalDecider$ TriggeredCardController | TriggerZones$ Battlefield | Execute$ TrigDraw | TriggerDescription$ Whenever a creature dies, that creature's controller may draw a card.\nSVar:TrigDraw:DB$ Draw | Defined$ TriggeredCardController | NumCards$ 1\n",
    json!({"ControllerOf": "Triggering"})
)]
fn the_events_player_decides(#[case] lines: &str, #[case] who: Value) {
    let script = format!("Name:T\nTypes:Enchantment\n{lines}");
    let found = abilities(&script);
    assert_eq!(
        found[0]["Triggered"]["effects"][0]["May"]["who"],
        who,
        "{:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::an_opponent_casts("Opponent", json!({"ControllerOf": "Candidate"}))]
#[case::you_cast("You", json!("You"))]
fn a_graveyard_name_pool_is_the_casters(#[case] caster: &str, #[case] owner: Value) {
    let script = format!(
        "Name:T\nTypes:Creature Dragon\nPT:6/4\nT:Mode$ SpellCast | ValidSAonCard$ Spell.Creature+sharesNameWith YourGraveyard | ValidActivatingPlayer$ {caster} | TriggerZones$ Battlefield | Execute$ TrigLoseLife | TriggerDescription$ Whenever a player casts a creature spell with the same name as a card in their graveyard, that player loses 10 life.\nSVar:TrigLoseLife:DB$ LoseLife | Defined$ TriggeredActivator | LifeAmount$ 10\n"
    );
    let (when, _) = when_and_if(&script);
    let pool = json!({"SharesName": {"Each": [{"In": "Graveyard"}, {"OwnedBy": owner}]}});
    let what = when["SpellCast"]["what"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(what.contains(&pool), "{when}");
}
