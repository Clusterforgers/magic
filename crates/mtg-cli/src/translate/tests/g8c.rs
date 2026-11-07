//! Goal: facts of the triggering event and of the cast read as the engine holds them: who controls the card of a discard,
//! sacrifice, draw or land play, who tapped for mana, the mana spent to cast a spell (at least N of a type, any amount),
//! the card revealed as the only cost, "you and that player each draw" and a target read only when it is of a type.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn trigger(mode: &str, word: &str) -> String {
    format!(
        "Name:T\nTypes:Enchantment\nT:Mode$ {mode} | ValidCard$ Card | TriggerZones$ Battlefield | Execute$ TrigLose | TriggerDescription$ Whenever it happens, that player loses 1 life.\nSVar:TrigLose:DB$ LoseLife | Defined$ {word} | LifeAmount$ 1\n"
    )
}

#[rstest]
#[case::discard("Discarded", "TriggeredCardController", Some(json!("TriggeringPlayer")))]
#[case::cycle("Cycled", "TriggeredCardController", Some(json!("TriggeringPlayer")))]
#[case::sacrifice("Sacrificed", "TriggeredCardController", Some(json!("TriggeringPlayer")))]
#[case::land_play("LandPlayed", "TriggeredCardController", Some(json!("TriggeringPlayer")))]
#[case::draw("Drawn", "TriggeredCardController", Some(json!("Triggering")))]
#[case::mana("TapsForMana", "TriggeredActivator", Some(json!("Triggering")))]
// A discard names no activator, and a tap of a permanent not for mana names no player who tapped it.
#[case::discard_activator("Discarded", "TriggeredActivator", None)]
#[case::untap_activator("Untaps", "TriggeredActivator", None)]
fn g8c_the_controller_of_the_event_card_is_the_player_of_the_event(
    #[case] mode: &str,
    #[case] word: &str,
    #[case] who: Option<Value>,
) {
    let text = trigger(mode, word);
    let Some(who) = who else {
        assert!(
            gaps(&text).iter().any(|gap| gap.contains(word)),
            "{:?}",
            gaps(&text)
        );
        return;
    };
    assert_eq!(gaps(&text), Vec::<String>::new());
    let effects = &abilities(&text)[0]["Triggered"]["effects"];
    assert_eq!(effects[0]["LoseLife"]["who"], who, "{effects}");
}

fn cast_trigger(valid: &str) -> String {
    format!(
        "Name:T\nTypes:Creature\nPT:1/1\nT:Mode$ SpellCast | ValidCard$ Card.nonCreature | ValidActivatingPlayer$ You | ValidSA$ {valid} | TriggerZones$ Battlefield | Execute$ TrigDraw | TriggerDescription$ Whenever you cast a noncreature spell, draw a card.\nSVar:TrigDraw:DB$ Draw\n"
    )
}

#[rstest]
#[case::at_least_four("Spell.ManaSpent GE4", Some(("Ge", 4)))]
#[case::none_spent("Spell.ManaSpent EQ0", Some(("Eq", 0)))]
// A named bound is read against the card in some scripts and the spell in others: a gap.
#[case::named_bound("Spell.ManaSpent LTX", None)]
fn g8c_the_mana_spent_to_cast_a_spell_is_the_condition_of_the_cast_trigger(
    #[case] valid: &str,
    #[case] expected: Option<(&str, i64)>,
) {
    let text = cast_trigger(valid);
    let Some((op, right)) = expected else {
        assert_ne!(gaps(&text), Vec::<String>::new());
        return;
    };
    assert_eq!(gaps(&text), Vec::<String>::new());
    let triggered = &abilities(&text)[0]["Triggered"];
    assert_eq!(
        triggered["if"],
        json!({"Compare": {"left": {"Stat": {"of": "Triggering", "stat": "ManaSpent"}}, "op": op, "right": right}})
    );
    let what = triggered["when"]["SpellCast"]["what"].to_string();
    assert!(!what.contains("ManaSpent"), "{what}");
}

fn adamant(check: &str) -> String {
    format!(
        "Name:T\nTypes:Creature\nPT:1/1\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | CheckSVar$ {check} | ValidCard$ Card.Self | Execute$ TrigDraw | TriggerDescription$ When this creature enters, draw a card.\nSVar:TrigDraw:DB$ Draw\n"
    )
}

#[rstest]
#[case::two_white(
    "CastSA>Count$Adamant_2.White.2.0",
    Some(json!({"Compare": {"left": {"ManaSpent": {"of": "White"}}, "op": "Ge", "right": 2}}))
)]
#[case::three_red(
    "CastSA>Count$Adamant.Red.1.0",
    Some(json!({"Compare": {"left": {"ManaSpent": {"of": "Red"}}, "op": "Ge", "right": 3}}))
)]
#[case::no_colorless(
    "CastSA>Count$Adamant_1.Colorless.0.1",
    Some(json!({"Not": {"ManaSpent": "Colorless"}}))
)]
// An X paid is no mana spent of a type: it stays a gap.
#[case::x_paid("CastSA>Count$xPaid", None)]
fn g8c_mana_of_a_type_spent_to_cast_the_card_is_its_condition(
    #[case] check: &str,
    #[case] condition: Option<Value>,
) {
    let text = adamant(check);
    let Some(condition) = condition else {
        assert_ne!(gaps(&text), Vec::<String>::new());
        return;
    };
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(abilities(&text)[0]["Triggered"]["if"], condition);
}

