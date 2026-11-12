//! Two steps a sentence joins to one target: "Return target creature you control and all Auras you control attached
//! to it to their owner's hand." (the Auras move first, then the creature) and "Destroy target creature and you lose
//! life equal to its toughness." (the toughness the target had as it last existed).

use mtg_core::card::Subtype;

use super::effects::target_object;
use super::tokens::Reader;
use crate::effect::{Destination, Effect};
use crate::refs::{Pred, Ref, Selector};
use crate::value::{Amount, StatKind};

/// "Return target creature you control and all Auras you control attached to it to their owner's hand."
pub(super) fn ce29_with_auras(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("return")?;
    let what = target_object(r)?;
    r.need("and all")?;
    // "Auras" inside the sentence is read as a name.
    (r.name()? == "Auras").then_some(())?;
    r.need("you control attached to it to their owner's hand")?;
    let auras = Ref::Each(Selector(vec![
        Pred::Subtype(Subtype("Aura".into())),
        Pred::ControlledBy(Ref::You),
        Pred::AttachedTo(what.clone()),
    ]));
    Some(vec![
        Effect::Move {
            what: auras,
            to: Destination::Hand,
            bind: None,
        },
        Effect::Move {
            what,
            to: Destination::Hand,
            bind: None,
        },
    ])
}

/// "Destroy target creature and you lose life equal to its toughness."
pub(super) fn ce29_destroy_and_lose(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("destroy")?;
    let what = target_object(r)?;
    r.need("and you lose life equal to its toughness")?;
    Some(vec![
        Effect::Destroy {
            what: what.clone(),
            regenerate: true,
            bind: None,
        },
        Effect::LoseLife {
            who: Ref::You,
            amount: Amount::Stat {
                of: what,
                stat: StatKind::Toughness,
            },
        },
    ])
}

#[cfg(test)]
mod tests {
    //! Goal: each joined sentence reads into its two steps on one target, and a sentence with another tail does not.
    use rstest::rstest;

    use super::super::tokens::{Reader, tokenize};
    use super::*;

    #[rstest]
    #[case::auras(
        "Return target creature you control and all Auras you control attached to it to their owner's hand.",
        true
    )]
    #[case::life(
        "Destroy target creature and you lose life equal to its toughness.",
        true
    )]
    #[case::other_tail(
        "Destroy target creature and you gain life equal to its toughness.",
        false
    )]
    #[case::opponents_auras(
        "Return target creature you control and all Auras your opponents control attached to it to their owner's hand.",
        false
    )]
    fn the_joined_sentences_read(#[case] text: &str, #[case] reads: bool) {
        let tokens = tokenize(text);
        let mut r = Reader::new(&tokens, false);
        let found = r
            .attempt(ce29_with_auras)
            .or_else(|| r.attempt(ce29_destroy_and_lose))
            .filter(|_| r.sentence_end());
        assert_eq!(found.as_ref().map(Vec::len), reads.then_some(2), "{text}");
        assert_eq!(r.targets.len(), usize::from(reads));
        let spell = super::super::clause_with(
            text,
            crate::compile::ClauseKind::Spell,
            false,
            &super::super::Own::default(),
        );
        if reads {
            assert!(spell.is_some(), "{text}");
        }
    }
}
