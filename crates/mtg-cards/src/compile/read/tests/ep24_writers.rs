//! Goal: the writers of ep24 write the card's words and nothing near them: a change of targets ("Change a target of target spell or ability to ~.", "you may change the target of target instant or sorcery spell with a single target to ~", "Change the target of target spell or ability with a single target.", "You may choose new targets for target spell or ability."); another object to change to, another kind of spell or a missing "single target" do not read back.
use super::*;

const SPELL_OR_ABILITY: &str =
    r#"{"what":{"Object":[{"AnyOf":["Spell",{"Ability":"Activated"},{"Ability":"Triggered"}]}]}}"#;
const SINGLE_SPELL_OR_ABILITY: &str = r#"{"what":{"Object":[{"AnyOf":["Spell",{"Ability":"Activated"},{"Ability":"Triggered"}]},"SingleTarget"]}}"#;
const SINGLE_INSTANT_OR_SORCERY: &str = r#"{"what":{"Object":["Spell",{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},"SingleTarget"]}}"#;
const INSTANT_OR_SORCERY: &str =
    r#"{"what":{"Object":["Spell",{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]}}"#;
const CREATURE_SPELL: &str = r#"{"what":{"Object":["Spell",{"Type":"Creature"}]}}"#;

fn change(to: &str, onto: &str) -> String {
    format!(r#"{{"ChangeTargets":{{"what":{{"Target":0}},"to":"{to}"{onto}}}}}"#)
}

fn activated(target: &str, effect: &str) -> Ability {
    ability(&format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{U/P}}"}}],"targets":[{target}],"effects":[{effect}]}}}}"#
    ))
}

fn enters(target: &str, effect: &str) -> Ability {
    ability(&format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"to":"Battlefield","what":["This"]}}}},"targets":[{target}],"effects":[{{"May":{{"effects":[{effect}]}}}}]}}}}"#
    ))
}

fn spell(target: &str, effect: &str) -> Ability {
    ability(&format!(
        r#"{{"Spell":{{"targets":[{target}],"effects":[{effect}]}}}}"#
    ))
}

const THIS: &str = r#","onto":"This""#;
const SKITE: &str = "{U/P}: Change a target of target spell or ability to ~.";
const SPECIMEN: &str = "When ~ enters, you may change the target of target instant or sorcery spell with a single target to ~.";
const MEDDLER: &str = "When ~ enters, you may change a target of target spell or ability to ~.";
const BEND: &str = "Change the target of target spell or ability with a single target.";
const SWAT: &str = "You may choose new targets for target spell or ability.";

#[rstest]
#[case::skite(activated(SPELL_OR_ABILITY, &change("One", THIS)), SKITE, true)]
#[case::skite_to_a_target(activated(SPELL_OR_ABILITY, &change("One", r#","onto":{"Target":1}"#)), SKITE, false)]
#[case::skite_of_a_creature_spell(activated(CREATURE_SPELL, &change("One", THIS)), SKITE, false)]
#[case::meddler(enters(SPELL_OR_ABILITY, &change("One", THIS)), MEDDLER, true)]
#[case::specimen(enters(SINGLE_INSTANT_OR_SORCERY, &change("One", THIS)), SPECIMEN, true)]
#[case::specimen_without_single(enters(INSTANT_OR_SORCERY, &change("One", THIS)), SPECIMEN, false)]
#[case::specimen_choosing(enters(SINGLE_INSTANT_OR_SORCERY, &change("Choose", "")), SPECIMEN, false)]
#[case::bend(spell(SINGLE_SPELL_OR_ABILITY, &change("Another", "")), BEND, true)]
#[case::bend_without_single(spell(SPELL_OR_ABILITY, &change("Another", "")), BEND, false)]
#[case::swat(spell(SPELL_OR_ABILITY, &change("Choose", "")), SWAT, true)]
#[case::swat_changing_one(spell(SPELL_OR_ABILITY, &change("Another", "")), SWAT, false)]
fn ep24_changes_of_targets_write_as_printed(
    #[case] ability: Ability,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability);
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}
