//! Goal: a static ability over goaded creatures applies to every creature a goad effect applies to, whichever of the two is older, since the selector reads the goad effects the pass has applied so far and the pass orders it after them.
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use rstest::rstest;
use std::sync::Arc;

use super::tests::{effect, named, rules};
use super::*;
use crate::characteristics::{cant_block, goaded_by};
use crate::fixtures::{creature, state};

const ME: PlayerId = PlayerId(0);

/// A creature of seat 0 with "Goaded creatures `whose` control can't block."
fn w8a_quasit(whose: &str) -> Arc<CardDefinition> {
    let affected =
        format!(r#"{{"Each":[{{"Type":"Creature"}},"Goaded",{{"ControlledBy":"{whose}"}}]}}"#);
    Arc::new(
        CardDefinition::new("Quasit")
            .with_type(CardType::Creature)
            .with_power_toughness(3, 2)
            .with_rules(rules(&format!(
                r#"{{"Static":{{"affected":{affected},"mods":["CantBlock"]}}}}"#
            ))),
    )
}

#[rstest]
#[case::goad_older_than_the_static(0)]
#[case::goad_newer_than_the_static(1_000)]
fn w8a_goaded_creatures_of_an_opponent_cannot_block(
    #[case] stamp: u64,
    #[values("Opponents", "You")] whose: &str,
) {
    let mut game = state(
        vec![vec![], vec![]],
        vec![
            vec![w8a_quasit(whose)],
            vec![creature("Bear", 2, 2), creature("Guard", 2, 2)],
        ],
    );
    effect(&mut game, "Bear", Modification::Goaded { by: ME }, stamp);
    let bear = named(&game, "Bear").id;
    let guard = named(&game, "Guard").id;
    assert_eq!(goaded_by(&game, bear), vec![ME]);
    assert_eq!(cant_block(&game, bear), whose == "Opponents");
    assert!(!cant_block(&game, guard));
}
