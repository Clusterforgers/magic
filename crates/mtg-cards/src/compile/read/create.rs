//! Tokens: "create two 1/1 white Soldier creature tokens that are tapped and attacking", "create a
//! Treasure token", "create a 1/1 white Spirit creature token with flying" and "create X Treasure tokens, where X
//! is the number of artifacts you control", read into `Effect::CreateToken` and written back the same way.
//! "Investigate" is "create a Clue token".

use mtg_core::card::{Subtype, Supertype};
use mtg_core::color::Color;

use super::amounts::{count_word, counted, counted_parts, number_word, stat_name};
use super::render;
use super::selectors::{
    Form, card_type, color, color_name, granted_text, keyword, phrase, phrase_text, subtype,
    type_name,
};
use super::statics;
use super::token_binds;
use super::tokens::Reader;
use super::triggers::article;
use super::where_x;
use crate::ability::Ability;
use crate::card::TokenSpec;
use crate::effect::{Effect, TokenRef};
use crate::modify::Modification;
use crate::refs::Ref;
use crate::value::Amount;

/// The predefined tokens, as a card names them and as their files are named.
const NAMED: [(&str, &str); 11] = [
    ("Treasure", "treasure"),
    ("Clue", "clue"),
    ("Food", "food"),
    ("Gold", "gold"),
    ("Blood", "blood"),
    ("Junk", "junk"),
    ("Powerstone", "powerstone"),
    ("Vibranium", "vibranium"),
    ("Map", "map"),
    ("Mutagen", "mutagen"),
    ("Gingerbrute", "gingerbrute"),
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
    // "an X/X token": its size is the amount the sentence gives X.
    let sized = r.attempt(|r| (r.word("x") && r.punct('/') && r.word("x")).then_some(()));
    let (power, toughness) = if sized.is_some() {
        (None, None)
    } else {
        let power = i32::try_from(r.number()?).ok()?;
        r.punct('/').then_some(())?;
        (Some(power), Some(i32::try_from(r.number()?).ok()?))
    };
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
    let spec = Box::new(TokenSpec {
        name,
        colors,
        supertypes: Vec::new(),
        types,
        subtypes,
        power,
        toughness,
        abilities: Vec::new(),
    });
    Some(match sized {
        // The announced X stands for the amount until the where clause or the cost names it.
        Some(()) if r.where_x || r.x_free => TokenRef::Sized {
            size: Amount::X,
            token: spec,
        },
        Some(()) => return None,
        None => TokenRef::Custom(spec),
    })
}

/// The description of a token that has one: a custom or a sized token.
fn spec_of(token: &mut TokenRef) -> Option<&mut TokenSpec> {
    match token {
        TokenRef::Custom(spec) | TokenRef::Sized { token: spec, .. } => Some(spec),
        _ => None,
    }
}

/// "with flying", "with flying and vigilance", "with flying, vigilance, and haste": the keywords a described token has.
fn keyword_abilities(r: &mut Reader<'_>) -> Option<Vec<Ability>> {
    r.word("with").then_some(())?;
    let mut found = vec![ability(r)?];
    while let Some(next) = r.attempt(|r| {
        let listed = r.punct(',');
        let joined = r.word("and");
        (listed || joined).then_some(())?;
        ability(r)
    }) {
        found.push(next);
    }
    Some(found)
}

/// A keyword, or an activated or triggered ability in quotes ("Sacrifice this token: Add {C}.").
fn ability(r: &mut Reader<'_>) -> Option<Ability> {
    if let Some(found) = r.attempt(keyword) {
        return Some(Ability::Keyword(found));
    }
    statics::quoted_ability(r).or_else(|| r.quoted(statics::ability))
}

/// How many tokens a card says: a number, X, or "that many".
enum Quantity {
    Number(u32),
    X,
    ThatMany,
    NumberOf,
}

