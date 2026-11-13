//! Goal: the shipped content holds together: every boss deck card and reward card is a real card the engine verifies, every boss deck has 40 cards, every custom card keeps all its abilities as an emblem, every content id a boss
//! names exists, and relics carry a role.
use mtg_campaign::{CardKind, content};
use mtg_cards::support::Support;
use mtg_testkit::corpus;

/// The basic lands, which the engine knows without a rules file.
const BASICS: [&str; 5] = ["Plains", "Island", "Swamp", "Mountain", "Forest"];

#[test]
fn every_card_a_boss_or_reward_names_is_played_in_full() {
    let content = content().unwrap();
    let corpus = corpus::corpus().unwrap();
    for theme in &content.themes {
        let names = theme
            .bosses
            .iter()
            .flat_map(|b| b.deck.iter().map(|(name, _)| name.as_str()))
            .chain(theme.pool.iter().map(|(name, _)| name.as_str()));
        for name in names.filter(|name| !BASICS.contains(name)) {
            let support = corpus
                .support(name)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(
                support.support,
                Support::Verified,
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
            let size: u32 = boss.deck.iter().map(|(_, n)| n).sum();
            assert_eq!(size, 40, "{}", boss.id);
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