#[rstest]
#[case::only_reveal(
    "As an additional cost to cast this spell, reveal a creature card from your hand.\\nYou gain life equal to the revealed card's mana value.",
    true
)]
// With a sacrifice as well the cost picks two cards: which one was revealed is not known.
#[case::reveal_and_sacrifice(
    "As an additional cost to cast this spell, reveal a creature card from your hand and sacrifice a land.\\nYou gain life equal to the revealed card's mana value.",
    false
)]
fn g8c_the_revealed_card_is_what_the_cost_picked_when_it_picks_nothing_else(
    #[case] oracle: &str,
    #[case] read: bool,
) {
    let text = format!(
        "Name:T\nManaCost:1 G\nTypes:Sorcery\nA:SP$ GainLife | Cost$ 1 G Reveal<1/Creature> | LifeAmount$ X\nSVar:X:Revealed$CardManaCost\nOracle:{oracle}\n"
    );
    if !read {
        assert!(
            gaps(&text).contains(&"amount:Revealed".to_owned()),
            "{:?}",
            gaps(&text)
        );
        return;
    }
    assert_eq!(gaps(&text), Vec::<String>::new());
    let all = json!(abilities(&text)).to_string();
    let wanted = json!({"Stat": {"of": "Paid", "stat": "ManaValue"}}).to_string();
    assert!(all.contains(&wanted), "{all}");
}

#[rstest]
#[case::target_opponent(
    "A:SP$ Draw | NumCards$ 2 | ValidTgts$ Opponent | Defined$ TargetedAndYou",
    Some(json!({"Target": 0}))
)]
#[case::damaged_player(
    "T:Mode$ DamageDone | ValidSource$ Card.Self | ValidTarget$ Player | CombatDamage$ True | Execute$ TrigDraw | TriggerDescription$ Whenever this deals combat damage to a player, you and that player each draw two cards.\nSVar:TrigDraw:DB$ Draw | NumCards$ 2 | Defined$ You & TriggeredTarget",
    Some(json!("Triggering"))
)]
// Damage to a creature or a player: the one dealt damage may not be a player.
#[case::damaged_anything(
    "T:Mode$ DamageDone | ValidSource$ Card.Self | ValidTarget$ Creature,Player | Execute$ TrigDraw | TriggerDescription$ Whenever this deals damage, you and that player each draw two cards.\nSVar:TrigDraw:DB$ Draw | NumCards$ 2 | Defined$ You & TriggeredTarget",
    None
)]
// A target creature draws nothing.
#[case::target_creature(
    "A:SP$ Draw | NumCards$ 2 | ValidTgts$ Creature | Defined$ TargetedAndYou",
    None
)]
fn g8c_you_and_another_player_each_draw(#[case] line: &str, #[case] other: Option<Value>) {
    let text = format!("Name:T\nTypes:Creature\nPT:1/1\n{line}\n");
    let Some(other) = other else {
        assert_ne!(gaps(&text), Vec::<String>::new());
        return;
    };
    assert_eq!(gaps(&text), Vec::<String>::new());
    let ability = &abilities(&text)[0];
    let effects = ability
        .get("Spell")
        .or_else(|| ability.get("Triggered"))
        .map(|inner| inner["effects"].clone())
        .unwrap_or_default();
    assert_eq!(
        effects,
        json!([
            {"Draw": {"count": 2}},
            {"Draw": {"who": other, "count": 2}},
        ])
    );
}

#[rstest]
#[case::creature("Targeted.Creature", Some("Creature"))]
#[case::artifact("Targeted.Artifact", Some("Artifact"))]
// A word that is no card type stays a gap.
#[case::other("Targeted.Opponent", None)]
fn g8c_the_target_of_a_type_is_the_target_only_while_it_has_that_type(
    #[case] word: &str,
    #[case] kind: Option<&str>,
) {
    let text = format!(
        "Name:T\nTypes:Instant\nA:SP$ DealDamage | ValidTgts$ Any | NumDmg$ 2 | SubAbility$ DBPump\nSVar:DBPump:DB$ Pump | Defined$ {word} | KW$ Flying\n"
    );
    let Some(kind) = kind else {
        assert_ne!(gaps(&text), Vec::<String>::new());
        return;
    };
    assert_eq!(gaps(&text), Vec::<String>::new());
    let effects = abilities(&text)[0]["Spell"]["effects"].to_string();
    let wanted = json!({"Each": [{"Is": {"Target": 0}}, {"Type": kind}]}).to_string();
    assert!(effects.contains(&wanted), "{effects}");
}

#[rstest]
#[case::entering_and_self("TriggeredCardLKICopy & Self", "", Some([json!("Triggering"), json!("This")]))]
// A target beside the pair is no one put on each.
#[case::with_a_target("TriggeredCardLKICopy & Self", " | ValidTgts$ Creature", None)]
// A player has no counters put this way.
#[case::a_player("TriggeredCardLKICopy & You", "", None)]
fn g8c_a_counter_on_each_of_two_named_objects(
    #[case] word: &str,
    #[case] more: &str,
    #[case] on: Option<[Value; 2]>,
) {
    let text = format!(
        "Name:T\nTypes:Creature\nPT:1/1\nT:Mode$ ChangesZone | Destination$ Battlefield | ValidCard$ Creature.Other+YouCtrl | TriggerZones$ Battlefield | Execute$ TrigPut | TriggerDescription$ Whenever another creature you control enters, put a +1/+1 counter on that creature and a +1/+1 counter on CARDNAME.\nSVar:TrigPut:DB$ PutCounter | Defined$ {word} | CounterType$ P1P1{more}\n"
    );
    let Some(on) = on else {
        assert_ne!(gaps(&text), Vec::<String>::new());
        return;
    };
    assert_eq!(gaps(&text), Vec::<String>::new());
    let effects = &abilities(&text)[0]["Triggered"]["effects"];
    assert_eq!(effects[0]["PutCounters"]["on"], on[0], "{effects}");
    assert_eq!(effects[1]["PutCounters"]["on"], on[1], "{effects}");
}
