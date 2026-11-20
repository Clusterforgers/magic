//! r112-04 abilities whose wordings no other reader or writer joins (words noted on a permanent, stickers, attachments
//! and abilities given for a while): each entry of `r112_04_wholes.json` is an ability with the words the card prints
//! for it (the card's name as `~`), read when a clause says exactly those words and written back from the ability. A
//! Saga chapter (CR 714.2) whose effects are an entry's spell is written as that entry's sentence.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;
use crate::refs::ZoneKind;
use crate::trigger::TriggerPattern;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const R112_04_WHOLES: &str = include_str!("r112_04_wholes.json");

/// The entries parsed once.
static R112_04_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R112_04_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The ability of the entry whose wording is the clause.
pub(super) fn r112_04_whole_read(text: &str) -> Option<Ability> {
    R112_04_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(ability, _)| ability.clone())
}

/// Adds the wording of an ability that one of the entries matches whole, a chapter as the sentence of its spell.
pub(super) fn r112_04_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    let spell = match ability {
        Ability::Triggered {
            when: TriggerPattern::Chapter { .. },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets,
            effects,
        } => Some(Ability::Spell {
            targets: targets.clone(),
            effects: effects.clone(),
        }),
        _ => None,
    };
    for (shape, text) in R112_04_SHAPES.iter() {
        if (shape == ability || spell.as_ref() == Some(shape)) && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, a chapter of an entry's
    //! spell is written as its sentence, and a near miss of its words (a kind, a step, a number or a recipient changed)
    //! reads as nothing.
    use super::{R112_04_SHAPES, R112_04_WHOLES, r112_04_add_wholes, r112_04_whole_read};
    use crate::ability::Ability;
    use crate::refs::ZoneKind;
    use crate::trigger::TriggerPattern;

    #[test]
    fn r112_04_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R112_04_WHOLES).unwrap();
        assert_eq!(R112_04_SHAPES.len(), entries.len());
        for (ability, text) in R112_04_SHAPES.iter() {
            assert_eq!(r112_04_whole_read(text).as_ref(), Some(ability), "{text}");
            let mut texts = Vec::new();
            r112_04_add_wholes(ability, &mut texts);
            assert!(texts.contains(text), "{text}");
        }
    }

    #[rstest::rstest]
    fn r112_04_a_chapter_of_a_whole_spell_is_written_as_its_sentence(
        #[values(1, 3, 6)] number: u32,
    ) {
        let mut chapters = 0;
        for (shape, text) in R112_04_SHAPES.iter() {
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
            r112_04_add_wholes(&chapter, &mut texts);
            assert_eq!(texts, vec![text.clone()], "{text}");
            chapters += 1;
        }
        assert!(chapters > 0);
    }

    #[test]
    fn r112_04_a_near_miss_reads_as_nothing() {
        let swaps = [
            ("hasn't been noted", "has been noted"),
            (
                "a creature spell of that type",
                "an artifact spell of that type",
            ),
            ("+1/+1 counter", "-1/-1 counter"),
            ("this turn", "this game"),
            ("name sticker", "art sticker"),
            ("enchant creature", "enchant artifact"),
            ("from your graveyard", "from your hand"),
            ("sacrifice enchanted creature", "sacrifice this Aura"),
            ("they gain cascade", "they gain flash"),
            ("from your hand this turn", "from exile this turn"),
            ("sticker kicker {1}", "sticker kicker {2}"),
            ("Creature spells you cast", "Artifact spells you cast"),
            ("you lose control", "you gain control"),
            ("refine counter", "time counter"),
        ];
        for (_, text) in R112_04_SHAPES.iter() {
            for (from, to) in swaps {
                if text.contains(from) {
                    let near = text.replacen(from, to, 1);
                    assert_eq!(r112_04_whole_read(&near), None, "{near}");
                }
            }
        }
    }
}
