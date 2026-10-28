//! Goal: replacements whose steps name the event they replace ("that player", "that much", the source of the damage, the creature that would die) and "tap or untap" read into the DSL, write back as the same text, and leave every near-miss to the table.
use super::*;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::trigger::ReplaceBy;

#[rstest]
#[case::an_opponent_loses_instead(
    "If an opponent would gain life, that player loses that much life instead.",
    r#"{"Replacement":{"when":{"LifeGain":{"who":"Opponents"}},"by":{"Instead":[{"LoseLife":{"who":"Triggering","amount":"Triggering"}}]}}}"#
)]
#[case::a_player_loses_instead(
    "If a player would gain life, that player loses that much life instead.",
    r#"{"Replacement":{"when":{"LifeGain":{"who":"EachPlayer"}},"by":{"Instead":[{"LoseLife":{"who":"Triggering","amount":"Triggering"}}]}}}"#
)]
#[case::damage_to_you_goes_to_the_host(
    "All damage that would be dealt to you is dealt to enchanted creature instead.",
    r#"{"Replacement":{"when":{"Damage":{"to":"You"}},"by":{"Instead":[{"DealDamage":{"source":"Dealer","to":"Attached","amount":"Triggering"}}]}}}"#
)]
#[case::damage_to_you_goes_to_the_card(
    "All damage that would be dealt to you is dealt to this creature instead.",
    r#"{"Replacement":{"when":{"Damage":{"to":"You"}},"by":{"Instead":[{"DealDamage":{"source":"Dealer","to":"This","amount":"Triggering"}}]}}}"#
)]
#[case::a_death_feeds_the_card(
    "If a creature an opponent controls would die, instead exile it and put a number of +1/+1 counters equal to that creature's power on this creature.",
    r#"{"Replacement":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"Opponents"}],"from":"Battlefield","to":"Graveyard"}},"by":{"Instead":[{"Exile":{"what":"Triggering"}},{"PutCounters":{"on":"This","counter":"+1/+1","count":{"Stat":{"of":"Triggering","stat":"Power"}}}}]}}}"#
)]
#[case::combat_damage_becomes_counters_and_a_mill(
    "If this creature would deal combat damage to a player, instead put that many +1/+1 counters on this creature and that player mills that many cards.",
    r#"{"Replacement":{"when":{"Damage":{"to":"EachPlayer","source":["This"],"combat":true}},"by":{"Instead":[{"PutCounters":{"on":"This","counter":"+1/+1","count":"Triggering"}},{"Mill":{"who":"Triggering","count":"Triggering"}}]}}}"#
)]
fn a_replacement_that_names_its_event_reads_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[test]
fn a_death_that_makes_a_token_reads_back() {
    let read = golden(
        "If a nontoken creature an opponent controls would die, instead exile that card and create a 2/2 black Zombie creature token.",
        ClauseKind::Static,
        false,
    );
    let Ability::Replacement {
        by: ReplaceBy::Instead(effects),
        ..
    } = read
    else {
        panic!("a replacement: {read:?}");
    };
    assert!(matches!(
        effects.as_slice(),
        [
            Effect::Exile {
                what: Ref::Triggering,
                ..
            },
            Effect::CreateToken { .. }
        ]
    ));
}

#[rstest]
#[case::a_spell(
    "Tap or untap target permanent.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":{"Object":[]}}],"effects":[{"TapOrUntap":{"what":{"Target":0}}}]}}"#
)]
#[case::a_trigger(
    "At the beginning of each upkeep, you may tap or untap target permanent.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep"}},"targets":[{"what":{"Object":[]}}],"effects":[{"May":{"effects":[{"TapOrUntap":{"what":{"Target":0}}}]}}]}}"#
)]
fn tap_or_untap_reads_back(#[case] text: &str, #[case] kind: ClauseKind, #[case] json: &str) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::you_gain("If you would gain life, that player loses that much life instead.")]
#[case::twice_as_much(
    "If an opponent would gain life, that player loses twice that much life instead."
)]
#[case::to_an_opponent(
    "All damage that would be dealt to an opponent is dealt to enchanted creature instead."
)]
#[case::not_instead("All damage that would be dealt to you is dealt to enchanted creature.")]
#[case::noncombat(
    "If this creature would deal damage to a player, instead put that many +1/+1 counters on this creature and that player mills that many cards."
)]
#[case::return_instead(
    "If a creature an opponent controls would die, instead return it to its owner's hand and draw a card."
)]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
