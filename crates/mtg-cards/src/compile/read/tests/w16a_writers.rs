//! Goal: the writers of the shapes the stored local-model proposals hit (r4d-d, r4b-d) give the oracle wording of the ability, and a changed clause does not read back.
use super::*;

/// The wordings the checker accepts for the rules, for the message of a failed golden.
fn wordings(json: &str) -> Vec<String> {
    written_variants(&ability(json))
}

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    wordings(json)
        .iter()
        .any(|wording| same_text(wording, clause))
}

const RAID: &str = r#"{"Replacement":{"by":{"WithCounters":{"count":1,"counter":"+1/+1"}},"when":{"Enters":{"what":["This"]}},"condition":{"Compare":{"left":{"Tallied":{"what":"Attackers","who":"You"}},"op":"Gt","right":0}}}}"#;
const MORBID: &str = r#"{"Replacement":{"by":{"WithCounters":{"count":2,"counter":"+1/+1"}},"when":{"Enters":{"what":["This"]}},"condition":{"Compare":{"left":{"Tallied":{"what":"CreaturesDied","who":"EachPlayer"}},"op":"Ge","right":1}}}}"#;
const RAID_TWO: &str = r#"{"Replacement":{"by":{"WithCounters":{"count":1,"counter":"+1/+1"}},"when":{"Enters":{"what":["This"]}},"condition":{"Compare":{"left":{"Tallied":{"what":"Attackers","who":"You"}},"op":"Gt","right":1}}}}"#;
const SPECTER: &str = r#"{"Triggered":{"effects":[{"Discard":{"who":"Defending"}}],"when":{"Attacks":{"what":["This"]}}}}"#;
const SLIVER: &str = r#"{"Triggered":{"effects":[{"LoseLife":{"amount":1,"who":"Defending"}}],"when":{"Attacks":{"what":[{"ControlledBy":"You"},{"Subtype":"Sliver"}]}}}}"#;
const SPECTER_EACH: &str = r#"{"Triggered":{"effects":[{"Discard":{"who":"Opponents"}}],"when":{"Attacks":{"what":["This"]}}}}"#;
const BROKEN_FALL: &str = r#"{"Activated":{"cost":[{"ReturnToHand":{"filter":["This"]}}],"effects":[{"Regenerate":{"what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const ATTUNEMENT: &str = r#"{"Activated":{"cost":[{"ReturnToHand":{"filter":[{"Type":"Enchantment"},{"Is":"This"}]}}],"effects":[{"Draw":{"count":3}},{"Discard":{"count":4}}]}}"#;
const RETURN_OTHER: &str = r#"{"Activated":{"cost":[{"ReturnToHand":{"filter":["Other",{"Type":"Creature"}]}}],"effects":[{"Regenerate":{"what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const DRAKE: &str = r#"{"Triggered":{"effects":[{"Choose":{"as":"returned","from":{"Each":[{"Type":"Creature"},{"AnyOf":[{"Color":"White"},{"Color":"Blue"}]},{"ControlledBy":"You"}]},"count":{}}},{"Move":{"to":"Hand","what":{"Bound":"returned"}}}],"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}}}}"#;
const DRAKE_BOTH: &str = r#"{"Triggered":{"effects":[{"Choose":{"as":"returned","from":{"Each":[{"Type":"Creature"},{"Color":"White"},{"Color":"Blue"},{"ControlledBy":"You"}]},"count":{}}},{"Move":{"to":"Hand","what":{"Bound":"returned"}}}],"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}}}}"#;
const PHYTOHYDRA: &str = r#"{"Replacement":{"by":{"Instead":[{"PutCounters":{"counter":"+1/+1","on":"This","count":"Triggering"}}]},"when":{"Damage":{"to":"This"}}}}"#;
const PHYTOHYDRA_ONE: &str = r#"{"Replacement":{"by":{"Instead":[{"PutCounters":{"counter":"+1/+1","on":"This"}}]},"when":{"Damage":{"to":"This"}}}}"#;
const MASTIFF: &str = r#"{"Activated":{"cost":[{"Mana":"{1}{R}"}],"effects":[{"Continuous":{"affected":{"Each":[{"ControlledBy":"You"},{"Named":"Pack Mastiff"},{"Type":"Creature"}]},"duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":1,"toughness":0}}]}}]}}"#;
const MASTIFF_ALL: &str = r#"{"Activated":{"cost":[{"Mana":"{1}{R}"}],"effects":[{"Continuous":{"affected":{"Each":[{"Named":"Pack Mastiff"},{"Type":"Creature"}]},"duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":1,"toughness":0}}]}}]}}"#;
const MISFORTUNE: &str = r#"{"Spell":{"effects":[{"Destroy":{"what":{"Target":0}}},{"GainLife":{"amount":4,"who":{"OwnerOf":{"Target":0}}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const MISFORTUNE_YOU: &str = r#"{"Spell":{"effects":[{"Destroy":{"what":{"Target":0}}},{"GainLife":{"amount":4}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const DOMINEER: &str = r#"{"Static":{"affected":"Attached","mods":[{"SetController":"You"}]}}"#;
const DOMINEER_PUMP: &str = r#"{"Static":{"affected":"Attached","mods":[{"SetController":"You"},{"ModifyPT":{"power":1,"toughness":1}}]}}"#;

