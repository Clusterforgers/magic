//! Whole abilities of the r65-02 pack whose sentences no other reader or writer words (names set by an effect,
//! Aura and Role forms, layer changes): each table entry is read from its sentence and worded back as the
//! card prints it.

use std::sync::LazyLock;

use super::tokens::same_written_text;
use crate::ability::Ability;
use crate::compile::classify;
use crate::refs::ZoneKind;
use crate::trigger::TriggerPattern;

/// The abilities, as JSON, each with its wording (`r6502_wholes.json`: a list of `[ability, wording]` pairs).
const R6502_WHOLES: &str = include_str!("r6502_wholes.json");

/// The entries parsed once.
static R6502_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R6502_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The ability of the entry whose sentence the clause is.
pub(super) fn r6502_read(text: &str) -> Option<Ability> {
    R6502_SHAPES
        .iter()
        .find(|(_, whole)| same_written_text(whole, text))
        .map(|(shape, _)| shape.clone())
}

/// Adds the wording of an ability that one of the entries matches whole; a Saga chapter (CR 714.2) is worded as the
/// sentence of the entry its effects make as a spell, behind its numeral.
pub(super) fn r6502_wordings(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R6502_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
    if let Ability::Triggered {
        when: TriggerPattern::Chapter { number },
        intervening_if: None,
        limit: None,
        from: ZoneKind::Battlefield,
        targets,
        effects,
    } = ability
        && let Some(numeral) = classify::numeral(*number)
    {
        let spell = Ability::Spell {
            targets: targets.clone(),
            effects: effects.clone(),
        };
        for (shape, text) in R6502_SHAPES.iter() {
            let worded = format!("{numeral} \u{2014} {text}");
            if *shape == spell && !texts.contains(&worded) {
                texts.push(worded);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses as an ability and reads back from its own sentence and from no near miss of
    //! it, so none is silently dropped or over-read.
    use super::{R6502_SHAPES, R6502_WHOLES, r6502_read, r6502_wordings};
    use crate::ability::Ability;
    use crate::compile::read::render::written_variants;
    use crate::refs::ZoneKind;
    use crate::trigger::TriggerPattern;
    use rstest::rstest;

    #[test]
    fn r6502_every_whole_parses_and_reads() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R6502_WHOLES).unwrap();
        assert_eq!(R6502_SHAPES.len(), entries.len());
        for (shape, text) in R6502_SHAPES.iter() {
            assert_eq!(r6502_read(text).as_ref(), Some(shape), "{text}");
            // A near miss: the same sentence with more after it, or with a word changed, reads as other text.
            assert!(
                r6502_read(&format!("{text} Draw a card.")).is_none(),
                "{text}"
            );
            assert!(
                r6502_read(&text.replacen(' ', " not ", 1)).is_none(),
                "{text}"
            );
        }
    }

    /// A Saga chapter made of the effects of a spell entry is worded as the numeral and the sentence (CR 714.2).
    #[rstest]
    #[case(1, "I")]
    #[case(2, "II")]
    #[case(3, "III")]
    #[case(4, "IV")]
    fn r6502_a_chapter_of_a_whole_spell_is_worded_with_its_numeral(
        #[case] number: u32,
        #[case] numeral: &str,
    ) {
        let mut chapters = 0;
        for (shape, text) in R6502_SHAPES.iter() {
            let Ability::Spell { targets, effects } = shape else {
                continue;
            };
            let chapter = Ability::Triggered {
                when: TriggerPattern::Chapter { number },
                intervening_if: None,
                limit: None,
                from: ZoneKind::Battlefield,
                targets: targets.clone(),
                effects: effects.clone(),
            };
            let mut texts = Vec::new();
            r6502_wordings(&chapter, &mut texts);
            assert_eq!(texts, vec![format!("{numeral} \u{2014} {text}")], "{text}");
            assert!(written_variants(&chapter).contains(&texts[0]), "{text}");
            chapters += 1;
        }
        assert!(chapters > 0);
    }
}
