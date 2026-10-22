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
const NAMED: [(&str, &str); 4] = [
    ("Treasure", "treasure"),
    ("Clue", "clue"),
    ("Food", "food"),
    ("Gold", "gold"),
];

fn effect(who: Ref, token: TokenRef, count: Amount, tapped: bool, attacking: bool) -> Effect {
    Effect::CreateToken {
        who,
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

/// How many tokens a card says: a number, X, or "that many".
enum Quantity {
    Number(u32),
    X,
    ThatMany,
}

/// "Create a Treasure token.", "Create two 1/1 white Soldier creature tokens that are tapped and
/// attacking.", "Create X Treasure tokens, where X is the number of artifacts you control.",
/// "Create a 1/1 white Spirit creature token with flying.", "Create two tapped Powerstone tokens.", "Create that
/// many Treasure tokens.", "Investigate."
pub fn tokens(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    if r.word("investigate") {
        return Some(vec![effect(
            Ref::You,
            TokenRef::Named("clue".to_owned()),
            Amount::one(),
            false,
            false,
        )]);
    }
    made_by(r, Ref::You)
}

/// The tokens a player creates: "create ..." for you, "creates ..." after another player ("target opponent creates
/// two 1/1 white Soldier creature tokens", "its controller creates a 3/3 green Beast creature token").
pub fn made_by(r: &mut Reader<'_>, who: Ref) -> Option<Vec<Effect>> {
    r.need(if who == Ref::You { "create" } else { "creates" })?;
    let quantity = if r.words("that many") {
        Quantity::ThatMany
    } else if r.word("x") {
        Quantity::X
    } else {
        Quantity::Number(number_word(r.take_word()?)?)
    };
    // "two tapped Powerstone tokens": tapped without attacking, before the token.
    let tapped_alone = r.word("tapped");
    let mut token = token(r)?;
    let single = matches!(quantity, Quantity::Number(1));
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
    let (tapped, attacking) = if tapped_alone {
        (true, false)
    } else if r.words(tapped_and_attacking) {
        (true, true)
    } else {
        (false, r.words(attacking_only))
    };
    let count = match quantity {
        // "... token for each Elf you control": the count of the Elves, times the number when it is not one.
        Quantity::Number(number) => counted(r, Amount::Fixed(i32::try_from(number).ok()?))?,
        Quantity::ThatMany => Amount::Triggering,
        Quantity::X if r.punct(',') => {
            r.need("where x is the number of")?;
            Amount::Count(phrase(r, Form::All)?)
        }
        // The X of the spell's or the ability's own cost.
        Quantity::X => {
            r.x_free.then_some(())?;
            Amount::X
        }
    };
    Some(vec![effect(who, token, count, tapped, attacking)])
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
    let Effect::CreateToken {
        who,
        token,
        count,
        tapped,
        attacking,
        bind: None,
    } = effect
    else {
        return None;
    };
    let mut what = token_text(token)?;
    // A token that enters tapped without attacking: "a tapped Treasure token".
    if *tapped && !*attacking {
        what = format!("tapped {what}");
    }
    let with = keywords_text(token)?;
    let (number, single, tail) = quantity_text(count, &what, per)?;
    let verb = if *who == Ref::You {
        "create"
    } else {
        "creates"
    };
    let noun = if single { "token" } else { "tokens" };
    let entering = match (*tapped, *attacking, single) {
        (_, false, _) => "",
        (true, true, true) => " that's tapped and attacking",
        (true, true, false) => " that are tapped and attacking",
        (false, true, true) => " that's attacking",
        (false, true, false) => " that are attacking",
    };
    Some(format!(
        "{verb} {number} {what} {noun}{with}{entering}{tail}"
    ))
}

/// How many tokens as a card words it, whether that is one token, and what follows the noun: "a", "two", "X" (with
/// ", where X is the number of ..." for a count), "that many".
fn quantity_text(count: &Amount, what: &str, per: bool) -> Option<(String, bool, String)> {
    Some(match count {
        Amount::Fixed(1) => (article(what).to_owned(), true, String::new()),
        Amount::Fixed(number) => (
            count_word(u32::try_from(*number).ok()?)?.to_owned(),
            false,
            String::new(),
        ),
        Amount::X => ("X".to_owned(), false, String::new()),
        Amount::Triggering => ("that many".to_owned(), false, String::new()),
        // The same count in the other wording: "a Goblin token for each Goblin you control".
        counted if per => {
            let (each, tail) = counted_parts(counted)?;
            let (number, single) = match each {
                Amount::Fixed(1) => (article(what).to_owned(), true),
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
    })
}
