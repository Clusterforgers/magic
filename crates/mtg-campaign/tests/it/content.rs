//! Goal: the shipped content holds together: every satchel and reward card is a real card the engine plays in full,
//! every boss names its deck file, innate, two phases and four satchel cards, each theme has a final boss, every custom
//! card keeps all its abilities as an emblem, relics carry a role, and every boss is pictured by its own real card.
use mtg_campaign::{CardKind, content};
use mtg_cards::support::Support;
use mtg_testkit::corpus;

/// The basic lands, which the engine knows without a rules file.
const BASICS: [&str; 6] = ["Plains", "Island", "Swamp", "Mountain", "Forest", "Wastes"];

#[test]
fn every_satchel_or_reward_card_is_played_in_full() {
    let content = content().unwrap();
    let corpus = corpus::corpus().unwrap();
    for theme in &content.themes {
        let names = theme
            .bosses
            .iter()
            .flat_map(|b| b.satchel.iter().map(|s| s.card.clone()))
            .chain(theme.pool.iter().map(|(name, _)| name.clone()))
            .collect::<Vec<_>>();
        for name in names.iter().filter(|name| !BASICS.contains(&name.as_str())) {
            let support = corpus
                .support(name)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(
                matches!(support.support, Support::Verified | Support::Read),
                "{name} is not played in full"
            );
        }
    }
}

#[test]
fn every_boss_names_its_parts() {
    let content = content().unwrap();
    for theme in &content.themes {
        assert!(theme.bosses.iter().any(|b| b.final_boss), "{}", theme.id);
        for boss in &theme.bosses {
            let deck = content.deck(&boss.deck_file).unwrap();
            assert_eq!(
                deck.cards.iter().map(|(_, n)| n).sum::<u32>(),
                100,
                "{}",
                boss.id
            );
            let innate = mtg_campaign::innate::innate_id(&boss.innate, 1);
            assert_eq!(
                content.card(&innate).unwrap().kind,
                CardKind::Innate,
                "{}",
                boss.id
            );
            assert_eq!(boss.round_phases.len(), 2, "{}", boss.id);
            for id in &boss.round_phases {
                let id = content
                    .card(id)
                    .or_else(|_| content.card(&mtg_campaign::innate::innate_id(id, 1)));
                assert_eq!(id.unwrap().kind, CardKind::Phase);
            }
            assert_eq!(boss.satchel.len(), 4, "{}", boss.id);
        }
    }
}

#[test]
fn every_custom_card_keeps_all_its_abilities_as_an_emblem() {
    let content = content().unwrap();
    for card in content.cards.values() {
        let definition = card.definition();
        assert_eq!(definition.name, card.name);
        assert_eq!(
            definition.rules.len(),
            card.abilities.len(),
            "{} lost an ability the engine does not run",
            card.id
        );
        if card.kind == CardKind::Relic {
            assert!(
                ["Healer", "Removal", "Damage", "Support", "Risky"].contains(&card.role.as_str()),
                "{}",
                card.id
            );
            assert_eq!(
                card.price == 0,
                card.role == "Risky",
                "{}: only risky relics are not sold",
                card.id
            );
        }
    }
    assert!(content.haunt().is_some());
}

#[test]
fn every_boss_is_pictured_by_a_real_card_whose_oracle_id_matches_its_name() {
    let content = content().unwrap();
    let cards: Vec<serde_json::Value> =
        serde_json::from_str(&corpus::scryfall_array().unwrap()).unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for boss in content.themes.iter().flat_map(|theme| &theme.bosses) {
        let card = cards
            .iter()
            .find(|card| card["name"] == boss.art.card.as_str())
            .unwrap_or_else(|| panic!("{}: no card {}", boss.id, boss.art.card));
        assert_eq!(
            card["oracle_id"],
            boss.art.oracle_id.as_str(),
            "{}",
            boss.id
        );
        assert!(
            seen.insert(&boss.art.oracle_id),
            "{} shares its art",
            boss.id
        );
    }
}