/// "Create a Treasure token.", "Create two 1/1 white Soldier creature tokens that are tapped and
/// attacking.", "Create X Treasure tokens, where X is the number of artifacts you control.",
/// "Create a 1/1 white Spirit creature token with flying.", "Create two tapped Powerstone tokens.", "Create that
/// many Treasure tokens.", "Investigate."
pub fn tokens(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    if r.word("investigate") {
        return Some(vec![clue(Ref::You)]);
    }
    made_by(r, &Ref::You)
}

/// The Clue token a player creates when they investigate.
pub fn clue(who: Ref) -> Effect {
    effect(
        who,
        TokenRef::Named("clue".to_owned()),
        Amount::one(),
        false,
        false,
    )
}

/// The other wording of the tokens the controller of an object creates: "That creature's controller" for "Its controller".
pub fn other_wordings(texts: &[String]) -> Vec<String> {
    let controllers = texts
        .iter()
        .filter(|text| text.contains("Its controller creates "))
        .map(|text| text.replacen("Its controller", "That creature's controller", 1));
    // A list of two kinds may say the verb again: "create two Fish tokens and create two tapped Treasure tokens".
    let again = texts.iter().filter_map(|text| {
        let at = text.find("create ")?;
        let (head, tail) = text.split_at(at);
        let joint = [" token and ", " tokens and "]
            .into_iter()
            .find_map(|joint| tail.find(joint).map(|found| (joint, found)))?;
        let (joint, found) = joint;
        let (before, after) = tail.split_at(found + joint.len());
        // Only the first joint, and only the one that is followed by a kind of token.
        (!after.contains(" and ")).then(|| format!("{head}{before}create {after}"))
    });
    // A trigger may name who creates: "Whenever a land enchanted player controls enters, you create a Zombie token".
    let named = texts
        .iter()
        .filter(|text| text.starts_with("Whenever ") || text.starts_with("When "))
        .filter(|text| text.contains(", create "))
        .map(|text| text.replacen(", create ", ", you create ", 1));
    let have = texts.iter().filter_map(|text| quoted_in_a_sentence(text));
    controllers
        .chain(again)
        .chain(pronoun_wordings(texts))
        .chain(have)
        .chain(named)
        .collect()
}

/// The text with the one ability in quotes that a token has said in a sentence of its own: `create a 0/1 Spawn token
/// with "Sacrifice this token: Add {C}."` is `create a 0/1 Spawn token. It has "Sacrifice this token: Add {C}."`.
fn quoted_in_a_sentence(text: &str) -> Option<String> {
    let at = text.find("create ").or_else(|| text.find("Create "))?;
    let rest = &text[at..];
    // "with \"...\"" alone, or the quote joined to keywords: "with flying and \"...\"".
    let (cut, joint) = rest
        .find(" with \"")
        .map(|with| (with, " with "))
        .or_else(|| rest.rfind(" and \"").map(|and| (and, " and ")))?;
    let (head, quoted) = (&text[..at + cut], &text[at + cut + joint.len()..]);
    // Only a quote that closes the text: nothing joined to it.
    let closed = quoted.strip_suffix('.').unwrap_or(quoted);
    (closed.matches('"').count() == 2 && closed.ends_with('"')).then_some(())?;
    let nouns = head[at..].split(" with ").next()?;
    let verb = if nouns.ends_with(" tokens") {
        "They have"
    } else if nouns.ends_with(" token") {
        "It has"
    } else {
        return None;
    };
    Some(format!("{head}. {verb} {quoted}"))
}

/// A count of the card's own power or counters, which a card may word with "it": "tokens equal to its power", "where X
/// is the number of +1/+1 counters on it".
pub fn pronoun_wordings(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter(|text| text.contains("create "))
        .filter_map(|text| {
            let said = text
                .replacen(" tokens equal to ~'s ", " tokens equal to its ", 1)
                .replacen(" counters on ~", " counters on it", 1);
            (said != *text).then_some(said)
        })
        .collect()
}

