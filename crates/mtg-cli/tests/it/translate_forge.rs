//! Goal: `translate-forge` over a folder of invented scripts writes a proposal in the format a model answers with for each
//! card it can translate, marks it as from Forge beside the other meta of the card, names what it left out in a ledger, and
//! selects cards by name.

use std::fs;
use std::path::Path;

use clap::Parser;
use mtg_cli::args::{Cli, Command};
use mtg_cli::cache::Cache;
use mtg_cli::translate::translate_forge;
use serde_json::{Value, json};

const BOLT: &str = "Name:Quartz Spark\nManaCost:R\nTypes:Instant\nA:SP$ DealDamage | ValidTgts$ Any | NumDmg$ 3 | SpellDescription$ Quartz Spark deals 3 damage to any target.\nOracle:Quartz Spark deals 3 damage to any target.\n";
const SEER: &str = "Name:Willow Seer\nManaCost:1 U\nTypes:Creature Human Wizard\nPT:1/2\nK:Flying\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigDraw | TriggerDescription$ When Willow Seer enters, draw a card.\nSVar:TrigDraw:DB$ Draw | NumCards$ 1\nA:AB$ Dig | Cost$ T | DigNum$ 3\nOracle:Flying\\nWhen Willow Seer enters, draw a card.\n";
const VANILLA: &str = "Name:Plain Ox\nManaCost:2\nTypes:Creature Ox\nPT:2/2\nOracle:\n";
const TWO_FACED: &str = "Name:Ember Pup\nManaCost:R\nTypes:Creature Dog\nPT:1/1\nK:Haste\nALTERNATE\nName:Ember Hound\nManaCost:no cost\nTypes:Creature Dog\nPT:3/3\nK:Trample\nOracle:Trample\n";

fn write_script(root: &Path, name: &str, text: &str) {
    let dir = root.join("forge-gui/res/cardsfolder").join(&name[..1]);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(format!("{name}.txt")), text).unwrap();
}

fn checkout(root: &Path) {
    write_script(root, "quartz_spark", BOLT);
    write_script(root, "willow_seer", SEER);
    write_script(root, "plain_ox", VANILLA);
    write_script(root, "ember_pup", TWO_FACED);
}

async fn run(root: &Path, out: &Path, extra: &[&str]) -> String {
    let mut words = vec![
        "mtg-cli",
        "translate-forge",
        root.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ];
    words.extend_from_slice(extra);
    let Command::TranslateForge(args) = Cli::try_parse_from(words).unwrap().command else {
        panic!("expected translate-forge");
    };
    let cache = Cache::new(root.join("cache"));
    translate_forge(&cache, None, &args).await.unwrap()
}

fn json_of(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[tokio::test]
async fn proposals_are_written_for_the_cards_with_something_translated() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    checkout(dir.path());
    let summary = run(dir.path(), &out, &[]).await;
    assert!(
        summary.starts_with("translate-forge: 4 scripts read: 2 translated whole, 1 partly, 0 skipped, 1 with no rule line"),
        "{summary}"
    );
    assert_eq!(
        json_of(&out.join("quartz_spark.json")),
        json!({"faces": [{"abilities": [{"Spell": {
            "targets": [{"what": "AnyTarget"}],
            "effects": [{"DealDamage": {"to": {"Target": 0}, "amount": 3}}]}}]}]})
    );
    // The line without a mapper is left out, the others are kept.
    assert_eq!(
        json_of(&out.join("willow_seer.json")),
        json!({"faces": [{"abilities": [
            {"Keyword": "Flying"},
            {"Triggered": {"when": {"ZoneChange": {"what": ["This"], "to": "Battlefield"}}, "effects": [{"Draw": {}}]}}]}]})
    );
    // A card of two faces is named as Scryfall names it and has the abilities of each face.
    assert_eq!(
        json_of(&out.join("ember_pup_ember_hound.json")),
        json!({"faces": [
            {"abilities": [{"Keyword": "Haste"}]},
            {"abilities": [{"Keyword": "Trample"}]}]})
    );
    assert!(!out.join("plain_ox.json").exists());
}

#[tokio::test]
async fn each_proposal_is_marked_as_from_forge_and_other_meta_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    checkout(dir.path());
    fs::create_dir_all(out.join("meta")).unwrap();
    fs::write(
        out.join("meta/quartz_spark.json"),
        r#"{"solved_examples": 5}"#,
    )
    .unwrap();
    run(dir.path(), &out, &[]).await;
    assert_eq!(
        json_of(&out.join("meta/quartz_spark.json")),
        json!({"solved_examples": 5, "source": "forge"})
    );
    assert_eq!(
        json_of(&out.join("meta/willow_seer.json")),
        json!({"source": "forge"})
    );
}

#[tokio::test]
async fn the_ledger_names_what_was_left_out_and_the_cards_it_holds_back() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    checkout(dir.path());
    run(dir.path(), &out, &[]).await;
    let ledger = fs::read_to_string(out.join("translate-gaps.tsv")).unwrap();
    let mut lines = ledger.lines();
    assert_eq!(
        lines.next(),
        Some("kind\treason\tblocked\tfreed\tweight\texamples")
    );
    assert_eq!(
        lines.next(),
        Some("unmapped\teffect:Dig\t1\t1\t0\tWillow Seer")
    );
    let index = fs::read_to_string(out.join("translate-index.tsv")).unwrap();
    assert!(index.contains("willow_seer\tWillow Seer\tpartial\t"), "{index}");
    assert!(index.contains("quartz_spark\tQuartz Spark\twhole\t"), "{index}");
}

#[tokio::test]
async fn cards_are_selected_by_name_or_by_the_name_of_a_face() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    checkout(dir.path());
    run(dir.path(), &out, &["--cards", "quartz spark", "Ember Hound"]).await;
    assert!(out.join("quartz_spark.json").exists());
    assert!(out.join("ember_pup_ember_hound.json").exists());
    assert!(!out.join("willow_seer.json").exists());
}

#[tokio::test]
async fn a_folder_with_no_scripts_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let Command::TranslateForge(args) = Cli::try_parse_from([
        "mtg-cli",
        "translate-forge",
        dir.path().to_str().unwrap(),
    ])
    .unwrap()
    .command
    else {
        panic!("expected translate-forge");
    };
    let cache = Cache::new(dir.path().join("cache"));
    let error = translate_forge(&cache, None, &args).await.unwrap_err();
    assert!(error.to_string().contains("no Forge card scripts"), "{error}");
}
