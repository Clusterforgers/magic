//! Goal: a line of a script splits into its keys and values, a script names its verbs by family with the keys of each line, a part named by another line is read once through the reference whatever the nesting, and a checkout is read from either of its two roots.
use std::fs;

use rstest::rstest;

use super::*;

fn pairs(params: &Params) -> Vec<(&str, &str)> {
    params.iter().collect()
}

#[rstest]
#[case::the_usual_shape("AB$ Zap | Cost$ T | Amount$ 3", &[("AB", "Zap"), ("Cost", "T"), ("Amount", "3")])]
#[case::pipes_without_spaces("AB$ Zap|Cost$ T", &[("AB", "Zap"), ("Cost", "T")])]
#[case::a_key_with_no_value("DB$ Zap | Optional$", &[("DB", "Zap"), ("Optional", "")])]
#[case::a_part_with_no_dollar("DB$ Zap | Flag", &[("DB", "Zap"), ("Flag", "")])]
#[case::a_dollar_inside_a_value("DB$ Zap | Note$ costs $5", &[("DB", "Zap"), ("Note", "costs $5")])]
#[case::empty_parts_are_dropped("DB$ Zap | | ", &[("DB", "Zap")])]
#[case::nothing("", &[])]
fn a_line_splits_into_its_keys_and_values(#[case] body: &str, #[case] expected: &[(&str, &str)]) {
    assert_eq!(pairs(&Params::parse(body)), expected);
}

#[test]
fn a_key_is_found_by_its_first_pair() {
    let params = Params::parse("DB$ Zap | Amount$ 3 | Amount$ 4");
    assert_eq!(params.get("Amount"), Some("3"));
    assert!(params.has("DB") && !params.has("Cost"));
}

#[rstest]
#[case::text_and_hints("DB$ Zap | SpellDescription$ x | AILogic$ Y | TgtPrompt$ z | StackDescription$ w", &[])]
#[case::references_and_targeting("DB$ Zap | SubAbility$ Next | ValidTgts$ Any | Defined$ You | Cost$ T", &[])]
#[case::memory("DB$ Zap | RememberChanged$ True | ForgetOtherRemembered$ True | ClearRemembered$ True", &[])]
#[case::variations("DB$ Zap | Optional$ True | NonLegendary$ True | Mode$ Hand", &["Optional", "NonLegendary", "Mode"])]
fn only_keys_that_vary_the_verb_are_meaningful(#[case] body: &str, #[case] expected: &[&str]) {
    assert_eq!(Params::parse(body).meaningful_keys(), expected);
}

const TWO_FACES: &str = "\
Name:Quill Sprite
ManaCost:1 U
Types:Creature Faerie
K:Flying
K:Cycling:2
A:AB$ Zap | Cost$ T Sac<1/Creature.Other/another creature> | ValidTgts$ Any | Amount$ 2 | SubAbility$ DBMore | SpellDescription$ Ignored text.
SVar:DBMore:DB$ Mend | Defined$ You | Amount$ X
SVar:X:Count$Valid Creature.YouCtrl
SVar:AIPreference:SacCost$Creature
Oracle:Whatever.
ALTERNATE
Name:Ink Well
ManaCost:no cost
Types:Land
S:Mode$ Glow | Affected$ Creature.YouCtrl | AddPower$ 1
";

#[test]
fn a_script_has_one_face_for_each_alternate() {
    let script = Script::parse(TWO_FACES);
    assert_eq!(
        script.names().collect::<Vec<_>>(),
        ["Quill Sprite", "Ink Well"]
    );
    assert_eq!(script.faces[0].keywords, ["Flying", "Cycling:2"]);
    assert_eq!(script.faces[1].statics.len(), 1);
    assert_eq!(script.faces[0].svars.len(), 3);
}

fn summary(items: &[Item]) -> Vec<(Kind, &str)> {
    items
        .iter()
        .map(|item| (item.kind, item.name.as_str()))
        .collect()
}

#[test]
fn a_script_names_its_verbs_by_family() {
    let items = Script::parse(TWO_FACES).items();
    assert_eq!(
        summary(&items),
        [
            (Kind::Effect, "Zap"),
            (Kind::Cost, "T"),
            (Kind::Cost, "Sac"),
            (Kind::Effect, "Mend"),
            (Kind::Keyword, "Flying"),
            (Kind::Keyword, "Cycling"),
            (Kind::Static, "Glow"),
        ]
    );
    let zap = &items[0];
    assert_eq!(zap.params.get("Amount"), Some("2"));
    assert_eq!(zap.params.meaningful_keys(), ["Amount"]);
}

