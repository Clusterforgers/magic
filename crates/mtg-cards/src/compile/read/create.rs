//! Tokens: "create two 1/1 white Soldier creature tokens that are tapped and attacking", "create a
//! Treasure token" and "create X Treasure tokens, where X is the number of artifacts you control",
//! read into `Effect::CreateToken` and written back the same way. "Investigate" is "create a Clue token".

use mtg_core::card::Subtype;
use mtg_core::color::Color;

use super::amounts::{count_word, number_word};
use super::selectors::{
    Form, card_type, color, color_name, phrase, phrase_text, subtype, type_name,
};
use super::tokens::Reader;
use super::triggers::article;
use crate::card::TokenSpec;
use crate::effect::{Effect, TokenRef};
use crate::refs::Ref;
use crate::value::Amount;

/// The predefined tokens, as a card names them and as their files are named.
const NAMED: [(&str, &str); 3] = [("Treasure", "treasure"), ("Clue", "clue"), ("Food", "food")];

fn effect(token: TokenRef, count: Amount, tapped: bool, attacking: bool) -> Effect {
    Effect::CreateToken {
        who: Ref::You,
        token,
        count,
        tapped,
        attacking,
        bind: None,
    }
}

/// A predefined token ("Treasure token") or a described one ("1/1 white Soldier creature token"),
/// without the noun "token" that ends it.
fn token(r: &mut Reader<'_>) -> Option<TokenRef> {
    if let Some(id) = r.attempt(|r| {
        let name = r.name()?;
        NAMED
            .iter()
            .find(|(shown, _)| *shown == name)
            .map(|(_, id)| (*id).to_owned())
    }) {
        return Some(TokenRef::Named(id));
    }
    let power = i32::try_from(r.number()?).ok()?;
    r.punct('/').then_some(())?;
    let toughness = i32::try_from(r.number()?).ok()?;
    let mut colors = Vec::new();
    if !r.word("colorless") {
        colors.push(r.take_word().and_then(color)?);
        while r.word("and") {
            colors.push(r.take_word().and_then(color)?);
        }
    }
    let mut subtypes = Vec::new();
    while let Some(name) = subtype(r) {
        subtypes.push(Subtype(name));
    }
    let mut types = Vec::new();
    while let Some(found) = r.peek_word().and_then(card_type) {
        r.skip();
        types.push(found);
    }
    if subtypes.is_empty() || types.is_empty() {
        return None;
    }
    let name = subtypes
        .iter()
        .map(|subtype| subtype.0.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    Some(TokenRef::Custom(Box::new(TokenSpec {
        name,
        colors,
        supertypes: Vec::new(),
        types,
        subtypes,
        power: Some(power),
        toughness: Some(toughness),
        abilities: Vec::new(),
    })))
}

/// "Create a Treasure token.", "Create two 1/1 white Soldier creature tokens that are tapped and
/// attacking.", "Create X Treasure tokens, where X is the number of artifacts you control.",
/// "Investigate."
pub fn tokens(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    if r.word("investigate") {
        return Some(vec![effect(
            TokenRef::Named("clue".to_owned()),
            Amount::one(),
            false,
            false,
        )]);
    }
    r.need("create")?;
    let variable = r.word("x");
    let number = if variable {
        None
    } else {
        Some(number_word(r.take_word()?)?)
    };
    let token = token(r)?;
    let single = number == Some(1);
    r.word(if single { "token" } else { "tokens" })
        .then_some(())?;
    let on_the_battlefield = if single {
        "that's tapped and attacking"
    } else {
        "that are tapped and attacking"
    };
    let entering = r.words(on_the_battlefield);
    let count = if let Some(number) = number {
        Amount::Fixed(i32::try_from(number).ok()?)
    } else {
        r.punct(',').then_some(())?;
        r.need("where x is the number of")?;
        Amount::Count(phrase(r, Form::All)?)
    };
    Some(vec![effect(token, count, entering, entering)])
}

/// The colors of a token as a card words them: "white and blue", "colorless".
fn colors_text(colors: &[Color]) -> String {
    if colors.is_empty() {
        return "colorless".to_owned();
    }
    colors
        .iter()
        .map(|found| color_name(*found))
        .collect::<Vec<_>>()
        .join(" and ")
}

/// The token as a card words it, without its count and noun.
fn token_text(token: &TokenRef) -> Option<String> {
    match token {
        TokenRef::Named(id) => NAMED
            .iter()
            .find(|(_, file)| file == id)
            .map(|(shown, _)| (*shown).to_owned()),
        TokenRef::Custom(spec) => {
            let (Some(power), Some(toughness)) = (spec.power, spec.toughness) else {
                return None;
            };
            let subtypes: Vec<&str> = spec.subtypes.iter().map(|found| found.0.as_str()).collect();
            let types: Vec<&str> = spec.types.iter().map(|found| type_name(*found)).collect();
            let plain = spec.supertypes.is_empty()
                && spec.abilities.is_empty()
                && !subtypes.is_empty()
                && !types.is_empty()
                && spec.name == subtypes.join(" ")
                && power >= 0
                && toughness >= 0;
            plain.then(|| {
                format!(
                    "{power}/{toughness} {} {} {}",
                    colors_text(&spec.colors),
                    subtypes.join(" "),
                    types.join(" ")
                )
            })
        }
        TokenRef::CopyOf { .. } => None,
    }
}

/// An effect that creates tokens, as a sentence without its first capital and period.
pub fn text(effect: &Effect) -> Option<String> {
    let Effect::CreateToken {
        who: Ref::You,
        token,
        count,
        tapped,
        attacking,
        bind: None,
    } = effect
    else {
        return None;
    };
    if tapped != attacking {
        return None;
    }
    let what = token_text(token)?;
    let (number, single, tail) = match count {
        Amount::Fixed(1) => (article(&what).to_owned(), true, String::new()),
        Amount::Fixed(number) => (
            count_word(u32::try_from(*number).ok()?)?.to_owned(),
            false,
            String::new(),
        ),
        Amount::Count(selector) => (
            "X".to_owned(),
            false,
            format!(
                ", where X is the number of {}",
                phrase_text(selector, Form::All)?
            ),
        ),
        _ => return None,
    };
    let noun = if single { "token" } else { "tokens" };
    let entering = match (*tapped, single) {
        (false, _) => "",
        (true, true) => " that's tapped and attacking",
        (true, false) => " that are tapped and attacking",
    };
    Some(format!("create {number} {what} {noun}{entering}{tail}"))
}
