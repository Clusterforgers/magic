//! Goal: over the design corpus, a proposal that holds exactly the hand-written abilities the compiler does not read, merged with the compiler, keeps only hand-written abilities, loses and doubles no clause, and is a whole card whenever the compiler's own and the proposal's abilities cover every clause.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::card::ImplStatus;
use mtg_cards::compile::compile;
use mtg_cards::proposal::{check, merge};

use crate::common::{Corpus, printed};

/// The ability without the labels of its modes, as data.
fn data(ability: &Ability) -> String {
    let mut text = serde_json::to_string(ability).unwrap();
    while let Some(at) = text.find("\"label\":") {
        let end = text[at..]
            .find([',', '}'])
            .map_or(text.len(), |n| at + n + 1);
        text.replace_range(at..end, "");
    }
    text
}

#[test]
fn the_merge_of_the_compiler_and_the_rest_of_the_hand_written_file_is_the_hand_written_file() {
    let library = Corpus::repo().library();
    let cards = printed();
    let names: BTreeSet<String> = cards.iter().map(|c| c.name.to_lowercase()).collect();
    let caps = Caps::everything();
    let (mut tried, mut whole) = (0, 0);
    for printed in &cards {
        let Some(by_hand) = library.card(&printed.oracle_id) else {
            continue;
        };
        let Some(compiled) = compile(printed).card else {
            continue;
        };
        if compiled.impl_status == ImplStatus::Full {
            continue;
        }
        let read: BTreeSet<String> = compiled
            .abilities()
            .filter(|(_, ability)| !ability.is_manual())
            .map(|(_, ability)| data(ability))
            .collect();
        let mut rest = by_hand.clone();
        for face in &mut rest.faces {
            face.abilities.retain(|a| !read.contains(&data(a)));
        }
        let text = serde_json::to_string(&rest).unwrap();
        let proposal = check(&text, printed, &library, &names, &caps);
        let Some(merged) = merge(&proposal, printed, &library, &names, &caps) else {
            continue;
        };
        let Some(card) = merged.rules else { continue };
        tried += 1;
        let written: BTreeSet<String> = by_hand.abilities().map(|(_, a)| data(a)).collect();
        for (_, ability) in card.abilities().filter(|(_, a)| !a.is_manual()) {
            assert!(
                written.contains(&data(ability)),
                "{}: {ability:?} is not in the hand-written file",
                printed.name
            );
        }
        let once: BTreeSet<(usize, String)> = card
            .abilities()
            .filter(|(_, a)| !a.is_manual())
            .map(|(face, a)| (face, data(a)))
            .collect();
        let count = card.abilities().filter(|(_, a)| !a.is_manual()).count();
        assert_eq!(count, once.len(), "{}: an ability twice", printed.name);
        if card.impl_status == ImplStatus::Full && merged.problems.is_empty() {
            whole += 1;
            assert!(
                card.abilities().all(|(_, a)| !a.is_manual()),
                "{}: whole with a Manual ability",
                printed.name
            );
        }
    }
    assert!(tried >= 100, "only {tried} cards merged");
    assert!(
        whole >= 50,
        "only {whole} of {tried} merged cards are whole"
    );
}

mod refusal {
    //! Goal: a merged report is refused by name when it drops a clause the proposal covered or leaves an ability
    //! without a clause, and passes when it covers every clause once.
    use mtg_cards::proposal::{ClauseState, MergeRefusal, Report, Stray, refusal};
    use rstest::rstest;

    use super::*;

    const BOLT: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}]}]}"#;

    fn same(_: &mut Report) {}

    fn lose(report: &mut Report) {
        report.clauses[0].state = ClauseState::Open;
    }

    fn manual(report: &mut Report) {
        report.clauses[0].state = ClauseState::Manual {
            reason: "by hand".into(),
        };
    }

    fn double(report: &mut Report) {
        report.strays.push(Stray {
            face: 0,
            ability: 1,
            construct: "Spell".into(),
        });
    }

    #[rstest]
    #[case::the_same_cover(same, None)]
    #[case::a_clause_lost(lose, Some("merge drops a clause"))]
    #[case::a_clause_left_to_the_table(manual, Some("merge drops a clause"))]
    #[case::an_ability_doubled(double, Some("merge doubles a clause"))]
    fn merges_are_judged_by_their_clauses(
        #[case] change: fn(&mut Report),
        #[case] refused: Option<&str>,
    ) {
        let library = Corpus::repo().library();
        let cards = printed();
        let names: BTreeSet<String> = cards.iter().map(|c| c.name.to_lowercase()).collect();
        let bolt = cards.iter().find(|c| c.name == "Lightning Bolt").unwrap();
        let proposal = check(BOLT, bolt, &library, &names, &Caps::everything());
        let mut merged = proposal.clone();
        change(&mut merged);
        let found: Option<MergeRefusal> = refusal(&proposal, &merged);
        assert_eq!(found.as_ref().map(MergeRefusal::word), refused);
        if let Some(found) = found {
            assert_ne!(found.named(), "");
        }
    }
}

mod shared_sentence {
    //! Goal: the second of two abilities read as one sentence (`static_pairs`) keeps that sentence in a merge, so the
    //! merge loses neither the ability nor the clause.
    use mtg_cards::proposal::refusal;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::arrest("Arrest")]
    #[case::aether_tunnel("Aether Tunnel")]
    fn the_second_ability_of_a_sentence_stays(#[case] name: &str) {
        let library = Corpus::repo().library();
        let cards = printed();
        let names: BTreeSet<String> = cards.iter().map(|c| c.name.to_lowercase()).collect();
        let printed = cards.iter().find(|c| c.name == name).unwrap();
        let by_hand = library.card(&printed.oracle_id).unwrap();
        let text = serde_json::to_string(by_hand).unwrap();
        let caps = Caps::everything();
        let proposal = check(&text, printed, &library, &names, &caps);
        let merged = merge(&proposal, printed, &library, &names, &caps).unwrap();
        assert_eq!(refusal(&proposal, &merged), None, "{name}");
        let card = merged.rules.unwrap();
        assert_eq!(card.faces[0].abilities.len(), 3, "{name}");
    }
}
