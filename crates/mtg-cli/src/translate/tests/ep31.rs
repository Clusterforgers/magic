//! Goal: the pieces of ep31 translate as the card prints them. "Copy target activated or triggered ability you control" (`CopySpellAbility | TargetType$ Activated.YouCtrl,Triggered.YouCtrl`) is a `CopyAbility` of a target ability, its source judged by `ValidTgts$`; a copy with no `TargetType$` targets a spell; mixed controllers, a spell among the kinds, an emblem source and another controller stay gaps. `CopyPermanent | DefinedName$ Tarmogoyf` is a token copy of the named card with its count, tapped state and controller; a described land token, exceptions and attachments stay gaps. A card exiled from a hand "until ~ leaves the battlefield" lasts `WhileSourceOnBattlefield`. `NoAbilities`, `hasManaAbility` and `hasNonManaActivatedAbility` are `HasAbility` predicates.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn resonator(target_type: &str, valid: &str) -> String {
    format!(
        "Name:T\nTypes:Artifact\nA:AB$ CopySpellAbility | Cost$ 2 T | TargetType$ {target_type} | ValidTgts$ {valid} | MayChooseTarget$ True | SpellDescription$ Copy target ability.\n"
    )
}

fn copied(selector: Value) -> Value {
    json!({"Activated": {
        "cost": [{"Mana": "{2}"}, "Tap"],
        "targets": [{"what": {"Object": selector}}],
        "effects": [{"CopyAbility": {"what": {"Target": 0}, "new_targets": true}}]}})
}

#[rstest]
#[case::triggered("Triggered.YouCtrl", "Card,Emblem", json!([{"Ability": "Triggered"}, {"ControlledBy": "You"}]))]
#[case::both(
    "Activated.YouCtrl,Triggered.YouCtrl",
    "Card,Emblem",
    json!([{"AnyOf": [{"Ability": "Activated"}, {"Ability": "Triggered"}]}, {"ControlledBy": "You"}])
)]
#[case::any_controller("Activated", "Card", json!([{"Ability": "Activated"}]))]
#[case::an_artifact_source(
    "Activated.YouCtrl,Triggered.YouCtrl",
    "Artifact",
    json!([{"AnyOf": [{"Ability": "Activated"}, {"Ability": "Triggered"}]}, {"ControlledBy": "You"}, {"Type": "Artifact"}])
)]
fn ep31_copy_target_ability(
    #[case] target_type: &str,
    #[case] valid: &str,
    #[case] selector: Value,
) {
    let script = resonator(target_type, valid);
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(abilities(&script), vec![copied(selector)]);
}

#[rstest]
#[case::mixed_controllers(resonator("Activated.YouCtrl,Triggered", "Card"))]
#[case::a_spell_among_the_kinds(resonator("Activated,Triggered,Instant,Sorcery", "Card,Emblem"))]
#[case::an_emblem_source(resonator("Activated.YouCtrl", "Card.Colorless,Emblem"))]
#[case::another_controller(resonator("Activated.YouCtrl", "Card").replace("MayChooseTarget", "Controller$ Opponent | MayChooseTarget"))]
fn ep31_other_ability_copies_stay_gaps(#[case] script: String) {
    assert!(!gaps(&script).is_empty(), "{script}");
}

#[test]
fn ep31_a_copy_without_a_kind_targets_a_spell() {
    let script = "Name:T\nTypes:Creature Human Wizard\nPT:1/1\nA:AB$ CopySpellAbility | Cost$ U U T | ValidTgts$ Instant,Sorcery | MayChooseTarget$ True | SpellDescription$ Copy target instant or sorcery spell.\n";
    assert_eq!(gaps(script), Vec::<String>::new());
    let ability = &abilities(script)[0]["Activated"];
    assert_eq!(ability["targets"][0]["what"]["Object"][0], json!("Spell"));
    assert_eq!(
        ability["effects"][0]["CopySpell"]["new_targets"],
        json!(true)
    );
}

fn nest(keys: &str, oracle: &str) -> String {
    format!(
        "Name:T\nTypes:Artifact\nA:AB$ CopyPermanent | Cost$ 1 G T | DefinedName$ Tarmogoyf{keys} | SpellDescription$ {oracle}\nOracle:{{1}}{{G}}, {{T}}: {oracle}\n"
    )
}

#[rstest]
#[case::one("", "Create a Tarmogoyf token.", json!({"token": {"Card": "Tarmogoyf"}}))]
#[case::two(" | NumCopies$ 2", "Create two Tarmogoyf tokens.", json!({"token": {"Card": "Tarmogoyf"}, "count": 2}))]
#[case::tapped(" | TokenTapped$ True", "Create a tapped Tarmogoyf token.", json!({"token": {"Card": "Tarmogoyf"}, "tapped": true}))]
fn ep31_a_token_copy_of_a_named_card(
    #[case] keys: &str,
    #[case] oracle: &str,
    #[case] create: Value,
) {
    let script = nest(keys, oracle);
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(
        abilities(&script)[0]["Activated"]["effects"],
        json!([{ "CreateToken": create }])
    );
}

#[rstest]
#[case::a_described_land_token(nest("", "Create a Tarmogoyf land token."))]
#[case::an_exception(nest(" | SetPower$ 1", "Create a Tarmogoyf token."))]
#[case::attached(nest(" | AttachedTo$ Self", "Create a Tarmogoyf token."))]
fn ep31_other_named_tokens_stay_gaps(#[case] script: String) {
    assert!(!gaps(&script).is_empty(), "{script}");
}

#[rstest]
#[case::from_a_hand("Hand", true)]
#[case::from_a_library("Library", false)]
fn ep31_exiled_from_a_hand_until_the_source_leaves(#[case] origin: &str, #[case] reads: bool) {
    let script = format!(
        "Name:T\nTypes:Creature Insect\nPT:1/1\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigExile | TriggerDescription$ When CARDNAME enters, exile a card.\nSVar:TrigExile:DB$ ChangeZone | Origin$ {origin} | Destination$ Exile | ValidTgts$ Opponent | DefinedPlayer$ Targeted | Chooser$ You | ChangeType$ Card.nonLand | ChangeNum$ 1 | IsCurse$ True | Duration$ UntilHostLeavesPlay\n"
    );
    let found = abilities(&script);
    let lasts = found.first().and_then(|ability| {
        ability["Triggered"]["effects"]
            .as_array()?
            .iter()
            .find_map(|effect| effect.pointer("/Exile/duration").cloned())
    });
    assert_eq!(
        lasts == Some(json!("WhileSourceOnBattlefield")),
        reads,
        "{found:?} {:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::no_abilities("Creature.NoAbilities", json!({"Not": {"HasAbility": "Any"}}))]
#[case::a_mana_ability("Creature.hasManaAbility", json!({"HasAbility": "Mana"}))]
#[case::activated("Creature.hasNonManaActivatedAbility", json!({"HasAbility": "NonManaActivated"}))]
fn ep31_abilities_an_object_has(#[case] affected: &str, #[case] pred: Value) {
    let script = format!(
        "Name:T\nTypes:Enchantment\nS:Mode$ Continuous | Affected$ {affected} | AddPower$ 2 | AddToughness$ 2 | Description$ Creatures get +2/+2.\n"
    );
    assert_eq!(gaps(&script), Vec::<String>::new());
    let found = abilities(&script);
    assert_eq!(
        found[0]["Static"]["affected"]["Each"],
        json!([{"Type": "Creature"}, pred]),
        "{found:?}"
    );
}
