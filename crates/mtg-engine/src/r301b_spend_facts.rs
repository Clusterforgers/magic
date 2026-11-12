//! Goal: a spend restriction reads whether a spell has freerunning and whether it has no abilities at all (a keyword, a
//! rule or a clause left to the table each counts as one), from the spell as it is being cast.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ManualClause;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_cards::spend::{SpendFact, SpendFilter, SpendOn};
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use rstest::rstest;

use crate::fixtures::state;
use crate::spend::Spend;

/// What the creature card has beside its cost and stats.
#[derive(Debug, Clone, Copy)]
enum Has {
    Nothing,
    Freerunning,
    Flying,
    Rule,
    Clause,
}

/// A creature card in seat 0's hand with what `has` gives it, and the payment of its cast.
fn spend_for(has: Has) -> Spend {
    let mut card = CardDefinition::new("Spell")
        .with_type(CardType::Creature)
        .with_power_toughness(2, 2)
        .with_mana_cost(ManaCost::parse("{1}{G}").unwrap());
    card = match has {
        Has::Nothing => card,
        Has::Freerunning => card.with_rules(Ability::Keyword(Keyword::Freerunning(Cost::mana(
            "{B}".parse().unwrap(),
        )))),
        Has::Flying => card.with_rules(Ability::Keyword(Keyword::Flying)),
        Has::Rule => card.with_rules(
            serde_json::from_value(serde_json::json!({"Triggered": {
                "when": {"ZoneChange": {"what": ["This"], "to": "Battlefield"}},
                "effects": [{"Draw": {}}]}}))
            .unwrap(),
        ),
        Has::Clause => {
            card.manual.push(ManualClause {
                text: "Do something by hand.".to_owned(),
                reason: "by hand".to_owned(),
            });
            card
        }
    };
    let game = state(
        vec![vec![Arc::new(card)], Vec::new()],
        vec![Vec::new(), Vec::new()],
    );
    let id = game
        .objects()
        .find(|object| object.card().name == "Spell")
        .map(crate::object::GameObject::id)
        .unwrap();
    Spend::for_spell(&game, id, PlayerId(0), false)
}

fn only(fact: SpendFact) -> [SpendOn; 1] {
    [SpendOn::Spell(SpendFilter {
        facts: vec![fact],
        ..SpendFilter::default()
    })]
}

#[rstest]
#[case::freerunning(Has::Freerunning, SpendFact::Freerunning, true)]
#[case::no_freerunning(Has::Flying, SpendFact::Freerunning, false)]
#[case::vanilla(Has::Nothing, SpendFact::NoAbilities, true)]
#[case::a_keyword(Has::Flying, SpendFact::NoAbilities, false)]
#[case::a_rule(Has::Rule, SpendFact::NoAbilities, false)]
#[case::a_clause_by_hand(Has::Clause, SpendFact::NoAbilities, false)]
#[case::freerunning_is_an_ability(Has::Freerunning, SpendFact::NoAbilities, false)]
fn a_spell_is_read_for_freerunning_and_for_no_abilities(
    #[case] has: Has,
    #[case] fact: SpendFact,
    #[case] allowed: bool,
) {
    assert_eq!(spend_for(has).allows(&only(fact)), allowed);
}
