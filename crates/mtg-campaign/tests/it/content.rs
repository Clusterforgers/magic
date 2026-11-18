//! Goal: the shipped content holds together: every boss deck card and reward card is a real card the engine verifies, every boss deck as written before brackets has 40 cards, every custom card keeps all its abilities as an emblem, every content id a boss
//! names exists, relics carry a role, the scheme deck holds schemes and every boss has two minions to field and is pictured by its own real card.
use mtg_campaign::{CardKind, content};
use mtg_cards::support::Support;
use mtg_testkit::corpus;

/// The basic lands, which the engine knows without a rules file.
const BASICS: [&str; 6] = ["Plains", "Island", "Swamp", "Mountain", "Forest", "Wastes"];

#[test]
fn every_card_a_boss_or_reward_names_is_played_in_full() {
    let content = content().unwrap();
    let corpus = corpus::corpus().unwrap();
    for theme in &content.themes {
        let names = theme
            .bosses
            .iter()
            .flat_map(|b| {
                let brackets = (1..=mtg_campaign::scale::MAX_BRACKET).flat_map(|at| b.deck_at(at));
                b.deck
                    .iter()
                    .cloned()
                    .chain(brackets)
                    .map(|(name, _)| name)
                    .chain(std::iter::once(b.commander.clone()).filter(|c| !c.is_empty()))
                    .collect::<Vec<_>>()
            })
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
fn boss_decks_have_forty_cards_and_their_rules_exist() {
    let content = content().unwrap();
    for theme in &content.themes {
        assert_eq!(
            theme.bosses.iter().filter(|b| b.final_boss).count(),
            1,
            "{}",
            theme.id
        );
        for boss in &theme.bosses {
            if boss.since == 0 {
                let size: u32 = boss.deck.iter().map(|(_, n)| n).sum();
                assert_eq!(size, 40, "{}", boss.id);
            }
            for id in &boss.rules {
                assert_eq!(content.card(id).unwrap().kind, CardKind::Boss, "{id}");
            }
            let mut last = 100;
            for (id, percent) in &boss.phases {
                assert_eq!(content.card(id).unwrap().kind, CardKind::Phase, "{id}");
                assert!(*percent < last, "{}: phases go down", boss.id);
                last = *percent;
            }
        }
    }
}

#[test]
fn schemes_minions_and_scaling_cards_exist() {
    let content = content().unwrap();
    for id in [
        mtg_campaign::scale::REINFORCEMENTS,
        mtg_campaign::scale::RESERVES,
        mtg_campaign::scale::WARPATH,
        mtg_campaign::scale::ESCALATION,
    ] {
        assert_eq!(content.card(id).unwrap().kind, CardKind::Boss, "{id}");
    }
    assert_eq!(
        content.card(mtg_campaign::scale::BULWARK).unwrap().kind,
        CardKind::Phase
    );
    for theme in &content.themes {
        assert!(theme.schemes.len() >= 6, "{}: a scheme deck", theme.id);
        for id in &theme.schemes {
            assert_eq!(content.card(id).unwrap().kind, CardKind::Scheme, "{id}");
        }
        // The biggest party meets two minions in every fight.
        for boss in &theme.bosses {
            let fielded = theme
                .minions
                .iter()
                .filter(|m| m.deck_of != boss.id)
                .count();
            assert!(fielded >= 2, "{}: minions", boss.id);
        }
        for minion in &theme.minions {
            assert!(theme.boss(&minion.deck_of).is_some(), "{}", minion.id);
            assert!(minion.life > 0, "{}", minion.id);
            for id in &minion.rules {
                assert_eq!(content.card(id).unwrap().kind, CardKind::Boss, "{id}");
            }
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
