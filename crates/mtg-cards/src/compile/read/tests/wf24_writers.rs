//! Goal: the wf24 writers give the oracle wording of an activation trigger, whose ability is never a mana ability and
//! whose source of an object type is on the battlefield; another player, source, zone or a source anywhere never reads
//! back as the clause.
use super::*;

fn reads(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

#[test]
#[ignore = "probe"]
fn wf24_probe() {
    let text = std::fs::read_to_string("/tmp/claude-1000/-home-chris-private-Clusterforgers-magic/91019583-d986-44a1-b4cd-f75462831666/scratchpad/wf24/probe.json").unwrap();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let a = ability(line);
        println!("== {line}");
        for w in written_variants(&a).iter().take(60) {
            println!("  {w}");
        }
    }
}

fn activated(did: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"Did":{did}}},"effects":[{{"DealDamage":{{"to":"TriggeringPlayer","amount":2}}}}]}}}}"#
    )
}

const PERMANENTS: &str = r#"{"action":"Activated","who":"Opponents","what":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"},{"Type":"Land"}]},{"In":"Battlefield"}]}"#;
const HARSH: &str = "Whenever an opponent activates an ability of an artifact, creature, or land on the battlefield, if it isn't a mana ability, this creature deals 2 damage to that player.";
const IMMOLATION: &str = "Whenever an opponent activates an ability of an artifact, creature, or land that isn't a mana ability, this creature deals 2 damage to that player.";

#[rstest]
#[case::intervening_if(PERMANENTS, HARSH, true)]
#[case::that_isnt(PERMANENTS, IMMOLATION, true)]
#[case::any_player_for_opponent(&PERMANENTS.replace(r#""who":"Opponents","#, ""), IMMOLATION, false)]
#[case::you_for_opponent(&PERMANENTS.replace("Opponents", "You"), IMMOLATION, false)]
#[case::other_types(&PERMANENTS.replace("Land", "Enchantment"), IMMOLATION, false)]
#[case::anywhere(&PERMANENTS.replace(r#",{"In":"Battlefield"}"#, ""), IMMOLATION, false)]
#[case::in_a_graveyard(&PERMANENTS.replace(r#"{"In":"Battlefield"}"#, r#"{"In":"Graveyard"}"#), IMMOLATION, false)]
#[case::loyalty_only(&PERMANENTS.replace(r#""Activated""#, r#""ActivatedLoyalty""#), IMMOLATION, false)]
#[case::any_ability(
    r#"{"action":"Activated"}"#,
    "Whenever a player activates an ability that isn't a mana ability, this creature deals 2 damage to that player.",
    true
)]
#[case::any_ability_said_with_if(
    r#"{"action":"Activated","who":"You"}"#,
    "Whenever you activate an ability, if it isn't a mana ability, this creature deals 2 damage to that player.",
    true
)]
#[case::mana_abilities_counted(
    r#"{"action":"Activated"}"#,
    "Whenever a player activates an ability, this creature deals 2 damage to that player.",
    false
)]
#[case::a_permanent(
    r#"{"action":"Activated","who":"Opponents","what":[{"In":"Battlefield"}]}"#,
    "Whenever an opponent activates an ability of a permanent that isn't a mana ability, this creature deals 2 damage to that player.",
    true
)]
#[case::a_permanent_for_any_source(
    r#"{"action":"Activated","who":"Opponents"}"#,
    "Whenever an opponent activates an ability of a permanent that isn't a mana ability, this creature deals 2 damage to that player.",
    false
)]
#[case::equipped(
    r#"{"action":"Activated","what":[{"Type":"Creature"},{"Is":"Attached"},{"In":"Battlefield"}]}"#,
    "Whenever an ability of equipped creature is activated, if it isn't a mana ability, this creature deals 2 damage to that player.",
    true
)]
#[case::equipped_by_you_only(
    r#"{"action":"Activated","who":"You","what":[{"Type":"Creature"},{"Is":"Attached"},{"In":"Battlefield"}]}"#,
    "Whenever an ability of equipped creature is activated, if it isn't a mana ability, this creature deals 2 damage to that player.",
    false
)]
fn wf24_an_activation_is_never_of_a_mana_ability(
    #[case] did: &str,
    #[case] clause: &str,
    #[case] same: bool,
) {
    assert_eq!(reads(&activated(did), clause), same, "{clause}");
}

