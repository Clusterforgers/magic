//! Goal: the frame words of a verb are table rows: a target on an "all" verb limits the objects to those the target player
//! controls, `Counter` reads the kinds of stack object and what they target, `Imprint*` keys write the imprinted memory, a
//! forget of moved objects holds for a bound name, and the target slot of `Branch` and `Token` is read before its parts.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn spell(line: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {line}\n")
}

fn effects(script: &str) -> Value {
    let found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    found[0]["Spell"]["effects"].clone()
}

#[rstest]
#[case::destroy_of_a_player(
    "DestroyAll | ValidTgts$ Player | ValidCards$ Creature",
    json!({"Destroy": {"what": {"Each": [{"Type": "Creature"}, {"ControlledBy": {"Target": 0}}]}, }})
)]
#[case::destroy_of_an_opponent(
    "DestroyAll | ValidTgts$ Opponent | ValidCards$ Land | NoRegen$ True",
    json!({"Destroy": {"what": {"Each": [{"Type": "Land"}, {"ControlledBy": {"Target": 0}}]}, "regenerate": false}})
)]
#[case::damage_of_an_opponent(
    "DamageAll | ValidTgts$ Opponent | NumDmg$ 1 | ValidCards$ Creature.nonArtifact",
    json!({"DealDamage": {"to": {"Each": [{"Type": "Creature"}, {"Not": {"Type": "Artifact"}}, {"ControlledBy": {"Target": 0}}]}, "amount": 1}})
)]
fn an_all_verb_with_a_player_target_acts_on_what_the_player_controls(
    #[case] line: &str,
    #[case] node: Value,
) {
    assert_eq!(effects(&spell(line)), json!([node]));
}

#[rstest]
#[case::object_target_not_named(
    "DestroyAll | ValidTgts$ Creature | ValidCards$ Aura",
    "key:DestroyAll.ValidTgts-unread"
)]
#[case::damage_to_players_too(
    "DamageAll | ValidTgts$ Opponent | ValidCards$ Creature | ValidPlayers$ Player",
    "key:DamageAll.ValidTgts-with-ValidPlayers"
)]
#[case::bounds(
    "DestroyAll | ValidTgts$ Player | TargetMax$ 2 | ValidCards$ Creature",
    "key:DestroyAll.TargetMax"
)]
fn an_all_verb_with_a_target_it_cannot_place_is_a_gap(#[case] line: &str, #[case] gap: &str) {
    assert!(
        gaps(&spell(line)).contains(&gap.to_owned()),
        "{:?}",
        gaps(&spell(line))
    );
}

#[test]
fn an_all_verb_that_names_its_object_target_reads_it() {
    let script = spell("DestroyAll | ValidCards$ Card.AttachedTo Targeted | ValidTgts$ Creature");
    assert_eq!(
        effects(&script)[0]["Destroy"]["what"]["Each"],
        json!([{"AttachedTo": {"Target": 0}}])
    );
}

#[rstest]
#[case::spell("Spell", "Card", json!(["Spell"]))]
#[case::activated("Activated", "Card", json!([{"Ability": "Activated"}]))]
#[case::both(
    "Activated,Triggered",
    "Card,Emblem",
    json!([{"AnyOf": [{"Ability": "Activated"}, {"Ability": "Triggered"}]}])
)]
#[case::mixed(
    "Spell,Activated,Triggered",
    "Card,Emblem",
    json!([{"AnyOf": ["Spell", {"Ability": "Activated"}, {"Ability": "Triggered"}]}])
)]
fn a_counter_reads_the_kinds_of_stack_object(
    #[case] kinds: &str,
    #[case] valid: &str,
    #[case] want: Value,
) {
    let script = spell(&format!(
        "Counter | TargetType$ {kinds} | ValidTgts$ {valid}"
    ));
    let effects = effects(&script);
    assert_eq!(effects[0], json!({"Counter": {"what": {"Target": 0}}}));
    let targets = abilities(&script)[0]["Spell"]["targets"].clone();
    assert_eq!(targets[0]["what"]["Object"], want);
}

#[rstest]
#[case::you("You", json!({"TargetsPlayer": "You"}))]
#[case::your_permanent("Permanent.YouCtrl+inRealZoneBattlefield", json!({"Targeting": [{"ControlledBy": "You"}]}))]
#[case::either(
    "You,Creature.inRealZoneBattlefield",
    json!({"AnyOf": [{"TargetsPlayer": "You"}, {"Targeting": [{"Type": "Creature"}]}]})
)]
fn a_counter_can_ask_what_the_spell_targets(#[case] words: &str, #[case] pred: Value) {
    let script = spell(&format!(
        "Counter | TargetType$ Spell | ValidTgts$ Card | TargetValidTargeting$ {words}"
    ));
    let targets = abilities(&script)[0]["Spell"]["targets"].clone();
    assert_eq!(targets[0]["what"]["Object"][1], pred);
}

#[rstest]
#[case::spell_with_abilities_and_control("Spell,Activated.YouCtrl", "Card")]
#[case::unknown_qualifier("Spell.nonCreature", "Card")]
#[case::ability_of_a_type("Activated", "Creature")]
fn a_counter_kind_it_cannot_state_stays_a_gap(#[case] kinds: &str, #[case] valid: &str) {
    let script = spell(&format!(
        "Counter | TargetType$ {kinds} | ValidTgts$ {valid}"
    ));
    assert!(
        gaps(&script)
            .iter()
            .any(|gap| gap.starts_with("key:Counter.")),
        "{:?}",
        gaps(&script)
    );
}

