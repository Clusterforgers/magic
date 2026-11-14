//! The words of name stickers (CR 123): "for each name sticker on this Aura with eight or more letters", "... with seven
//! or fewer letters", "for each name sticker on it that begins with the chosen letter", "the number of u's in name
//! stickers on this enchantment" and "choose a letter", read into `Amount::NameStickers`, `Amount::StickerLetters` and
//! `ValueKind::Letter` and written back the same way.

use super::amounts::{count_word, number_word};
use super::tokens::Reader;
use crate::ability::NameWords;
use crate::refs::Ref;
use crate::value::Amount;

/// The name a resolution keeps the chosen letter under.
pub(super) const LETTER: &str = "letter";

/// "it" or "~": the object with the ability.
fn this(r: &mut Reader<'_>) -> Option<()> {
    (r.word("it") || r.source()).then_some(())
}

/// After "for each": "name sticker on ~ with eight or more letters", "name sticker on it that begins with the chosen
/// letter".
pub(super) fn r6101_name_stickers(r: &mut Reader<'_>) -> Option<Amount> {
    r.need("name sticker on")?;
    this(r)?;
    let words = if r.words("that begins with the chosen letter") {
        NameWords::BeginsWithChosen(LETTER.to_owned())
    } else {
        r.need("with")?;
        let count = number_word(r.take_word()?)?;
        let words = if r.words("or more") {
            NameWords::LettersAtLeast(count)
        } else {
            r.need("or fewer")?;
            NameWords::LettersAtMost(count)
        };
        r.need("letters")?;
        words
    };
    Some(Amount::NameStickers {
        on: Ref::This,
        words,
    })
}

/// After "the number of": "u's in name stickers on this enchantment".
pub(super) fn r6101_letters_in(r: &mut Reader<'_>) -> Option<Amount> {
    let word = r.take_word()?;
    let letter = word.strip_suffix("'s")?;
    let mut chars = letter.chars();
    let (Some(letter), None) = (chars.next(), chars.next()) else {
        return None;
    };
    letter.is_ascii_lowercase().then_some(())?;
    r.need("in name stickers on")?;
    this(r)?;
    Some(Amount::StickerLetters {
        on: Ref::This,
        letter: letter.to_string(),
    })
}

/// What `r6101_name_stickers` read, after "for each ".
pub(super) fn r6101_name_stickers_text(amount: &Amount) -> Option<String> {
    let Amount::NameStickers {
        on: Ref::This,
        words,
    } = amount
    else {
        return None;
    };
    Some(match words {
        NameWords::BeginsWithChosen(name) if name == LETTER => {
            "name sticker on it that begins with the chosen letter".to_owned()
        }
        NameWords::BeginsWithChosen(_) => return None,
        NameWords::LettersAtLeast(count) => {
            format!(
                "name sticker on ~ with {} or more letters",
                count_word(*count)?
            )
        }
        NameWords::LettersAtMost(count) => {
            format!(
                "name sticker on ~ with {} or fewer letters",
                count_word(*count)?
            )
        }
    })
}

/// What `r6101_letters_in` read, with "the number of" before it.
pub(super) fn r6101_letters_text(amount: &Amount) -> Option<String> {
    let Amount::StickerLetters {
        on: Ref::This,
        letter,
    } = amount
    else {
        return None;
    };
    (letter.len() == 1 && letter.chars().all(|c| c.is_ascii_lowercase()))
        .then(|| format!("the number of {letter}'s in name stickers on ~"))
}

/// "choose a letter": the letter is kept under [`LETTER`].
pub(super) fn r6101_choose_letter(r: &mut Reader<'_>) -> Option<Vec<crate::effect::Effect>> {
    r.need("choose a letter")?;
    Some(vec![crate::effect::Effect::ChooseValue {
        who: Ref::You,
        kind: crate::effect::ValueKind::Letter,
        bind: LETTER.to_owned(),
    }])
}