const ENTERS: &str = r#"{"ZoneChange":{"what":["This"],"to":"Battlefield"}}"#;
const OPPONENTS_CREATURE: &str =
    r#"[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}]"#;
const STUN: &str = r#"{"PutCounters":{"on":{"Target":0},"counter":{"Other":"stun"}}}"#;
const STUNNED: &str = "When this creature enters, tap target creature an opponent controls and put a stun counter on it.";

fn triggered(when: &str, targets: &str, effects: &str) -> String {
    format!(r#"{{"Triggered":{{"when":{when},"targets":{targets},"effects":[{effects}]}}}}"#)
}

#[rstest]
#[case::and_it(&triggered(ENTERS, OPPONENTS_CREATURE, &format!(r#"{{"Tap":{{"what":{{"Target":0}}}}}},{STUN}"#)), STUNNED, true)]
#[case::sentence_it(
    &triggered(ENTERS, OPPONENTS_CREATURE, &format!(r#"{{"Tap":{{"what":{{"Target":0}}}}}},{STUN}"#)),
    "When this creature enters, tap target creature an opponent controls. Put a stun counter on it.",
    true
)]
#[case::several(
    &triggered(ENTERS, r#"[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":2}}]"#, &format!(r#"{{"Tap":{{"what":{{"Target":0}}}}}},{STUN}"#)),
    "When this creature enters, tap up to two target creatures. Put a stun counter on each of them.",
    true
)]
#[case::untap_for_tap(&triggered(ENTERS, OPPONENTS_CREATURE, &format!(r#"{{"Untap":{{"what":{{"Target":0}}}}}},{STUN}"#)), STUNNED, false)]
#[case::counter_on_self(&triggered(ENTERS, OPPONENTS_CREATURE, r#"{"Tap":{"what":{"Target":0}}},{"PutCounters":{"on":"This","counter":{"Other":"stun"}}}"#), STUNNED, false)]
#[case::two_counters(&triggered(ENTERS, OPPONENTS_CREATURE, &format!(r#"{{"Tap":{{"what":{{"Target":0}}}}}},{}"#, STUN.replace("}}", r#"},"count":2}"#))), STUNNED, false)]
#[case::any_creature(&triggered(ENTERS, r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#, &format!(r#"{{"Tap":{{"what":{{"Target":0}}}}}},{STUN}"#)), STUNNED, false)]
fn wf24_a_target_named_once_is_it_after(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] same: bool,
) {
    assert_eq!(reads(json, clause), same, "{clause}");
}

const NINJA_ENTERS: &str =
    r#"{"ZoneChange":{"what":[{"Subtype":"Ninja"},{"ControlledBy":"You"}],"to":"Battlefield"}}"#;
const ROBOT: &str = r#"{"CreateToken":{"token":{"Custom":{"name":"Robot","types":["Artifact","Creature"],"subtypes":["Robot"],"power":2,"toughness":2}},"as":"token"}}"#;

#[rstest]
#[case::entering(
    &triggered(NINJA_ENTERS, "[]", r#"{"May":{"effects":[{"Attach":{"what":"This","to":"Triggering"}}]}}"#),
    "Whenever a Ninja you control enters, you may attach this Equipment to it.",
    true
)]
#[case::entering_must(
    &triggered(NINJA_ENTERS, "[]", r#"{"Attach":{"what":"This","to":"Triggering"}}"#),
    "Whenever a Ninja you control enters, you may attach this Equipment to it.",
    false
)]
#[case::entering_other_kind(
    &triggered(&NINJA_ENTERS.replace("Ninja", "Samurai"), "[]", r#"{"May":{"effects":[{"Attach":{"what":"This","to":"Triggering"}}]}}"#),
    "Whenever a Ninja you control enters, you may attach this Equipment to it.",
    false
)]
#[case::token_and(
    &triggered(ENTERS, "[]", &format!(r#"{ROBOT},{{"Attach":{{"what":"This","to":{{"Bound":"token"}}}}}}"#)),
    "When this Equipment enters, create a 2/2 colorless Robot artifact creature token and attach this Equipment to it.",
    true
)]
#[case::token_then(
    &triggered(ENTERS, "[]", &format!(r#"{{"Pay":{{"cost":"{{2}}","paid":[{ROBOT},{{"Attach":{{"what":"This","to":{{"Bound":"token"}}}}}}]}}}}"#)),
    "When this Equipment enters, you may pay {2}. If you do, create a 2/2 colorless Robot artifact creature token, then attach this Equipment to it.",
    true
)]
#[case::two_tokens(
    &triggered(ENTERS, "[]", &format!(r#"{},{{"Attach":{{"what":"This","to":{{"Bound":"token"}}}}}}"#, ROBOT.replace(r#""as""#, r#""count":2,"as""#))),
    "When this Equipment enters, create a 2/2 colorless Robot artifact creature token and attach this Equipment to it.",
    false
)]
#[case::other_bound(
    &triggered(ENTERS, "[]", &format!(r#"{},{{"Attach":{{"what":"This","to":{{"Bound":"other"}}}}}}"#, ROBOT.replace("token\"}", "other\"}"))),
    "When this Equipment enters, create a 2/2 colorless Robot artifact creature token and attach this Equipment to it.",
    false
)]
fn wf24_an_equipment_attaches_to_the_one_object(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] same: bool,
) {
    assert_eq!(reads(json, clause), same, "{clause}");
}

