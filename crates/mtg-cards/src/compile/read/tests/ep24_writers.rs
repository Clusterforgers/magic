//! Goal: the writers of ep24 write the card's words and nothing near them: a change of targets ("Change a target of target spell or ability to ~.", "you may change the target of target instant or sorcery spell with a single target to ~", "Change the target of target spell or ability with a single target.", "You may choose new targets for target spell or ability."); another object to change to, another kind of spell or a missing "single target" do not read back. Unspent mana kept or turned into one type ("You don't lose unspent red mana as steps and phases end.", "If you would lose unspent mana, that mana becomes colorless instead."); another type, another player or another group do not read back. Mana spent as though of any color or type, for everything or one kind of spell; the other conversion or kind does not read back. A draw limit of one card for opponents and opponents that can't make you sacrifice; another limit or group does not read back.
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

fn rule(rule: &str) -> Ability {
    ability(&format!(r#"{{"Rule":{{"rule":{rule}}}}}"#))
}

const ELECTRO: &str = "You don't lose unspent red mana as steps and phases end.";
const UPWELLING: &str = "Players don't lose unspent mana as steps and phases end.";
const KRUPHIX: &str = "If you would lose unspent mana, that mana becomes colorless instead.";
const OZAI: &str = "If you would lose unspent mana, that mana becomes red instead.";
const ORRERY: &str = "You may spend mana as though it were mana of any color.";
const OATH: &str =
    "You may spend mana as though it were mana of any color to cast planeswalker spells.";
const VIZIER: &str = "You can spend mana of any type to cast creature spells.";
const NARSET: &str = "Each opponent can't draw more than one card each turn.";
const SIGARDA: &str =
    "Spells and abilities your opponents control can't cause you to sacrifice permanents.";
const KRRIK: &str = "For each {B} in a cost, you may pay 2 life rather than pay that mana.";
const ELIXIR: &str =
    "You may activate abilities of creatures you control as though those creatures had haste.";

#[rstest]
#[case::red_kept(r#"{"KeepUnspentMana":{"who":"You","mana":"Red"}}"#, ELECTRO, true)]
#[case::green_is_not_red(r#"{"KeepUnspentMana":{"who":"You","mana":"Green"}}"#, ELECTRO, false)]
#[case::every_type_is_not_red(r#"{"KeepUnspentMana":{"who":"You"}}"#, ELECTRO, false)]
#[case::opponents_keep(
    r#"{"KeepUnspentMana":{"who":"Opponents","mana":"Red"}}"#,
    ELECTRO,
    false
)]
#[case::players_keep_all(r#"{"KeepUnspentMana":{"who":"EachPlayer"}}"#, UPWELLING, true)]
#[case::you_keep_all(r#"{"KeepUnspentMana":{"who":"You"}}"#, UPWELLING, false)]
#[case::colorless(
    r#"{"UnspentManaBecomes":{"who":"You","into":"Colorless"}}"#,
    KRUPHIX,
    true
)]
#[case::red(r#"{"UnspentManaBecomes":{"who":"You","into":"Red"}}"#, OZAI, true)]
#[case::red_is_not_colorless(
    r#"{"UnspentManaBecomes":{"who":"You","into":"Red"}}"#,
    KRUPHIX,
    false
)]
#[case::an_opponent_s_mana(
    r#"{"UnspentManaBecomes":{"who":"Opponents","into":"Colorless"}}"#,
    KRUPHIX,
    false
)]
#[case::orrery(r#"{"SpendManaAs":{"who":"You","spend_as":"AnyColor"}}"#, ORRERY, true)]
#[case::orrery_of_any_type(r#"{"SpendManaAs":{"who":"You","spend_as":"AnyType"}}"#, ORRERY, false)]
#[case::oath(
    r#"{"SpendManaAs":{"who":"You","spend_as":"AnyColor","spells":[{"Type":"Planeswalker"}]}}"#,
    OATH,
    true
)]
#[case::oath_of_creatures(
    r#"{"SpendManaAs":{"who":"You","spend_as":"AnyColor","spells":[{"Type":"Creature"}]}}"#,
    OATH,
    false
)]
#[case::vizier(
    r#"{"SpendManaAs":{"who":"You","spend_as":"AnyType","spells":[{"Type":"Creature"}]}}"#,
    VIZIER,
    true
)]
#[case::vizier_of_any_color(
    r#"{"SpendManaAs":{"who":"You","spend_as":"AnyColor","spells":[{"Type":"Creature"}]}}"#,
    VIZIER,
    false
)]
#[case::narset(r#"{"DrawLimit":{"who":"Opponents","most":1}}"#, NARSET, true)]
#[case::narset_of_two(r#"{"DrawLimit":{"who":"Opponents","most":2}}"#, NARSET, false)]
#[case::narset_of_everyone(r#"{"DrawLimit":{"who":"EachPlayer","most":1}}"#, NARSET, false)]
#[case::sigarda(r#"{"OpponentsCantMakeSacrifice":"You"}"#, SIGARDA, true)]
#[case::krrik(r#"{"PayLifeForMana":{"who":"You","color":"Black"}}"#, KRRIK, true)]
#[case::elixir(
    r#"{"AbilitiesAsThoughHaste":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#,
    ELIXIR,
    true
)]
#[case::elixir_of_others(
    r#"{"AbilitiesAsThoughHaste":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]}"#,
    ELIXIR,
    false
)]
#[case::elixir_of_artifacts(
    r#"{"AbilitiesAsThoughHaste":[{"Type":"Artifact"},{"ControlledBy":"You"}]}"#,
    ELIXIR,
    false
)]
#[case::krrik_of_green(r#"{"PayLifeForMana":{"who":"You","color":"Green"}}"#, KRRIK, false)]
#[case::sigarda_of_opponents(r#"{"OpponentsCantMakeSacrifice":"Opponents"}"#, SIGARDA, false)]
fn ep24_rules_write_as_printed(#[case] json: &str, #[case] text: &str, #[case] reads: bool) {
    let found = written_variants(&rule(json));
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

fn tap_for(among: &str) -> Ability {
    ability(&format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"AddMana":{{"mana":{{"AnyColorAmong":{among}}}}}}}]}}}}"#
    ))
}

const LEGENDARY: &str = r#"[{"Supertype":"Legendary"},{"ControlledBy":"You"}]"#;
const MINE: &str = r#"[{"ControlledBy":"You"}]"#;
const MY_GRAVEYARD: &str = r#"[{"OwnedBy":"You"},{"In":"Graveyard"}]"#;
const PLAZA: &str = "{T}: Add one mana of any color among legendary permanents you control.";
const CRATER: &str = "{T}: Choose a color of a permanent you control. Add one mana of that color.";
const GRAFSTONE: &str =
    "{T}: Choose a color of a card in your graveyard. Add one mana of that color.";
const KATILDA: &str = "{T}: Add one mana of any of this creature's colors.";

#[rstest]
#[case::plaza(LEGENDARY, PLAZA, true)]
#[case::plaza_of_every_permanent(MINE, PLAZA, false)]
#[case::crater(MINE, CRATER, true)]
#[case::crater_of_legends(LEGENDARY, CRATER, false)]
#[case::grafstone(MY_GRAVEYARD, GRAFSTONE, true)]
#[case::grafstone_on_the_battlefield(MINE, GRAFSTONE, false)]
#[case::katilda(r#"["This"]"#, KATILDA, true)]
#[case::katilda_of_mine(MINE, KATILDA, false)]
fn ep24_mana_of_a_color_among_writes_as_printed(
    #[case] among: &str,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&tap_for(among));
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}