/// The tokens a player creates: "create ..." for you, "creates ..." after another player ("target opponent creates
/// two 1/1 white Soldier creature tokens", "its controller creates a 3/3 green Beast creature token").
pub fn made_by(r: &mut Reader<'_>, who: &Ref) -> Option<Vec<Effect>> {
    r.need(if *who == Ref::You {
        "create"
    } else {
        "creates"
    })?;
    let mut effects = vec![group(r, who)?];
    // "create a 1/1 Snake token, a 2/2 Wolf token, and a 3/3 Elephant token": one group after another.
    while let Some(next) = r.attempt(|r| {
        let listed = r.punct(',');
        let joined = r.word("and");
        (listed || joined).then_some(())?;
        // "create two Fish tokens and create two tapped Treasure tokens": the verb may be said again.
        if joined {
            r.word(if *who == Ref::You {
                "create"
            } else {
                "creates"
            });
        }
        group(r, who)
    }) {
        effects.push(next);
    }
    if let Some(abilities) = r.attempt(|r| they_have(r, &effects)) {
        spec_of(last_token(&mut effects)?)?
            .abilities
            .extend(abilities);
    }
    Some(effects)
}

/// The description of the token of the last effect.
fn last_token(effects: &mut [Effect]) -> Option<&mut TokenRef> {
    match effects.last_mut()? {
        Effect::CreateToken { token, .. } => Some(token),
        _ => None,
    }
}

/// ". They have \"Sacrifice this token: Add {C}.\"", ". It has \"...\"": the abilities the last kind of token has, said in a sentence of its own.
fn they_have(r: &mut Reader<'_>, made: &[Effect]) -> Option<Vec<Ability>> {
    r.punct('.').then_some(())?;
    let one = matches!(
        made.last()?,
        Effect::CreateToken {
            count: Amount::Fixed(1),
            ..
        }
    );
    r.words(if one { "it has" } else { "they have" })
        .then_some(())?;
    let mut found = vec![ability(r)?];
    while r.word("and") {
        found.push(ability(r)?);
    }
    Some(found)
}

/// "Draw two cards and create a Treasure token.", "Create three 1/1 green Elf Warrior creature tokens, then mill three
/// cards.": the steps of "you" joined by "and" or ", then", at least one of them a token created.
pub fn chained(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let mut effects = step(r)?;
    let mut steps = 1;
    while let Some(next) = r.attempt(|r| {
        if r.punct(',') {
            r.need("then")?;
        } else {
            r.need("and")?;
        }
        // "create a 1/1 white Soldier creature token, then attach this Equipment to it"
        token_binds::joined(r, &effects).or_else(|| step(r))
    }) {
        effects.extend(next);
        steps += 1;
    }
    token_binds::bind(&mut effects);
    let creates = effects
        .iter()
        .any(|effect| matches!(effect, Effect::CreateToken { .. }));
    (steps > 1 && creates).then_some(effects)
}

/// One step of "you": a token created, or a draw, mill, scry and the like.
fn step(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    if r.peek_word() == Some("create") {
        return made_by(r, &Ref::You);
    }
    super::effects::imperative(r).map(|effect| vec![effect])
}

