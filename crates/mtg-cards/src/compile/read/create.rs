//! Tokens: "create two 1/1 white Soldier creature tokens that are tapped and attacking", "create a
//! Treasure token", "create a 1/1 white Spirit creature token with flying" and "create X Treasure tokens, where X
//! is the number of artifacts you control", read into `Effect::CreateToken` and written back the same way.
//! "Investigate" is "create a Clue token".

use mtg_core::card::Subtype;
use mtg_core::color::Color;

use super::amounts::{count_word, counted, counted_parts, number_word};
use super::selectors::{
    Form, card_type, color, color_name, keyword, keyword_text, phrase, phrase_text, subtype,
    type_name,
};
use super::tokens::Reader;
use super::triggers::article;
use crate::ability::Ability;
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

/// "with flying", "with flying and vigilance", "with flying, vigilance, and haste": the keywords a described token has.
fn keyword_abilities(r: &mut Reader<'_>) -> Option<Vec<Ability>> {
    r.word("with").then_some(())?;
    let mut found = vec![Ability::Keyword(keyword(r)?)];
    while let Some(next) = r.attempt(|r| {
        let listed = r.punct(',');
        let joined = r.word("and");
        (listed || joined).then_some(())?;
        keyword(r)
    }) {
        found.push(Ability::Keyword(next));
    }
    Some(found)
}

/// "Create a Treasure token.", "Create two 1/1 white Soldier creature tokens that are tapped and
/// attacking.", "Create X Treasure tokens, where X is the number of artifacts you control.",
/// "Create a 1/1 white Spirit creature token with flying.", "Investigate."
pub fn tokens(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    tokens_of(r, false)
}

/// The tokens another player creates, as the controller of a permanent the sentence before named: "Its controller creates
/// a 3/3 green Beast creature token.", "Its controller investigates."
pub fn tokens_by(r: &mut Reader<'_>, who: &Ref) -> Option<Vec<Effect>> {
    let mut effects = tokens_of(r, true)?;
    for created in &mut effects {
        if let Effect::CreateToken { who: owner, .. } = created {
            owner.clone_from(who);
        }
    }
    Some(effects)
}

/// The sentence of `tokens`, in the person of its subject: "create" for you, "creates" for another player.
fn tokens_of(r: &mut Reader<'_>, third: bool) -> Option<Vec<Effect>> {
    if r.word(if third { "investigates" } else { "investigate" }) {
        return Some(vec![effect(
            TokenRef::Named("clue".to_owned()),
            Amount::one(),
            false,
            false,
        )]);
    }
    r.need(if third { "creates" } else { "create" })?;
    let variable = r.word("x");
    let number = if variable {
        None
    } else {
        Some(number_word(r.take_word()?)?)
    };
    let mut token = token(r)?;
    let single = number == Some(1);
    r.word(if single { "token" } else { "tokens" })
        .then_some(())?;
    if let Some(abilities) = r.attempt(keyword_abilities) {
        let TokenRef::Custom(spec) = &mut token else {
            return None;
        };
        spec.abilities = abilities;
    }
    let (tapped_and_attacking, attacking_only) = if single {
        ("that's tapped and attacking", "that's attacking")
    } else {
        ("that are tapped and attacking", "that are attacking")
    };
    let (tapped, attacking) = if r.words(tapped_and_attacking) {
        (true, true)
    } else {
        (false, r.words(attacking_only))
    };
    let count = if let Some(number) = number {
        // "... token for each Elf you control": the count of the Elves, times the number when it is not one.
        counted(r, Amount::Fixed(i32::try_from(number).ok()?))?
    } else {
        r.punct(',').then_some(())?;
        r.need("where x is the number of")?;
        Amount::Count(phrase(r, Form::All)?)
    };
    Some(vec![effect(token, count, tapped, attacking)])
}

/// The colors of a token as a card words them: "white and blue", "colorless".
pub(super) fn colors_text(colors: &[Color]) -> String {
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

/// The keywords of a described token as a card words them, with the space before: " with flying", " with flying and
/// vigilance", " with flying, vigilance, and haste". `None` for an ability that is not a plain keyword.
fn keywords_text(token: &TokenRef) -> Option<String> {
    let TokenRef::Custom(spec) = token else {
        return Some(String::new());
    };
    let names: Vec<String> = spec
        .abilities
        .iter()
        .map(|ability| match ability {
            Ability::Keyword(found) => keyword_text(found),
            _ => None,
        })
        .collect::<Option<_>>()?;
    Some(match names.as_slice() {
        [] => String::new(),
        [only] => format!(" with {only}"),
        [first, last] => format!(" with {first} and {last}"),
        [rest @ .., last] => format!(" with {}, and {last}", rest.join(", ")),
    })
}

/// An effect that creates tokens, as a sentence without its first capital and period. A count of things is worded
/// "X tokens, where X is the number of ..." or, with `per`, "a token for each ...".
pub fn text(effect: &Effect, per: bool) -> Option<String> {
    let Effect::CreateToken { who: Ref::You, .. } = effect else {
        return None;
    };
    created_text(effect, per)
}

/// The sentence of `text` for another player: "creates a 3/3 green Beast creature token".
pub fn text_by(effect: &Effect, per: bool) -> Option<String> {
    let Effect::CreateToken { who, .. } = effect else {
        return None;
    };
    let text = created_text(effect, per)?;
    if *who == Ref::You {
        return Some(text);
    }
    text.strip_prefix("create")
        .map(|rest| format!("creates{rest}"))
}

/// The other wording of the tokens the controller of an object creates: "That creature's controller" for "Its controller".
pub fn other_wordings(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter(|text| text.contains("Its controller creates "))
        .map(|text| text.replacen("Its controller", "That creature's controller", 1))
        .collect()
}

/// The wording of the tokens an effect creates, whoever creates them, in the person of "create".
fn created_text(effect: &Effect, per: bool) -> Option<String> {
    let Effect::CreateToken {
        token,
        count,
        tapped,
        attacking,
        bind: None,
        ..
    } = effect
    else {
        return None;
    };
    // A token that enters tapped and not attacking has no wording of its own here.
    if *tapped && !*attacking {
        return None;
    }
    let what = token_text(token)?;
    let with = keywords_text(token)?;
    let (number, single, tail) = match count {
        Amount::Fixed(1) => (article(&what).to_owned(), true, String::new()),
        Amount::Fixed(number) => (
            count_word(u32::try_from(*number).ok()?)?.to_owned(),
            false,
            String::new(),
        ),
        // The same count in the other wording: "a Goblin token for each Goblin you control".
        counted if per => {
            let (each, tail) = counted_parts(counted)?;
            let (number, single) = match each {
                Amount::Fixed(1) => (article(&what).to_owned(), true),
                Amount::Fixed(number) => {
                    (count_word(u32::try_from(number).ok()?)?.to_owned(), false)
                }
                _ => return None,
            };
            (number, single, format!(" {tail}"))
        }
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
    let entering = match (*tapped, *attacking, single) {
        (false, false, _) => "",
        (true, _, true) => " that's tapped and attacking",
        (true, _, false) => " that are tapped and attacking",
        (false, true, true) => " that's attacking",
        (false, true, false) => " that are attacking",
    };
    Some(format!(
        "create {number} {what} {noun}{with}{entering}{tail}"
    ))
}
