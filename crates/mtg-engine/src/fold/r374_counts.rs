//! Goal: the counts of an object's own types and keyword values (`StatKind::R374*`) and the different mana costs and color
//! pairs among objects read the game as the rules say, a cap holds and an object with every creature type reaches it, at
//! tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_cards::value::{Amount, Cmp, StatKind};
use mtg_core::card::{CardType, Supertype};
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use rstest::rstest;

use crate::fixtures::state;
use crate::resolve::{ResolveError, Scope};
use crate::state::GameState;
use crate::zones::Zone;

fn card(name: &str, cost: &str) -> CardDefinition {
    CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_mana_cost(ManaCost::parse(cost).unwrap())
        .with_power_toughness(1, 1)
}

/// Seat 0 controls the cards of the cases; every other seat a two-colored Bear.
fn table(seats: usize) -> GameState {
    let mine = vec![
        // A legendary Elf Druid: 1 supertype, 1 card type, 2 subtypes.
        Arc::new(
            card("Druid", "{1}{G}")
                .with_supertype(Supertype::Legendary)
                .with_subtype("Elf")
                .with_subtype("Druid"),
        ),
        // An artifact creature Golem with toxic 2 and toxic 1 and bushido 2: 2 card types, 1 subtype.
        Arc::new(
            card("Golem", "{1}{G}")
                .with_type(CardType::Artifact)
                .with_subtype("Golem")
                .with_rules(Ability::Keyword(Keyword::Toxic(2)))
                .with_rules(Ability::Keyword(Keyword::Toxic(1)))
                .with_rules(Ability::Keyword(Keyword::Macro {
                    name: "bushido".to_owned(),
                    amount: 2,
                })),
        ),
        // A changeling: every creature type.
        Arc::new(card("Shapeshifter", "{W}{U}").with_rules(Ability::Keyword(Keyword::Changeling))),
        // Two colors, as the Shapeshifter but another pair.
        Arc::new(card("Rakdos", "{B}{R}")),
        Arc::new(card("Azorius", "{1}{W}{U}")),
    ];
    let field = |seat: usize| {
        if seat == 0 {
            mine.clone()
        } else {
            vec![Arc::new(card("Bear", "{G}{U}").with_subtype("Bear"))]
        }
    };
    state(
        (0..seats).map(|_| Vec::new()).collect(),
        (0..seats).map(field).collect(),
    )
}

fn named(name: &str) -> Ref {
    Ref::Each(Selector(vec![Pred::Named(name.to_owned())]))
}

fn amount(state: &GameState, amount: &Amount) -> Result<i64, ResolveError> {
    let source = state.zone(Zone::Battlefield)[0];
    let source = state.object(source).unwrap().reference();
    Scope::announcing(state, PlayerId(0), source).amount(amount)
}

fn stat(of: &str, stat: StatKind) -> Amount {
    Amount::Stat {
        of: named(of),
        stat,
    }
}

#[rstest]
#[case::types_of_a_legend(stat("Druid", StatKind::R374Types), 4)]
#[case::types_of_an_artifact_creature(stat("Golem", StatKind::R374Types), 3)]
#[case::creature_types(stat("Druid", StatKind::R374CreatureTypes), 2)]
#[case::toxic_adds_up(stat("Golem", StatKind::R374Toxic), 3)]
#[case::no_toxic(stat("Druid", StatKind::R374Toxic), 0)]
#[case::bushido(stat("Golem", StatKind::R374Bushido), 2)]
#[case::cap_holds_below(
    Amount::R374AtMost(Box::new(stat("Druid", StatKind::R374CreatureTypes)), 1),
    1
)]
#[case::cap_leaves_less(
    Amount::R374AtMost(Box::new(stat("Druid", StatKind::R374CreatureTypes)), 10),
    2
)]
#[case::every_creature_type_reaches_the_cap(
    Amount::R374AtMost(Box::new(stat("Shapeshifter", StatKind::R374CreatureTypes)), 10),
    10
)]
fn counts_of_an_object(
    #[values(2, 3, 4)] seats: usize,
    #[case] counted: Amount,
    #[case] expected: i64,
) {
    assert_eq!(amount(&table(seats), &counted).unwrap(), expected);
}

/// Without a cap the engine refuses a count of the types of an object with every creature type.
#[rstest]
fn every_creature_type_uncapped_is_refused(#[values(2, 3, 4)] seats: usize) {
    let refused = amount(&table(seats), &stat("Shapeshifter", StatKind::R374Types));
    assert!(
        matches!(refused, Err(ResolveError::Unsupported(_))),
        "{refused:?}"
    );
}

#[rstest]
#[case::mana_costs_of_mine(vec![Pred::ControlledBy(Ref::You)], StatKind::R374ManaCost, 4)]
#[case::mana_costs_of_all(vec![], StatKind::R374ManaCost, 5)]
#[case::color_pairs_of_mine(vec![Pred::ControlledBy(Ref::You)], StatKind::R374ColorSet, 2)]
#[case::color_pairs_of_all(vec![], StatKind::R374ColorSet, 3)]
fn different_values_among(
    #[values(2, 3, 4)] seats: usize,
    #[case] mut of: Vec<Pred>,
    #[case] stat: StatKind,
    #[case] expected: i64,
) {
    if stat == StatKind::R374ColorSet {
        of.push(Pred::Compare {
            stat: StatKind::Colors,
            op: Cmp::Eq,
            to: Amount::Fixed(2),
        });
    }
    let found = amount(
        &table(seats),
        &Amount::Distinct {
            of: Selector(of),
            stat,
        },
    )
    .unwrap();
    assert_eq!(found, expected);
}
