//! Goal: every boss Satchel card of the content has a bot cue, a card two bosses carry has one cue, and the Drowned
//! Spire's Satchel cards, which its content does not hold yet, have the brain's built-in cues.

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
fn the_spire_satchel_has_built_in_cues(
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
    assert!(satchel::builtin(card).is_some());
    assert!(satchel::cue_of(card).is_some());
}