fn face(lines: &str) -> Face {
    Script::parse(&format!("Name:Test Card\n{lines}"))
        .faces
        .remove(0)
}

#[test]
fn a_trigger_reaches_its_effect_through_a_name_and_the_chain_through_sub_abilities() {
    let items = face(
        "T:Mode$ Bell | ValidCard$ Card.Self | Execute$ TrigOne | TriggerDescription$ Ring.\n\
         SVar:TrigOne:DB$ Alpha | SubAbility$ DBTwo\n\
         SVar:DBTwo:DB$ Beta | SubAbility$ DBThree | Optional$ True\n\
         SVar:DBThree:DB$ Gamma",
    )
    .items();
    assert_eq!(
        summary(&items),
        [
            (Kind::Trigger, "Bell"),
            (Kind::Effect, "Alpha"),
            (Kind::Effect, "Beta"),
            (Kind::Effect, "Gamma"),
        ]
    );
}

#[test]
fn a_name_in_a_list_or_behind_a_sign_is_followed() {
    let items = face(
        "A:SP$ Menu | Choices$ DBOne,DBTwo | Amount$ -Z\n\
         SVar:DBOne:DB$ One\n\
         SVar:DBTwo:DB$ Two\n\
         SVar:Z:Count$Valid Card",
    )
    .items();
    assert_eq!(
        summary(&items),
        [
            (Kind::Effect, "Menu"),
            (Kind::Effect, "One"),
            (Kind::Effect, "Two")
        ]
    );
}

#[test]
fn a_reference_cycle_ends_and_a_shared_part_counts_once() {
    let items = face(
        "A:SP$ Loop | SubAbility$ DBBack | Execute$ DBShared\n\
         A:SP$ Other | SubAbility$ DBShared\n\
         SVar:DBBack:DB$ Back | SubAbility$ DBBack\n\
         SVar:DBShared:DB$ Shared",
    )
    .items();
    assert_eq!(
        summary(&items),
        [
            (Kind::Effect, "Loop"),
            (Kind::Effect, "Back"),
            (Kind::Effect, "Shared"),
            (Kind::Effect, "Other"),
        ]
    );
}

#[test]
fn a_part_that_no_line_names_is_read_and_a_hint_or_an_amount_is_not() {
    let items = face(
        "A:SP$ Main\n\
         SVar:Orphan:DB$ Loose | Amount$ 1\n\
         SVar:HasAttackEffect:TRUE\n\
         SVar:NeedsToPlayVar:Y LE2\n\
         SVar:Y:Count$Valid Card",
    )
    .items();
    assert_eq!(
        summary(&items),
        [(Kind::Effect, "Main"), (Kind::Effect, "Loose")]
    );
}

#[rstest]
#[case::a_trigger_by_its_key(
    "A:SP$ Give | Triggers$ Hook\nSVar:Hook:Mode$ Ping | Execute$ DBGo\nSVar:DBGo:DB$ Go",
    Kind::Trigger
)]
#[case::a_static_by_its_key(
    "A:SP$ Give | StaticAbilities$ Stat\nSVar:Stat:Mode$ Ping | Affected$ Card",
    Kind::Static
)]
#[case::a_replacement_by_its_shape(
    "A:SP$ Give | ReplacementEffects$ Rep\nSVar:Rep:Event$ Ping | ValidCard$ Card",
    Kind::Replacement
)]
#[case::a_trigger_by_its_execute_when_no_key_says(
    "A:SP$ Give | Gives$ Hook\nSVar:Hook:Mode$ Ping | Execute$ DBGo\nSVar:DBGo:DB$ Go",
    Kind::Trigger
)]
#[case::a_static_when_nothing_says(
    "S:Mode$ Other\nA:SP$ Give | Gives$ Hook\nSVar:Hook:Mode$ Ping | Affected$ Card",
    Kind::Static
)]
fn a_part_with_a_mode_is_a_trigger_or_a_static_ability_by_how_it_is_named(
    #[case] lines: &str,
    #[case] expected: Kind,
) {
    let items = face(lines).items();
    let ping = items.iter().find(|item| item.name == "Ping");
    assert_eq!(ping.map(|item| item.kind), Some(expected));
}

