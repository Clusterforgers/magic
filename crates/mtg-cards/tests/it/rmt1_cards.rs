//! Goal: every card of the rmt1 pack reads back as its printed text: the checker accepts its rules file clause by clause (a clause left to the table is a `Manual` one of a `Partial` card), so no card plays rules other than the ones it prints.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::BTreeSet;

use mtg_cards::proposal::{ClauseState, Verdict, check_exact};
use mtg_cards::support::Caps;

/// The cards of the pack, as printed.
const PACK: &[&str] = &[
    "Aang, Swift Savior // Aang and La, Ocean's Fury",
    "Abdel Adrian, Gorion's Ward",
    "Adagia, Windswept Bastion",
    "Alliance of Arms",
    "Aminatou, the Fateshifter",
    "Arid Archway",
    "Aurelia's Vindicator",
    "Bridge from Below",
    "Bumi, Unleashed",
    "Burning-Rune Demon",
    "Clandestine Meddler",
    "Collective Voyage",
    "Colossal Grave-Reaver",
    "Diffusion Sliver",
    "Donal, Herald of Wings",
    "Durkwood Tracker",
    "Energybending",
    "Gifts Ungiven",
    "Gluntch, the Bestower",
    "Gravestorm",
    "Harald Unites the Elves",
    "Huatli, Poet of Unity // Roar of the Fifth People",
    "Intuition",
    "Invasion of Arcavios // Invocation of the Founders",
    "J. Jonah Jameson",
    "Jace, Mirror Mage",
    "Kirol, Attentive First-Year",
    "Koth, Fire of Resistance",
    "Lich's Mastery",
    "Maddening Hex",
    "Mindlink Mech",
    "Minds Aglow",
    "Mirror Sheen",
    "Mischievous Quanar",
    "Mizzix, Replica Rider",
    "Nalfeshnee",
    "Nethroi, Apex of Death",
    "Nihil Spellbomb",
    "Peter Parker's Camera",
    "Professor Onyx",
    "Ratadrabik of Urborg",
    "Reality Smasher",
    "Reunion of the House",
    "Saheeli, Sublime Artificer",
    "Schema Thief",
    "Scheming Symmetry",
    "Secret Invasion",
    "Shared Trauma",
    "Sigil Tracer",
    "Silent Gravestone",
    "Start the TARDIS",
    "Summon: Primal Odin",
    "Sword of Hearth and Home",
    "Tezzeret, Cruel Machinist",
    "The Jolly Balloon Man",
    "The Phasing of Zhalfir",
    "The Pride of Hull Clade",
    "The Rise of Sozin // Fire Lord Sozin",
    "Triumph of Saint Katherine",
    "Ulalek, Fused Atrocity",
    "Ursine Champion",
];

#[test]
fn every_pack_card_reads_back_as_its_printed_text() {
    let corpus = common::Corpus::repo();
    let library = corpus.library();
    let printed = corpus.printed();
    let names: BTreeSet<String> = printed
        .iter()
        .map(|card| card.name.to_lowercase())
        .collect();
    let mut wrong = Vec::new();
    for name in PACK {
        let card = printed
            .iter()
            .find(|card| card.name == *name)
            .unwrap_or_else(|| panic!("{name}: no fixture"));
        let rules = library
            .card(&card.oracle_id)
            .unwrap_or_else(|| panic!("{name}: no rules file"));
        let text = serde_json::to_string(rules).unwrap();
        let report = check_exact(&text, card, &library, &names, &Caps::everything());
        if report.verdict() != Verdict::Accepted {
            let clauses: Vec<String> = report
                .clauses
                .iter()
                .filter(|clause| !matches!(clause.state, ClauseState::Reads { .. }))
                .map(|clause| match &clause.state {
                    ClauseState::Unwritten { ability, .. } => format!(
                        "  UNWRITTEN {:?}\n  {}",
                        clause.text,
                        report
                            .rules
                            .as_ref()
                            .and_then(|card| card.faces[clause.face].abilities.get(*ability))
                            .map(|it| serde_json::to_string(it).unwrap())
                            .unwrap_or_default()
                    ),
                    state => format!("  {:?} {} => {state:?}", clause.kind, clause.text),
                })
                .collect();
            wrong.push(format!(
                "{name}: {:?} {:?} strays {:?}\n{}",
                report.verdict(),
                report.problems,
                report.strays,
                clauses.join("\n")
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