#[test]
fn imprinting_the_source_binds_the_imprinted_memory() {
    let script = spell(
        "Effect | ImprintCards$ Self | ReplacementEffects$ Prevent | SubAbility$ Hit\nSVar:Prevent:Event$ DamageDone | ValidTarget$ You | ValidSource$ Card.IsImprinted | Prevent$ True | Description$ x\nSVar:Hit:DB$ GainLife | LifeAmount$ 1",
    );
    // The shield reads the imprinted source: the line is no gap about the key.
    assert!(
        !gaps(&script).iter().any(|gap| gap.contains("ImprintCards")),
        "{:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::unknown_word(
    "ImprintCards$ AttachedBy Sacrificed",
    "key:Effect.ImprintCards=AttachedBy Sacrificed"
)]
fn an_imprint_word_it_cannot_read_stays_a_gap(#[case] key: &str, #[case] gap: &str) {
    let script = spell(&format!("Effect | {key}"));
    assert!(
        gaps(&script).contains(&gap.to_owned()),
        "{:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::token(
    "Token | TokenScript$ w_1_1_soldier | ImprintTokens$ True",
    "CreateToken"
)]
fn imprinted_tokens_are_the_objects_the_line_made(#[case] line: &str, #[case] verb: &str) {
    let script = format!(
        "Name:T\nTypes:Sorcery\nA:SP${} | SubAbility$ Pump\nSVar:Pump:DB$ PutCounter | Defined$ Imprinted | CounterType$ P1P1 | CounterNum$ 1\n",
        format_args!(" {line}")
    );
    let effects = effects(&script);
    assert_eq!(
        effects[0][verb]["as"],
        effects[1]["PutCounters"]["on"]["Bound"]
    );
}

#[test]
fn a_copy_that_is_remembered_is_bound_for_the_next_step() {
    let script = spell(
        "CopyPermanent | ValidTgts$ Creature | RememberTokens$ True | SubAbility$ Up\nSVar:Up:DB$ PutCounter | Defined$ Remembered | CounterType$ P1P1 | CounterNum$ 1",
    );
    let effects = effects(&script);
    assert_eq!(
        effects[0]["CreateToken"]["as"],
        effects[1]["PutCounters"]["on"]["Bound"]
    );
}

#[rstest]
#[case::remembered_cards("RememberCards$ True")]
fn counters_put_on_a_target_remember_it(#[case] key: &str) {
    let script = spell(&format!(
        "PutCounter | ValidTgts$ Creature | CounterType$ P1P1 | CounterNum$ 1 | {key} | SubAbility$ Up\nSVar:Up:DB$ Pump | Defined$ Remembered | NumAtt$ +1"
    ));
    assert_eq!(gaps(&script), Vec::<String>::new());
}

#[test]
fn a_bound_name_drops_what_moved_without_a_step() {
    let script = "Name:T\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Battlefield | Destination$ Exile | ValidTgts$ Creature | RememberChanged$ True | SubAbility$ Back\nSVar:Back:DB$ ChangeZone | Defined$ Remembered | Origin$ Exile | Destination$ Battlefield | ForgetChanged$ True | SubAbility$ Rest\nSVar:Rest:DB$ ChangeZone | Defined$ Remembered | Origin$ Exile | Destination$ Hand\n";
    assert_eq!(gaps(script), Vec::<String>::new());
}

#[rstest]
#[case::up_to_one("TargetMin$ 0 | TargetMax$ 1")]
#[case::exactly_one("")]
fn a_role_token_goes_to_at_most_one_target(#[case] bounds: &str) {
    let script = spell(&format!(
        "Token | TokenScript$ w_1_1_soldier | AttachedTo$ ThisTargetedCard | ValidTgts$ Creature.YouCtrl | {bounds}"
    ));
    let found = effects(&script);
    assert_eq!(found[1]["Attach"]["to"], json!({"Target": 0}));
}

#[test]
fn a_token_for_several_targets_stays_a_gap() {
    let script = spell(
        "Token | TokenScript$ w_1_1_soldier | AttachedTo$ Targeted | ValidTgts$ Creature | TargetMax$ 2",
    );
    assert!(gaps(&script).contains(&"key:Token.TargetMax".to_owned()));
}

#[test]
fn a_branch_with_a_target_declares_it_before_its_parts() {
    let script = spell(
        "Branch | ValidTgts$ Creature | BranchConditionSVar$ X | TrueSubAbility$ Big | FalseSubAbility$ Small\nSVar:X:Count$Valid Card.YouCtrl\nSVar:Big:DB$ DealDamage | Defined$ Targeted | NumDmg$ 3\nSVar:Small:DB$ DealDamage | Defined$ Targeted | NumDmg$ 1",
    );
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let effects = &found[0]["Spell"]["effects"];
    assert_eq!(
        effects[0]["If"]["then"][0]["DealDamage"]["to"],
        json!({"Target": 0})
    );
    assert_eq!(
        effects[0]["If"]["otherwise"][0]["DealDamage"]["to"],
        json!({"Target": 0})
    );
}

#[test]
fn tap_all_of_a_target_player_taps_what_the_player_controls() {
    let script = spell("TapAll | ValidTgts$ Player | ValidCards$ Creature");
    assert_eq!(
        effects(&script)[0]["Tap"]["what"]["Each"],
        json!([{"Type": "Creature"}, {"ControlledBy": {"Target": 0}}])
    );
}