#[rstest]
#[case::tap_and_mana("T 2 G", &["T", "Mana", "Mana"])]
#[case::mixed_symbols_and_brackets("X R/G Sac<1/Creature.Other/another creature> PayLife<2>", &["Mana", "Mana", "Sac", "PayLife"])]
#[case::the_source("SacMe Q", &["SacMe", "Q"])]
#[case::the_source_by_name("Sac<1/CARDNAME> Discard<1/CARDNAME> Sac<1/NICKNAME> Exile<1/Card.Self>", &["SacMe", "DiscardMe", "SacMe", "ExileMe"])]
#[case::snow_and_colorless("S C 0", &["Mana", "Mana", "Mana"])]
#[case::a_counter_cost("SubCounter<2/P1P1> tapXType<1/Creature.Elf/untapped Elf>", &["SubCounter", "tapXType"])]
#[case::nothing("", &[])]
fn a_cost_splits_outside_its_brackets_into_named_parts(
    #[case] cost: &str,
    #[case] expected: &[&str],
) {
    assert_eq!(cost_parts(cost), expected);
}

#[rstest]
#[case::backup("K:Backup:1:BackupAb\nSVar:BackupAb:DB$ Pump | KW$ Double Strike | SubAbility$ DBMore\nSVar:DBMore:DB$ Draw", &["Backup"])]
#[case::gift("K:Gift\nSVar:GiftAbility:DB$ Token | TokenScript$ x | SubAbility$ DBMore\nSVar:DBMore:DB$ Draw", &["Gift"])]
#[case::a_keyword_with_a_part_of_its_own("K:Fabricate:1:FabAb\nSVar:FabAb:DB$ Pump", &["Fabricate", "Pump"])]
fn the_ability_of_a_keyword_the_dsl_has_whole_gives_no_item(
    #[case] lines: &str,
    #[case] expected: &[&str],
) {
    let names: Vec<_> = face(lines)
        .items()
        .into_iter()
        .map(|item| item.name)
        .collect();
    assert_eq!(names, expected);
}

#[test]
fn the_effect_a_replacement_names_carries_the_event_it_replaces() {
    let lines = "R:Event$ Draw | ReplaceWith$ Twice\nSVar:Twice:DB$ Draw | SubAbility$ DBMore\nSVar:DBMore:DB$ Draw\nA:SP$ Draw";
    let items = face(lines).items();
    let replacing: Vec<_> = items
        .iter()
        .map(|item| item.params.get("Replacing"))
        .collect();
    // The ability comes first, then the replacement, the part it names and the part that one names.
    assert_eq!(replacing, [None, None, Some("Draw"), None]);
}

#[rstest]
#[case("Lim-Dûl's Vault", "lim-dul's vault")]
#[case("Æther Spike", "aether spike")]
#[case(
    "Kongming, \u{201c}Sleeping Dragon\u{201d}",
    "kongming, \"sleeping dragon\""
)]
#[case("Jötun Grunt", "jotun grunt")]
#[case("Mark of the Oni", "mark of the oni")]
fn names_are_folded_for_matching(#[case] name: &str, #[case] folded: &str) {
    assert_eq!(fold(name), folded);
}

#[test]
fn a_checkout_is_read_from_its_repository_root_or_from_the_card_folder() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("forge-gui/res/cardsfolder");
    fs::create_dir_all(folder.join("q")).unwrap();
    fs::create_dir_all(folder.join("rebalanced")).unwrap();
    fs::write(folder.join("q/quill_sprite.txt"), TWO_FACES).unwrap();
    fs::write(folder.join("q/notes.md"), "Name:Not A Script").unwrap();
    fs::write(folder.join("rebalanced/a_quill.txt"), "Name:A-Quill Sprite").unwrap();
    fs::write(folder.join("q/empty.txt"), "no name here").unwrap();

    let from_root = read_checkout(root.path()).unwrap();
    let from_folder = read_checkout(&folder).unwrap();
    assert_eq!(from_root, from_folder);
    assert_eq!(from_root.len(), 1);
    assert_eq!(from_root[0].faces.len(), 2);
}

#[test]
fn a_directory_without_scripts_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let error = read_checkout(root.path()).unwrap_err().to_string();
    assert!(error.contains("no Forge card scripts"), "{error}");
    assert!(read_checkout(&root.path().join("missing")).is_err());
}
