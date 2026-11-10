//! Goal: a card with no line proven but every gap on a script line of its own is written as a partial translation (an
//! empty proposal, the gapped lines in its ledger) that line fill can complete; a card with a gap of the whole card stays
//! skipped and writes nothing.

use std::fs;
use std::path::Path;

use clap::Parser;
use mtg_cli::args::{Cli, Command};
use mtg_cli::cache::Cache;
use mtg_cli::propose::lines::pool::Translation;
use mtg_cli::translate::translate_forge;
use rstest::rstest;
use serde_json::{Value, json};

const ONE_LINE: &str = "Name:Dice Monk\nManaCost:1 U\nTypes:Creature Human Monk\nPT:1/2\nA:AB$ RollDice | Cost$ T | Sides$ 6 | SpellDescription$ Roll a six-sided die.\nOracle:{T}: Roll a six-sided die.\n";
const TWO_LINES: &str = "Name:Dice Abbot\nManaCost:2 U\nTypes:Creature Human Monk\nPT:2/2\nA:AB$ RollDice | Cost$ T | Sides$ 6 | SpellDescription$ Roll a six-sided die.\nA:AB$ RollDice | Cost$ 1 T | Sides$ 20 | SpellDescription$ Roll a twenty-sided die.\nOracle:{T}: Roll a six-sided die.\\n{1}, {T}: Roll a twenty-sided die.\n";
const SPREE_UNUSED: &str = "Name:Dice Spree\nManaCost:R\nTypes:Sorcery\nK:Spree\nA:SP$ RollDice | Sides$ 6 | SpellDescription$ Roll a six-sided die.\nOracle:Spree\\nRoll a six-sided die.\n";

fn checkout(root: &Path) {
    for (name, text) in [
        ("dice_monk", ONE_LINE),
        ("dice_abbot", TWO_LINES),
        ("dice_spree", SPREE_UNUSED),
    ] {
        let dir = root.join("forge-gui/res/cardsfolder/d");
        fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        fs::write(dir.join(format!("{name}.txt")), text).unwrap_or_else(|e| panic!("{e}"));
    }
}

async fn run(root: &Path, out: &Path) -> String {
    let words = [
        "mtg-cli",
        "translate-forge",
        root.to_str().unwrap_or_default(),
        "--out",
        out.to_str().unwrap_or_default(),
    ];
    let Command::TranslateForge(args) = Cli::try_parse_from(words)
        .unwrap_or_else(|e| panic!("{e}"))
        .command
    else {
        panic!("expected translate-forge");
    };
    translate_forge(&Cache::new(root.join("cache")), None, &args)
        .await
        .unwrap_or_else(|e| panic!("{e}"))
}

fn state_of(out: &Path, slug: &str) -> String {
    let path = out.join("translate-cards.tsv");
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    text.lines()
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .find(|cells| cells[0] == slug)
        .map_or_else(|| panic!("{slug} has no row"), |cells| cells[2].to_owned())
}

#[rstest]
#[case::one_gapped_line("Dice Monk", 1)]
#[case::two_gapped_lines("Dice Abbot", 2)]
#[tokio::test]
async fn a_card_gapped_only_on_its_lines_is_a_partial_line_fill_takes(
    #[case] name: &str,
    #[case] gapped: usize,
) {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    checkout(dir.path());
    run(dir.path(), &out).await;
    let slug = mtg_cards::load::slug(name);
    assert_eq!(state_of(&out, &slug), "partial");
    let translation = Translation::load(&out, name).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(translation.proposal, json!({"faces": [{"abilities": []}]}));
    let gaps = translation.gaps().unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(gaps.len(), gapped);
    assert!(
        gaps.iter()
            .all(|gap| gap.key == "effect:RollDice" && gap.at == 0)
    );
    let meta: Value = serde_json::from_str(
        &fs::read_to_string(out.join("meta").join(format!("{slug}.json"))).unwrap(),
    )
    .unwrap();
    assert_eq!(meta["gaps"], json!(["effect:RollDice"]));
}

#[tokio::test]
async fn a_gap_of_the_whole_card_stays_skipped_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    checkout(dir.path());
    let summary = run(dir.path(), &out).await;
    assert!(
        summary.starts_with(
            "translate-forge: 3 scripts read: 0 translated whole, 2 partly, 1 skipped"
        ),
        "{summary}"
    );
    assert_eq!(state_of(&out, "dice_spree"), "skipped");
    assert!(!out.join("dice_spree.json").exists());
    assert!(!out.join("meta/dice_spree.json").exists());
}