/// One kind of token and how many: "a Treasure token", "two tapped 2/2 black Zombie creature tokens", "X tokens".
fn group(r: &mut Reader<'_>, who: &Ref) -> Option<Effect> {
    // "Tiny, a legendary 2/2 green Dog Detective creature token": one token, named.
    let legend = r.attempt(legendary);
    let quantity = if legend.is_some() {
        Quantity::Number(1)
    } else if r.ability && r.triggered && r.words("that many") {
        Quantity::ThatMany
    } else if r.words("a number of") {
        Quantity::NumberOf
    } else if r.word("x") {
        Quantity::X
    } else {
        Quantity::Number(number_word(r.take_word()?)?)
    };
    // "two tapped Powerstone tokens": tapped without attacking, before the token.
    let tapped_alone = r.word("tapped");
    let mut token = match legend {
        Some(found) => found,
        None => token(r)?,
    };
    let single = matches!(quantity, Quantity::Number(1));
    r.word(if single { "token" } else { "tokens" })
        .then_some(())?;
    if let Some(abilities) = r.attempt(keyword_abilities) {
        spec_of(&mut token)?.abilities = abilities;
    }
    if let Some(name) = r.attempt(token_name) {
        spec_of(&mut token)?.name = name;
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
        Quantity::NumberOf => where_x::equal_to(r)?,
        // The where clause of the sentence gives X its amount.
        Quantity::X if r.where_x => Amount::X,
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
    Some(effect(who.clone(), token, count, tapped, attacking))
}

/// "named Kobolds of Kher Keep": the name a token has when it is not its subtypes.
fn token_name(r: &mut Reader<'_>) -> Option<String> {
    r.need("named")?;
    proper_name(r)
}

/// A proper name: capitalized words, with "of" and "the" between them.
fn proper_name(r: &mut Reader<'_>) -> Option<String> {
    let mut words = vec![r.name()?.to_owned()];
    while let Some(word) = r.attempt(|r| {
        let joined = r.take_word().filter(|word| matches!(*word, "of" | "the"));
        match joined {
            Some(word) => Some(format!("{word} {}", r.name()?)),
            None => r.name().map(str::to_owned),
        }
    }) {
        words.push(word);
    }
    Some(words.join(" "))
}

/// "Tiny, a legendary 2/2 green Dog Detective creature": the name, then the description of a legendary token.
fn legendary(r: &mut Reader<'_>) -> Option<TokenRef> {
    let name = proper_name(r)?;
    r.punct(',').then_some(())?;
    r.need("a legendary")?;
    let TokenRef::Custom(mut spec) = token(r)? else {
        return None;
    };
    spec.name = name;
    spec.supertypes = vec![Supertype::Legendary];
    Some(TokenRef::Custom(spec))
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

/// A token as a card words it, without its count and noun: the description, the name when it is not its subtypes, and
/// whether it is legendary (the name then comes first: "Tiny, a legendary 2/2 green Dog Detective creature token").
struct Described {
    what: String,
    name: Option<String>,
    legendary: bool,
}

fn token_text(token: &TokenRef) -> Option<Described> {
    match token {
        TokenRef::Named(id) => {
            NAMED
                .iter()
                .find(|(_, file)| file == id)
                .map(|(shown, _)| Described {
                    what: (*shown).to_owned(),
                    name: None,
                    legendary: false,
                })
        }
        TokenRef::Custom(spec) | TokenRef::Sized { token: spec, .. } => {
            let size = match token {
                TokenRef::Sized { size, .. } if *size == Amount::X => "X/X".to_owned(),
                TokenRef::Sized { .. } => return None,
                _ => format!("{}/{}", spec.power?, spec.toughness?),
            };
            let (power, toughness) = (spec.power.unwrap_or(0), spec.toughness.unwrap_or(0));
            let subtypes: Vec<&str> = spec.subtypes.iter().map(|found| found.0.as_str()).collect();
            let types: Vec<&str> = spec.types.iter().map(|found| type_name(*found)).collect();
            let legendary = spec.supertypes == [Supertype::Legendary];
            let named = spec.name != subtypes.join(" ");
            let plain = (spec.supertypes.is_empty() || legendary)
                && legendary <= named
                && !subtypes.is_empty()
                && !types.is_empty()
                && power >= 0
                && toughness >= 0;
            plain.then(|| Described {
                what: format!(
                    "{size} {} {} {}",
                    colors_text(&spec.colors),
                    subtypes.join(" "),
                    types.join(" ")
                ),
                name: named.then(|| spec.name.clone()),
                legendary,
            })
        }
        TokenRef::CopyOf { .. } => None,
    }
}

/// The abilities of a described token as a card words them, with the space before: " with flying", " with flying and
/// vigilance", " with flying, vigilance, and haste", " with \"Sacrifice this token: Add {C}.\"". `None` for an ability
/// that is neither a keyword nor an activated or triggered ability.
fn keywords_text(token: &TokenRef) -> Option<String> {
    let (TokenRef::Custom(spec) | TokenRef::Sized { token: spec, .. }) = token else {
        return Some(String::new());
    };
    let mut granted: Vec<Modification> = Vec::new();
    let mut quoted: Vec<String> = Vec::new();
    for ability in &spec.abilities {
        match ability {
            Ability::Keyword(found) => granted.push(Modification::AddKeyword(found.clone())),
            Ability::Activated { .. } | Ability::Triggered { .. } | Ability::Static { .. } => {
                quoted.push(format!("\"{}\"", render::ability(ability)?));
            }
            _ => return None,
        }
    }
    let mut parts = Vec::new();
    if !granted.is_empty() {
        parts.push(granted_text(&granted)?);
    }
    parts.extend(quoted);
    if parts.is_empty() {
        return Some(String::new());
    }
    Some(format!(" with {}", parts.join(" and ")))
}

/// The same for tokens a later sentence names (`bind`), which is written as the sentence that makes them.
pub fn text_bound(effect: &Effect, per: bool) -> Option<String> {
    let Effect::CreateToken { bind: Some(_), .. } = effect else {
        return None;
    };
    let mut unbound = effect.clone();
    if let Effect::CreateToken { bind, .. } = &mut unbound {
        *bind = None;
    }
    text(&unbound, per)
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
    let Described {
        mut what,
        name,
        legendary,
    } = token_text(token)?;
    let with = keywords_text(token)?;
    let verb = if *who == Ref::You {
        "create"
    } else {
        "creates"
    };
    let entering = |single: bool| match (*tapped, *attacking, single) {
        (_, false, _) => "",
        (true, true, true) => " that's tapped and attacking",
        (true, true, false) => " that are tapped and attacking",
        (false, true, true) => " that's attacking",
        (false, true, false) => " that are attacking",
    };
    if legendary {
        let name = name?;
        return (*count == Amount::one() && !*tapped).then(|| {
            format!(
                "{verb} {name}, a legendary {what} token{with}{}",
                entering(true)
            )
        });
    }
    // A token that enters tapped without attacking: "a tapped Treasure token".
    if *tapped && !*attacking {
        what = format!("tapped {what}");
    }
    let (number, single, tail) = quantity_text(count, &what, per)?;
    let noun = if single { "token" } else { "tokens" };
    let named = name
        .map(|name| format!(" named {name}"))
        .unwrap_or_default();
    Some(format!(
        "{verb} {number} {what} {noun}{with}{named}{}{tail}",
        entering(single)
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
        Amount::Times { by: 2, of } if **of == Amount::X => {
            ("twice X".to_owned(), false, String::new())
        }
        Amount::Triggering => ("that many".to_owned(), false, String::new()),
        // "a number of 1/1 Rabbit creature tokens equal to ~'s power"
        Amount::Stat { of, stat } if matches!(of, Ref::This | Ref::Triggering) => {
            let owner = if *of == Ref::This {
                "~'s"
            } else {
                "that creature's"
            };
            (
                "a number of".to_owned(),
                false,
                format!(" equal to {owner} {}", stat_name(*stat)?),
            )
        }
        // "a number of 1/1 white Soldier creature tokens equal to your devotion to white"
        Amount::Devotion(colors) if matches!(colors.len(), 1 | 2) => {
            let names: Vec<&str> = colors.iter().map(|color| color_name(*color)).collect();
            (
                "a number of".to_owned(),
                false,
                format!(" equal to your devotion to {}", names.join(" and ")),
            )
        }
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

/// "4/4 white Angel creature tokens with flying and vigilance": a kind of token in the plural, with its keywords.
pub(super) fn w9c_plural_token(r: &mut Reader<'_>) -> Option<TokenRef> {
    let mut found = token(r)?;
    r.word("tokens").then_some(())?;
    if let Some(abilities) = r.attempt(keyword_abilities) {
        spec_of(&mut found)?.abilities = abilities;
    }
    Some(found)
}
