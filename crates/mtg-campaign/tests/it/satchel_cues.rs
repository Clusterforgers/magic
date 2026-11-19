//! Goal: every boss Satchel card has a cue in the campaign content, a card two bosses carry has one cue, and the
//! cues the content gives are the ones a bot brain reads once installed.

use std::collections::BTreeMap;

use mtg_campaign::content;
use mtg_testkit::brain::satchel;
use rstest::rstest;

#[test]
fn every_boss_satchel_card_has_one_cue() {
    let content = content::content().unwrap();
    let mut seen = BTreeMap::new();
    let mut cards = 0;
    for boss in content.themes.iter().flat_map(|theme| &theme.bosses) {
        for card in &boss.satchel {
            cards += 1;
            let cue = card
                .cue
                .unwrap_or_else(|| panic!("{}: {} has no cue", boss.name, card.card));
            let first = *seen.entry(card.card.clone()).or_insert(cue);
            assert_eq!(first, cue, "{} has two cues", card.card);
        }
    }
    assert!(cards >= 64, "{cards} satchel cards");
    assert_eq!(content.satchel_cues(), seen);
}

#[rstest]
fn the_spire_satchel_cues_come_from_the_content(
    #[values(
        "Entrancing Melody",
        "Extract from Darkness",
        "Sudden Spoiling",
        "Aetherize",
        "Memory Plunder",
        "Reanimate",
        "Spelltwine",
        "Rise of the Dark Realms",
        "Grapple with the Past",
        "Octopus Umbra",
        "Sleep",
        "Crush of Tentacles"
    )]
    card: &str,
) {
    let cues = content::content().unwrap().satchel_cues();
    satchel::install(cues.clone());
    assert!(cues.contains_key(card), "{card} has no content cue");
    assert_eq!(satchel::cue_of(card), cues.get(card).copied());
}