#[rstest]
#[case::raid(RAID, "~ enters with a +1/+1 counter on it if you attacked this turn.")]
#[case::morbid(MORBID, "~ enters with two +1/+1 counters on it if a creature died this turn.")]
#[case::defending_discards(SPECTER, "Whenever ~ attacks, defending player discards a card.")]
#[case::defending_loses(
    SLIVER,
    "Whenever a Sliver you control attacks, defending player loses 1 life."
)]
#[case::return_cost(BROKEN_FALL, "Return ~ to its owner's hand: Regenerate target creature.")]
#[case::return_cost_then(
    ATTUNEMENT,
    "Return ~ to its owner's hand: Draw three cards, then discard four cards."
)]
#[case::two_colors(
    DRAKE,
    "When ~ enters, return a white or blue creature you control to its owner's hand."
)]
#[case::damage_counters(
    PHYTOHYDRA,
    "If damage would be dealt to ~, put that many +1/+1 counters on it instead."
)]
#[case::named_yours(
    MASTIFF,
    "{1}{R}: Each creature you control named Pack Mastiff gets +1/+0 until end of turn."
)]
#[case::owner(MISFORTUNE, "Destroy target creature. Its owner gains 4 life.")]
#[case::enchanted_control(DOMINEER, "You control enchanted artifact creature.")]
fn w16a_writer_gives_the_oracle_wording(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}: {:#?}", wordings(json));
}

#[rstest]
#[case::more_attackers(RAID_TWO, "~ enters with a +1/+1 counter on it if you attacked this turn.")]
#[case::wrong_count(MORBID, "~ enters with a +1/+1 counter on it if a creature died this turn.")]
#[case::each_opponent(SPECTER_EACH, "Whenever ~ attacks, defending player discards a card.")]
#[case::other_creature(RETURN_OTHER, "Return ~ to its owner's hand: Regenerate target creature.")]
#[case::both_colors(
    DRAKE_BOTH,
    "When ~ enters, return a white or blue creature you control to its owner's hand."
)]
#[case::one_counter(
    PHYTOHYDRA_ONE,
    "If damage would be dealt to ~, put that many +1/+1 counters on it instead."
)]
#[case::anyone_named(
    MASTIFF_ALL,
    "{1}{R}: Each creature you control named Pack Mastiff gets +1/+0 until end of turn."
)]
#[case::you_gain(MISFORTUNE_YOU, "Destroy target creature. Its owner gains 4 life.")]
#[case::control_and_pump(DOMINEER_PUMP, "You control enchanted artifact creature.")]
fn w16a_near_miss_does_not_read_back(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}: {:#?}", wordings(json));
}

#[test]
#[ignore]
fn w16a_dump() {
    let Ok(path) = std::env::var("W16A_DUMP") else {
        return;
    };
    let text = std::fs::read_to_string(path).unwrap();
    for line in text.lines() {
        let (name, json) = line.split_once('\t').unwrap();
        println!("== {name}");
        for wording in written_variants(&ability(json)).iter().take(6) {
            println!("   {wording}");
        }
    }
}
