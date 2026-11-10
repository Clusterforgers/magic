//! Goal: the wf24 writers give the oracle wording of an activation trigger, whose ability is never a mana ability and
//! whose source of an object type is on the battlefield; another player, source, zone or a source anywhere never reads
//! back as the clause.
use super::*;

fn reads(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
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