const ATTACKS: &str = r#"{"Attacks":{"what":["This"]}}"#;

fn battalion(what: &str, right: u32, op: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{what},"if":{{"AtTrigger":{{"Compare":{{"left":{{"Count":[{{"Type":"Creature"}},"Attacking","Other"]}},"op":"{op}","right":{right}}}}}}},"effects":[{{"Draw":{{}}}}]}}}}"#
    )
}

#[rstest]
#[case::two_others(&battalion(ATTACKS, 2, "Ge"), "Whenever this creature and at least two other creatures attack, draw a card.", true)]
#[case::one_other(&battalion(ATTACKS, 1, "Ge"), "Whenever this creature and at least one other creature attacks, draw a card.", false)]
#[case::one_other_plural(&battalion(ATTACKS, 1, "Ge"), "Whenever this creature and at least one other creature attack, draw a card.", true)]
#[case::three_for_two(&battalion(ATTACKS, 3, "Ge"), "Whenever this creature and at least two other creatures attack, draw a card.", false)]
#[case::exactly(&battalion(ATTACKS, 2, "Eq"), "Whenever this creature and at least two other creatures attack, draw a card.", false)]
#[case::another_attacker(&battalion(r#"{"Attacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#, 2, "Ge"), "Whenever this creature and at least two other creatures attack, draw a card.", false)]
#[case::upkeep_of_enchanted(
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":{"ControllerOf":"Attached"}}},"effects":[{"DealDamage":{"to":"Triggering","amount":1}}]}}"#,
    "At the beginning of the upkeep of enchanted creature's controller, this creature deals 1 damage to that player.",
    true
)]
#[case::upkeep_of_yours(
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"DealDamage":{"to":"Triggering","amount":1}}]}}"#,
    "At the beginning of the upkeep of enchanted creature's controller, this creature deals 1 damage to that player.",
    false
)]
#[case::end_step_of_enchanted(
    r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":{"ControllerOf":"Attached"}}},"effects":[{"DealDamage":{"to":"Triggering","amount":1}}]}}"#,
    "At the beginning of the upkeep of enchanted creature's controller, this creature deals 1 damage to that player.",
    false
)]
#[case::attacks_alone(
    r#"{"Triggered":{"when":{"AttacksAlone":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"effects":[{"Continuous":{"affected":"Triggering","mods":[{"AddKeyword":"Lifelink"}],"duration":"UntilEndOfTurn"}}]}}"#,
    "Whenever a creature you control attacks alone, it gains lifelink until end of turn.",
    true
)]
#[case::attacks_not_alone(
    r#"{"Triggered":{"when":{"Attacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"effects":[{"Continuous":{"affected":"Triggering","mods":[{"AddKeyword":"Lifelink"}],"duration":"UntilEndOfTurn"}}]}}"#,
    "Whenever a creature you control attacks alone, it gains lifelink until end of turn.",
    false
)]
#[case::alone_any_controller(
    r#"{"Triggered":{"when":{"AttacksAlone":{"what":[{"Type":"Creature"}]}},"effects":[{"Continuous":{"affected":"Triggering","mods":[{"AddKeyword":"Lifelink"}],"duration":"UntilEndOfTurn"}}]}}"#,
    "Whenever a creature you control attacks alone, it gains lifelink until end of turn.",
    false
)]
fn wf24_trigger_heads(#[case] json: &str, #[case] clause: &str, #[case] same: bool) {
    assert_eq!(reads(json, clause), same, "{clause}");
}
