//! Goal: every boss of the content has its parts (one passive, an Enrage after the heroes' seventh turn, two phases, a
//! relic of its own, a modifier pool to draw from), at most one life drain with no way out and the words for the way out
//! of every other, a Hunting Party that spares commanders, and a scheme deck long enough for a fight at its cadence.
use std::collections::BTreeSet;

use mtg_campaign::content::Theme;
use mtg_campaign::scale::MOST;
use mtg_campaign::{Boss, CardKind, Scale, content};
use rstest::rstest;
use serde_json::{Value, json};

/// A player the boss's side can make lose life: every hero, or the hero in the spotlight.
const PLAYERS: [&str; 4] = ["Opponents", "EachPlayer", "Spotlit", "EachOpponent"];

/// How a card's life loss reaches a player: with no way out, or only unless the player pays a cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drain {
    Unavoidable,
    UnlessPaid,
}

/// The life losses (and damage to players) in `value`, an ability tree in the card DSL, each classified by whether a
/// `Pay` the player may make stands before it.
fn drains(value: &Value, paid_off: bool, out: &mut Vec<Drain>) {
    let kind = if paid_off {
        Drain::UnlessPaid
    } else {
        Drain::Unavoidable
    };
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                match key.as_str() {
                    "LoseLife" => out.push(kind),
                    "DealDamage"
                        if inner["to"].as_str().is_some_and(|to| PLAYERS.contains(&to)) =>
                    {
                        out.push(kind);
                    }
                    "Pay" => {
                        drains(&inner["paid"], paid_off, out);
                        drains(&inner["unpaid"], true, out);
                        continue;
                    }
                    _ => {}
                }
                drains(inner, paid_off, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                drains(item, paid_off, out);
            }
        }
        _ => {}
    }
}

/// The drains of the content card `id`.
fn drains_of(id: &str) -> Vec<Drain> {
    let card = content().unwrap().card(id).unwrap();
    let mut out = Vec::new();
    drains(
        &serde_json::to_value(&card.abilities).unwrap(),
        false,
        &mut out,
    );
    out
}

/// Every boss-side card a fight against `boss` can bring: its rules at every party size, its relic and phases, the
/// theme's schemes and modifiers and the minions' rules.
fn boss_side_cards(theme: &Theme, boss: &Boss) -> BTreeSet<String> {
    (1..=MOST)
        .flat_map(|heroes| Scale::new(heroes, 0).rules(boss))
        .chain(
            Scale::new(MOST, 0)
                .phases(boss, 100)
                .into_iter()
                .map(|(id, _)| id),
        )
        .chain([boss.relic.clone()])
        .chain(theme.schemes.iter().cloned())
        .chain(theme.modifiers.iter().cloned())
        .chain(theme.minions.iter().flat_map(|m| m.rules.iter().cloned()))
        .collect()
}

#[test]
fn a_boss_has_at_most_one_life_drain_with_no_way_out_and_words_the_way_out_of_the_others() {
    let content = content().unwrap();
    for theme in &content.themes {
        for boss in &theme.bosses {
            let mut unavoidable = Vec::new();
            for id in boss_side_cards(theme, boss) {
                let found = drains_of(&id);
                if found.contains(&Drain::UnlessPaid) {
                    let text = &content.card(&id).unwrap().text;
                    assert!(text.contains("unless"), "{id} names its way out");
                }
                unavoidable.extend(
                    found
                        .iter()
                        .filter(|d| **d == Drain::Unavoidable)
                        .map(|_| id.clone()),
                );
            }
            assert!(
                unavoidable.len() <= 1,
                "{}: drains with no way out: {unavoidable:?}",
                boss.id
            );
        }
    }
}

#[rstest]
#[case::escalation_has_no_way_out("escalation", &[Drain::Unavoidable])]
#[case::a_toll_is_paid_off_with_mana("deaths_toll", &[Drain::UnlessPaid])]
#[case::marked_prey_is_paid_off_with_a_discard("marked_prey", &[Drain::UnlessPaid])]
#[case::feast_of_rats_is_paid_off("feast_of_rats", &[Drain::UnlessPaid])]
#[case::the_last_phases_are_paid_off("plague_tide", &[Drain::UnlessPaid])]
#[case::hunting_party_drains_nothing("hunting_party", &[])]
fn the_drain_reader_classifies_each_life_loss(#[case] id: &str, #[case] expected: &[Drain]) {
    assert_eq!(drains_of(id), expected);
}

#[test]
fn every_boss_has_a_passive_an_enrage_two_phases_and_a_relic_of_its_own() {
    let content = content().unwrap();
    for theme in &content.themes {
        let mut seen = BTreeSet::new();
        for boss in &theme.bosses {
            assert_eq!(boss.rules.len(), 1, "{}: one passive", boss.id);
            assert_eq!(content.card(&boss.rules[0]).unwrap().kind, CardKind::Boss);
            let enrage = content.card(&boss.enrage).unwrap();
            assert_eq!(enrage.kind, CardKind::Enrage, "{}", boss.id);
            assert_eq!(
                enrage.turn, 7,
                "{}: enrage after the heroes' seventh turn",
                boss.id
            );
            // Every ability of the Enrage waits for the heroes' turns its record names.
            let abilities = serde_json::to_value(&enrage.abilities).unwrap();
            for ability in abilities.as_array().unwrap() {
                let condition = &ability["Triggered"]["if"]["Compare"];
                assert_eq!(
                    condition["left"]["TurnsTaken"], "Opponents",
                    "{}",
                    enrage.id
                );
                assert_eq!(condition["op"], "Ge", "{}", enrage.id);
                assert_eq!(condition["right"], enrage.turn, "{}", enrage.id);
            }
            assert_eq!(boss.phases.len(), 2, "{}: two phases", boss.id);
            assert_eq!(content.card(&boss.relic).unwrap().kind, CardKind::BossRelic);
            let own = boss
                .rules
                .iter()
                .chain([&boss.enrage, &boss.relic])
                .chain(boss.phases.iter().map(|(id, _)| id));
            for id in own {
                assert!(seen.insert(id.clone()), "{id} is shared between bosses");
            }
        }
        assert!(theme.modifiers.len() >= 4, "{}: a modifier pool", theme.id);
        for id in &theme.modifiers {
            assert_eq!(content.card(id).unwrap().kind, CardKind::Modifier, "{id}");
        }
    }
}

#[test]
fn hunting_party_spares_commanders() {
    let card = content().unwrap().card("hunting_party").unwrap();
    let abilities = serde_json::to_value(&card.abilities).unwrap();
    let filter = &abilities[0]["Triggered"]["effects"][0]["Sacrifice"]["filter"];
    assert!(
        filter
            .as_array()
            .unwrap()
            .contains(&json!({"Not": "Commander"})),
        "{filter}"
    );
}

#[test]
fn the_scheme_deck_outlasts_a_fight_at_one_scheme_every_other_boss_turn() {
    let content = content().unwrap();
    for theme in &content.themes {
        assert_eq!(theme.scheme_every, 2, "{}", theme.id);
        let distinct: BTreeSet<_> = theme.schemes.iter().collect();
        assert_eq!(
            distinct.len(),
            theme.schemes.len(),
            "{}: no scheme twice",
            theme.id
        );
        // From the boss's third turn, every other one: a fight of 30 boss turns sees each scheme at most once.
        let set = (3..=30)
            .step_by(usize::try_from(theme.scheme_every).unwrap())
            .count();
        assert!(
            theme.schemes.len() >= set,
            "{}: {} schemes for {set}",
            theme.id,
            theme.schemes.len()
        );
    }
}
