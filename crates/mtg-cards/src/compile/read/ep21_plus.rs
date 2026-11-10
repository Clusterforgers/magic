//! Token replacements that keep the tokens and add more (`ReplaceBy::Plus`): "If one or more tokens would be created
//! under your control, those tokens plus that many 1/1 green Squirrel creature tokens are created instead." and "If you
//! would create one or more Treasure tokens, instead create those tokens plus an additional Treasure token." Read and
//! written back.

use super::tokens::Reader;
use crate::ability::Ability;
use crate::effect::{Effect, TokenRef};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};
use crate::value::Amount;
use mtg_core::card::{CardType, Subtype};

/// The kinds of token named by a card type.
const KINDS: [(CardType, &str); 2] = [
    (CardType::Creature, "creature"),
    (CardType::Artifact, "artifact"),
];

/// The kinds of token named by their subtype.
const SUBTYPES: [&str; 4] = ["Treasure", "Food", "Clue", "Map"];

/// The kind of token words name: none for "tokens", a card type, or a subtype.
fn kind(r: &mut Reader<'_>) -> Selector {
    if let Some(found) = KINDS
        .iter()
        .find_map(|(kind, words)| r.word(words).then_some(*kind))
    {
        return Selector(vec![Pred::Type(found)]);
    }
    if let Some(found) = r.attempt(|r| {
        let name = r.name()?;
        SUBTYPES.contains(&name).then_some(name)
    }) {
        return Selector(vec![Pred::Subtype(Subtype(found.to_owned()))]);
    }
    Selector::default()
}

/// The words of a kind, with a space after them; empty for any token.
fn kind_text(what: &Selector) -> Option<String> {
    Some(match what.0.as_slice() {
        [] => String::new(),
        [Pred::Type(found)] => {
            let (_, words) = KINDS.iter().find(|(kind, _)| kind == found)?;
            format!("{words} ")
        }
        [Pred::Subtype(Subtype(name))] if SUBTYPES.contains(&name.as_str()) => format!("{name} "),
        _ => return None,
    })
}

/// "that many 1/1 green Squirrel creature tokens", "an additional Food token", "a 1/1 white Soldier creature token".
fn more(r: &mut Reader<'_>) -> Option<Effect> {
    let (token, count) = if r.words("that many") {
        (super::create::ep21_token(r, true)?, Amount::Triggering)
    } else {
        (r.word("a") || r.word("an")).then_some(())?;
        r.word("additional");
        (super::create::ep21_token(r, false)?, Amount::one())
    };
    Some(create(token, count))
}

fn create(token: TokenRef, count: Amount) -> Effect {
    Effect::CreateToken {
        who: Ref::You,
        token,
        count,
        tapped: false,
        attacking: false,
        against: None,
        blocking: None,
        bind: None,
    }
}

/// The predefined tokens "one of each" names, by their subtype.
const EACH: [&str; 4] = ["Clue", "Food", "Treasure", "Blood"];

/// "If you would create a Clue, Food, or Treasure token, instead create one of each.": each token of those kinds
/// becomes one of every kind listed.
fn one_of_each(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("if you would create a")?;
    let mut names = Vec::new();
    loop {
        let name = r.name()?;
        EACH.contains(&name).then_some(())?;
        names.push(name.to_owned());
        if r.word("token") {
            break;
        }
        r.punct(',');
        r.word("or");
    }
    (names.len() > 1).then_some(())?;
    r.punct(',').then_some(())?;
    r.need("instead create one of each")?;
    let what = Selector(vec![Pred::AnyOf(
        names
            .iter()
            .map(|name| Pred::Subtype(Subtype(name.clone())))
            .collect(),
    )]);
    let effects = names
        .iter()
        .map(|name| create(TokenRef::Named(name.to_lowercase()), Amount::Triggering))
        .collect();
    Some(Ability::Replacement {
        when: ReplaceWhen::TokenCreation {
            controller: Ref::You,
            what,
        },
        by: ReplaceBy::Instead(effects),
        condition: None,
        from: ZoneKind::Battlefield,
    })
}

