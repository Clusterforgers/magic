//! r55-00 readers of whole abilities: a cost permission that works once each turn on any turn ("Once each turn, you may
//! pay {0} rather than pay the mana cost for a spell you cast with mana value X or less, where X is the number of time
//! counters on this enchantment."). Each reading is kept only when its writer gives the text back.

use super::ClauseKind;
use super::selectors::{Form, phrase};
use super::tokens::{Reader, same_text, tokenize};
use super::written_variants;
use crate::ability::Ability;
use crate::modify::{CastPermission, GameRule};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::{Amount, Cmp, CounterKind, StatKind};

/// Reads a whole clause of the r55-00 wordings, kept only when it is written back as `text`.
pub(super) fn r5500_read(text: &str, kind: ClauseKind) -> Option<Ability> {
    let found = match kind {
        ClauseKind::Static => {
            r5500_once_rather_read(text).or_else(|| r5500_tax_as_life_read(text))?
        }
        _ => return None,
    };
    written_variants(&found)
        .iter()
        .any(|written| same_text(written, text))
        .then_some(found)
}

/// The permission, once each turn on any turn, to pay `cost` rather than the mana cost of a spell `what` picks whose mana
/// value is at most the number of `counter` counters on the source.
pub(in crate::compile::read) fn r5500_once_rather(
    what: &Selector,
    cost: crate::cost::Cost,
    counter: CounterKind,
) -> Ability {
    let mut what = what.clone();
    what.0.push(Pred::Compare {
        stat: StatKind::ManaValue,
        op: Cmp::Le,
        to: Amount::Counters {
            on: Ref::This,
            counter,
        },
    });
    Ability::Rule {
        rule: GameRule::MayCast(CastPermission {
            what,
            once: true,
            any_turn: true,
            rather: Some(cost),
            any_cast: true,
            ..CastPermission::default()
        }),
        condition: None,
        from: ZoneKind::Battlefield,
    }
}

/// "Once each turn, you may pay <mana> rather than pay the mana cost for a <spell> you cast with mana value X or less,
/// where X is the number of <kind> counters on ~."
fn r5500_once_rather_read(text: &str) -> Option<Ability> {
    let tokens = tokenize(text);
    let mut r = Reader::new(&tokens, false);
    r.need("once each turn")?;
    r.punct(',').then_some(())?;
    r.need("you may pay")?;
    let cost = super::effects::mana_cost(&mut r)?;
    r.need("rather than pay the mana cost for")?;
    (r.word("a") || r.word("an")).then_some(())?;
    let spell = phrase(&mut r, Form::Subject)?;
    r.need("you cast with mana value x or less")?;
    r.punct(',').then_some(())?;
    r.need("where x is the number of")?;
    let counter = super::counters::kind(&mut r)?;
    r.need("counters on")?;
    r.source().then_some(())?;
    (r.sentence_end() && r.end()).then_some(())?;
    let what = Selector(
        spell
            .0
            .into_iter()
            .filter(|pred| *pred != Pred::Spell)
            .collect(),
    );
    Some(r5500_once_rather(&what, cost, counter))
}

/// The commander tax of the card paid as `life` life for each previous cast from the command zone.
pub(in crate::compile::read) const fn r5500_tax_as_life(life: u32) -> Ability {
    Ability::Rule {
        rule: GameRule::TaxAsLife(life),
        condition: None,
        from: ZoneKind::Battlefield,
    }
}

/// "Rather than pay {2} for each previous time you've cast this spell from the command zone this game, pay N life that
/// many times."
fn r5500_tax_as_life_read(text: &str) -> Option<Ability> {
    let tokens = tokenize(text);
    let mut r = Reader::new(&tokens, false);
    r.need("rather than pay")?;
    (r.symbol()? == "2").then_some(())?;
    r.need("for each previous time you've cast")?;
    (r.source() || r.words("this spell")).then_some(())?;
    r.need("from the command zone this game")?;
    r.punct(',').then_some(())?;
    r.need("pay")?;
    let life = r.number()?;
    r.need("life that many times")?;
    (r.sentence_end() && r.end()).then_some(())?;
    let life = Some(life).filter(|life| *life > 0)?;
    Some(r5500_tax_as_life(life))
}

#[cfg(test)]
mod tests {
    //! Goal: the r55-00 readers read their wordings into the abilities their writers give back, and refuse near misses
    //! that say another limit, another cost or another counter.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::as_foretold(
        "Once each turn, you may pay {0} rather than pay the mana cost for a spell you cast with mana value X or less, where X is the number of time counters on ~.",
        true
    )]
    #[case::creature_spell(
        "Once each turn, you may pay {1} rather than pay the mana cost for a creature spell you cast with mana value X or less, where X is the number of charge counters on ~.",
        true
    )]
    #[case::your_turns_only(
        "Once during each of your turns, you may pay {0} rather than pay the mana cost for a spell you cast with mana value X or less, where X is the number of time counters on ~.",
        false
    )]
    #[case::x_cost(
        "Once each turn, you may pay {X} rather than pay the mana cost for a spell you cast with mana value X or less, where X is the number of time counters on ~.",
        false
    )]
    #[case::or_greater(
        "Once each turn, you may pay {0} rather than pay the mana cost for a spell you cast with mana value X or greater, where X is the number of time counters on ~.",
        false
    )]
    #[case::other_permanent(
        "Once each turn, you may pay {0} rather than pay the mana cost for a spell you cast with mana value X or less, where X is the number of time counters on target artifact.",
        false
    )]
    fn r5500_once_rather_reads(#[case] text: &str, #[case] reads: bool) {
        assert_eq!(
            r5500_read(text, ClauseKind::Static).is_some(),
            reads,
            "{text}"
        );
    }

    #[rstest]
    #[case::liesa(
        "Rather than pay {2} for each previous time you've cast this spell from the command zone this game, pay 2 life that many times.",
        true
    )]
    #[case::three_life(
        "Rather than pay {2} for each previous time you've cast this spell from the command zone this game, pay 3 life that many times.",
        true
    )]
    #[case::other_tax(
        "Rather than pay {3} for each previous time you've cast this spell from the command zone this game, pay 2 life that many times.",
        false
    )]
    #[case::once(
        "Rather than pay {2} for each previous time you've cast this spell from the command zone this game, pay 2 life.",
        false
    )]
    #[case::from_hand(
        "Rather than pay {2} for each previous time you've cast this spell from your hand this game, pay 2 life that many times.",
        false
    )]
    fn r5500_tax_as_life_reads(#[case] text: &str, #[case] reads: bool) {
        assert_eq!(
            r5500_read(text, ClauseKind::Static).is_some(),
            reads,
            "{text}"
        );
    }

    #[test]
    fn r5500_once_rather_is_any_turn_and_counts_the_source() {
        let text = "Once each turn, you may pay {0} rather than pay the mana cost for a spell you cast with mana value X or less, where X is the number of time counters on ~.";
        let found = r5500_read(text, ClauseKind::Static);
        let Some(Ability::Rule {
            rule: GameRule::MayCast(permission),
            ..
        }) = found
        else {
            panic!("not read: {text}");
        };
        assert!(permission.once && permission.any_turn && permission.any_cast);
        assert_eq!(
            permission.what.0,
            vec![Pred::Compare {
                stat: StatKind::ManaValue,
                op: Cmp::Le,
                to: Amount::Counters {
                    on: Ref::This,
                    counter: CounterKind::Other("time".to_owned()),
                },
            }]
        );
    }
}