/// "One of each", written back; `None` for any other replacement.
pub(super) fn ep21_each_sentence(when: &ReplaceWhen, by: &ReplaceBy) -> Option<String> {
    let (
        ReplaceWhen::TokenCreation {
            controller: Ref::You,
            what,
        },
        ReplaceBy::Instead(effects),
    ) = (when, by)
    else {
        return None;
    };
    let [Pred::AnyOf(kinds)] = what.0.as_slice() else {
        return None;
    };
    let names: Vec<&str> = kinds
        .iter()
        .map(|kind| match kind {
            Pred::Subtype(Subtype(name)) if EACH.contains(&name.as_str()) => Some(name.as_str()),
            _ => None,
        })
        .collect::<Option<_>>()?;
    (names.len() > 1 && names.len() == effects.len()).then_some(())?;
    for (name, effect) in names.iter().zip(effects) {
        let Effect::CreateToken {
            who: Ref::You,
            token: TokenRef::Named(made),
            count: Amount::Triggering,
            tapped: false,
            attacking: false,
            against: None,
            blocking: None,
            bind: None,
        } = effect
        else {
            return None;
        };
        (*made == name.to_lowercase()).then_some(())?;
    }
    let listed = match names.as_slice() {
        [first, second] => format!("{first} or {second}"),
        [rest @ .., last] => format!("{}, or {last}", rest.join(", ")),
        [] => return None,
    };
    Some(format!(
        "if you would create a {listed} token, instead create one of each"
    ))
}

/// Either wording of a replacement that adds tokens, or "one of each".
pub(super) fn ep21_plus(r: &mut Reader<'_>) -> Option<Ability> {
    if let Some(found) = r.attempt(one_of_each) {
        return Some(found);
    }
    r.word("if").then_some(())?;
    let (what, effect) = if r.words("you would create one or more") {
        let what = kind(r);
        r.word("tokens").then_some(())?;
        r.punct(',').then_some(())?;
        r.need("instead create those tokens plus")?;
        (what, more(r)?)
    } else {
        r.need("one or more")?;
        let what = kind(r);
        r.need("tokens would be created under your control")?;
        r.punct(',').then_some(())?;
        r.need("those tokens plus")?;
        let effect = more(r)?;
        r.need("are created instead")?;
        (what, effect)
    };
    Some(Ability::Replacement {
        when: ReplaceWhen::TokenCreation {
            controller: Ref::You,
            what,
        },
        by: ReplaceBy::Plus(vec![effect]),
        condition: None,
        from: ZoneKind::Battlefield,
    })
}

/// What `ep21_plus` reads, written back in both wordings; empty for any other ability.
pub(super) fn ep21_sentences(ability: &Ability) -> Vec<String> {
    if let Ability::Replacement {
        when,
        by,
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
        && let Some(each) = ep21_each_sentence(when, by)
    {
        return vec![each];
    }
    let Ability::Replacement {
        when:
            ReplaceWhen::TokenCreation {
                controller: Ref::You,
                what,
            },
        by: ReplaceBy::Plus(effects),
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return Vec::new();
    };
    let (Some(kind), [made @ Effect::CreateToken { who: Ref::You, .. }]) =
        (kind_text(what), effects.as_slice())
    else {
        return Vec::new();
    };
    let Some(tokens) = super::create::text(made, false)
        .and_then(|text| text.strip_prefix("create ").map(str::to_owned))
    else {
        return Vec::new();
    };
    let mut plus = vec![tokens.clone()];
    if let Some(rest) = tokens
        .strip_prefix("a ")
        .or_else(|| tokens.strip_prefix("an "))
    {
        plus.push(format!("an additional {rest}"));
    }
    plus.iter()
        .flat_map(|tokens| {
            [
                format!(
                    "if one or more {kind}tokens would be created under your control, those tokens plus {tokens} are created instead"
                ),
                format!("if you would create one or more {kind}tokens, instead create those tokens plus {tokens}"),
            ]
        })
        .collect()
}
