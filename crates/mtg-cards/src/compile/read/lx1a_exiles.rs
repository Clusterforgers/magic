//! The enter exiles linked to "return the exiled card" that need words of their own: several targets from a single
//! graveyard ("exile up to two target cards from a single graveyard"), targets on the battlefield and in graveyards
//! together ("up to three other target creatures from the battlefield and/or creature cards from graveyards"), a target
//! and the permanents that share its name ("exile target nonland permanent and all other permanents with the same name
//! as that permanent"), the card picked from a revealed hand said in two sentences, and the prohibition on casting
//! what the card exiled ("Your opponents can't cast spells with the same name as the exiled card."). Each is read as
//! its parts and written back so.

use super::amounts::{count_word, number_word};
use super::selectors::{Form, another, card_type, phrase, phrase_text, type_name};
use super::tokens::{Reader, same_text};
use crate::ability::Ability;
use crate::modify::{GameRule, Prohibited};
use crate::refs::{Bounds, Pred, Ref, Selector, TargetSet, TargetSpec, Targetable, ZoneKind};
use crate::value::Amount;

/// Whether a selector names a zone.
fn zoned(selector: &Selector) -> bool {
    selector.0.iter().any(|pred| matches!(pred, Pred::In(_)))
}

/// "up to three other target creatures from the battlefield and/or creature cards from graveyards": the targets are
/// either kind, the first on the battlefield and the second in graveyards (CR 115.1).
pub(super) fn lx1a_either_spec(r: &mut Reader<'_>) -> Option<TargetSpec> {
    let bounds = several(r)?;
    (bounds.fixed_min() == Some(0)).then_some(())?;
    let other = r.word("other");
    r.need("target")?;
    let first = phrase(r, Form::Targets)?;
    r.need("from the battlefield and")?;
    (r.punct('/') && r.word("or")).then_some(())?;
    let second = phrase(r, Form::Targets)?;
    (!zoned(&first) && second.0.contains(&Pred::In(ZoneKind::Graveyard))).then_some(())?;
    let first = if other { another(first) } else { first };
    Some(TargetSpec {
        what: Targetable::Either(vec![Targetable::Object(first), Targetable::Object(second)]),
        bounds,
        divide: None,
    })
}

/// The words of [`lx1a_either_spec`].
pub(super) fn lx1a_either_text(spec: &TargetSpec) -> Option<String> {
    let Targetable::Either(kinds) = &spec.what else {
        return None;
    };
    let [Targetable::Object(first), Targetable::Object(second)] = kinds.as_slice() else {
        return None;
    };
    (spec.bounds.fixed_min() == Some(0) && spec.divide.is_none()).then_some(())?;
    if zoned(first) || !second.0.contains(&Pred::In(ZoneKind::Graveyard)) {
        return None;
    }
    let count = match spec.bounds.max.as_ref()? {
        Amount::X => "X".to_owned(),
        Amount::Fixed(count @ 2..) => count_word(u32::try_from(*count).ok()?)?.to_owned(),
        _ => return None,
    };
    let other = first.0.contains(&Pred::Other);
    let plain = Selector(
        first
            .0
            .iter()
            .filter(|pred| **pred != Pred::Other)
            .cloned()
            .collect(),
    );
    Some(format!(
        "up to {count} {}target {} from the battlefield and/or {}",
        if other { "other " } else { "" },
        phrase_text(&plain, Form::Targets)?,
        phrase_text(second, Form::Targets)?
    ))
}

/// The count before "target" of several targets: "up to two", "up to X", "two", "X".
fn several(r: &mut Reader<'_>) -> Option<Bounds> {
    let up_to = r.words("up to");
    let count = if r.x() {
        Amount::X
    } else {
        let count = number_word(r.take_word()?).filter(|count| *count > 1)?;
        Amount::Fixed(i32::try_from(count).ok()?)
    };
    Some(if up_to {
        Bounds::up_to(Some(count))
    } else {
        Bounds::exactly(&count)
    })
}

/// "up to two target cards from a single graveyard", "two target creature cards from a single graveyard": several
/// cards, all in the graveyard of one player, the targets of one owner together (CR 115.3).
pub(super) fn lx1a_single_graveyard(r: &mut Reader<'_>) -> Option<TargetSpec> {
    let bounds = several(r)?;
    r.need("target")?;
    // "cards", "creature cards": the phrase reader names no card without a zone.
    let kind = r.attempt(|r| card_type(r.take_word()?));
    r.need("cards")?;
    let mut selector = Selector(kind.map(Pred::Type).into_iter().collect());
    r.need("from a single graveyard")?;
    selector.0.push(Pred::In(ZoneKind::Graveyard));
    Some(TargetSpec {
        what: Targetable::Together {
            what: Box::new(Targetable::Object(selector)),
            rule: TargetSet::SameController,
        },
        bounds,
        divide: None,
    })
}

/// The targets of an exile that this module reads: several from a single graveyard, or from the battlefield and
/// graveyards together.
pub(super) fn lx1a_grouped_target(r: &mut Reader<'_>) -> Option<Ref> {
    r.attempt(|r| {
        let what = super::selectors::target(r)?;
        is_grouped(&r.targets.first()?.what).then_some(what)
    })
}

/// Whether the targets are of a kind [`lx1a_grouped_target`] reads.
pub(super) fn is_grouped(what: &Targetable) -> bool {
    match what {
        Targetable::Together {
            what,
            rule: TargetSet::SameController,
        } => {
            matches!(&**what, Targetable::Object(selector) if selector.0.contains(&Pred::In(ZoneKind::Graveyard)))
        }
        Targetable::Either(kinds) => {
            matches!(kinds.as_slice(), [Targetable::Object(first), Targetable::Object(second)]
                if !zoned(first) && second.0.contains(&Pred::In(ZoneKind::Graveyard)))
        }
        _ => false,
    }
}

/// "not named ~": an object without the card's own name (CR 201.2), as a phrase ends.
pub(super) fn lx1a_suffix(r: &mut Reader<'_>) -> Option<Pred> {
    r.attempt(|r| {
        r.need("not named")?;
        r.source().then_some(())?;
        Some(Pred::Not(Box::new(Pred::SharesName(Ref::This))))
    })
}

/// The words of [`lx1a_suffix`].
pub(super) fn lx1a_suffix_text(pred: &Pred) -> Option<String> {
    let Pred::Not(inner) = pred else {
        return None;
    };
    (**inner == Pred::SharesName(Ref::This)).then(|| "not named ~".to_owned())
}

/// The target and the permanents that share its name.
fn with_namesakes(kind: Option<Pred>) -> Selector {
    let mut preds: Vec<Pred> = kind.into_iter().collect();
    preds.push(Pred::AnyOf(vec![
        Pred::Is(Ref::Target(0)),
        Pred::SharesName(Ref::Target(0)),
    ]));
    Selector(preds)
}

/// "and all other permanents with the same name as that permanent", "and all other creatures with the same name as that
/// creature" after a single target: the target and the permanents of that kind with its name (CR 201.2).
pub(super) fn lx1a_namesakes(r: &mut Reader<'_>) -> Option<Ref> {
    r.attempt(|r| {
        r.need("and all other")?;
        let plural = r.take_word()?;
        let noun = plural.strip_suffix('s')?;
        let kind = if noun == "permanent" {
            None
        } else {
            Some(Pred::Type(card_type(noun)?))
        };
        r.need("with the same name as that")?;
        r.need(noun)?;
        Some(Ref::Each(with_namesakes(kind)))
    })
}

/// The noun of a selector [`lx1a_namesakes`] reads: "permanent", "creature".
pub(super) fn lx1a_namesakes_noun(selector: &Selector) -> Option<&'static str> {
    let noun = match selector.0.as_slice() {
        [_] => "permanent",
        [Pred::Type(kind), _] => type_name(*kind),
        _ => return None,
    };
    let kind = match selector.0.as_slice() {
        [Pred::Type(kind), _] => Some(Pred::Type(*kind)),
        _ => None,
    };
    (*selector == with_namesakes(kind)).then_some(noun)
}

/// The words for the target `target` and its namesakes.
pub(super) fn lx1a_namesakes_text(target: &str, noun: &str) -> String {
    format!("{target} and all other {noun}s with the same name as that {noun}")
}

/// The cards the source exiled while it was on the battlefield.
fn exiled_with_this() -> Ref {
    Ref::Each(Selector(vec![
        Pred::In(ZoneKind::Exile),
        Pred::ExiledWith(Ref::This),
    ]))
}

/// The players a prohibition names and their words.
const CASTERS: [(&str, Ref); 3] = [
    ("Your opponents", Ref::Opponents),
    ("Players", Ref::EachPlayer),
    ("You", Ref::You),
];

/// The words for the cards the source exiled, as the object of "with the same name as".
const EXILED: [&str; 2] = ["the exiled card", "a card exiled with ~"];

/// "Your opponents can't cast spells with the same name as the exiled card.": no spell that shares a name with a card
/// the source exiled can be cast by those players (CR 201.2, 607.2a).
fn cast_rule(who: Ref) -> Ability {
    Ability::Rule {
        rule: GameRule::Cant {
            who,
            action: Prohibited::Cast {
                filter: Selector(vec![Pred::SharesName(exiled_with_this())]),
                more_than: None,
                unless: None,
            },
        },
        condition: None,
        from: ZoneKind::Battlefield,
    }
}

/// The wordings of the cast prohibitions, each with its ability.
fn cast_rules() -> impl Iterator<Item = (String, Ability)> {
    CASTERS.iter().flat_map(|(players, who)| {
        EXILED.iter().map(move |exiled| {
            (
                format!("{players} can't cast spells with the same name as {exiled}."),
                cast_rule(who.clone()),
            )
        })
    })
}

/// Reads a cast prohibition of [`cast_rule`].
pub(super) fn lx1a_rule_read(text: &str) -> Option<Ability> {
    cast_rules()
        .find(|(words, _)| same_text(words, text))
        .map(|(_, ability)| ability)
}

/// The end of the one-sentence pick from a revealed hand.
const JOINED: &str = " from it and exile that card";

/// Adds the words of a cast prohibition, and the other wordings of a card picked from a revealed hand: "target
/// opponent reveals their hand and you choose a nonland card from it. Exile that card." and "... from it. Exile that
/// card.".
pub(super) fn lx1a_wordings(ability: &Ability, texts: &mut Vec<String>) {
    for (words, rule) in cast_rules() {
        if rule == *ability && !texts.contains(&words) {
            texts.push(words);
        }
    }
    let mut more = Vec::new();
    for text in texts.iter() {
        if !text.contains(" reveals their hand. You choose ") || !text.contains(JOINED) {
            continue;
        }
        let apart = text.replacen(JOINED, " from it. Exile that card", 1);
        more.push(apart.replacen(
            " reveals their hand. You choose ",
            " reveals their hand and you choose ",
            1,
        ));
        more.push(apart);
    }
    for text in more {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: each target phrase of the module reads into one specification and is written back as printed, the "not
    //! named ~" suffix and the namesakes of a target round-trip, the cast prohibition reads only for its players and
    //! words, and near misses read as nothing.
    #![allow(clippy::unwrap_used)]
    use super::super::selectors::{spec, spec_text};
    use super::super::tokens::tokenize;
    use super::*;

    /// The whole text as one target specification.
    fn read_spec(text: &str) -> Option<TargetSpec> {
        let tokens = tokenize(text);
        let mut r = Reader::new(&tokens, true);
        let found = spec(&mut r)?;
        r.end().then_some(found)
    }

    #[rstest::rstest]
    #[case("up to two target cards from a single graveyard")]
    #[case("three target cards from a single graveyard")]
    #[case("X target cards from a single graveyard")]
    #[case("two target creature cards from a single graveyard")]
    #[case("up to two target cards from graveyards")]
    #[case(
        "up to three other target creatures from the battlefield and/or creature cards from graveyards"
    )]
    #[case(
        "up to X other target creatures from the battlefield and/or creature cards from graveyards"
    )]
    #[case("target nonland permanent not named ~")]
    fn lx1a_a_target_phrase_reads_and_writes_back(#[case] text: &str) {
        let found = read_spec(text).unwrap_or_else(|| panic!("{text}"));
        assert_eq!(spec_text(&found).as_deref(), Some(text));
    }

    #[rstest::rstest]
    #[case("target card from a single graveyard")]
    #[case("up to two target cards from a single library")]
    #[case("up to three target creatures from the battlefield and/or creatures")]
    #[case("target nonland permanent not named Detention Sphere")]
    fn lx1a_a_near_miss_is_no_target_phrase(#[case] text: &str) {
        assert_eq!(read_spec(text), None, "{text}");
    }

    #[test]
    fn lx1a_cards_from_a_single_graveyard_are_one_owners() {
        let found = read_spec("up to two target cards from a single graveyard").unwrap();
        assert_eq!(
            found.what,
            Targetable::Together {
                what: Box::new(Targetable::Object(Selector(vec![Pred::In(
                    ZoneKind::Graveyard
                )]))),
                rule: TargetSet::SameController,
            }
        );
        assert!(is_grouped(&found.what));
    }

    #[rstest::rstest]
    #[case(
        "and all other permanents with the same name as that permanent",
        Some("permanent")
    )]
    #[case(
        "and all other creatures with the same name as that creature",
        Some("creature")
    )]
    #[case("and all other creatures with the same name as that permanent", None)]
    #[case("and all other cards with the same name as that card", None)]
    fn lx1a_the_namesakes_round_trip(#[case] text: &str, #[case] noun: Option<&str>) {
        let tokens = tokenize(text);
        let mut r = Reader::new(&tokens, false);
        let found = lx1a_namesakes(&mut r).filter(|_| r.end());
        let written = found.as_ref().and_then(|what| match what {
            Ref::Each(selector) => lx1a_namesakes_noun(selector),
            _ => None,
        });
        assert_eq!(written, noun, "{text}");
        if let Some(noun) = noun {
            assert_eq!(
                lx1a_namesakes_text("target X", noun),
                format!("target X {text}")
            );
        }
    }

    #[rstest::rstest]
    #[case(
        "Your opponents can't cast spells with the same name as the exiled card.",
        Some(Ref::Opponents)
    )]
    #[case(
        "Players can't cast spells with the same name as the exiled card.",
        Some(Ref::EachPlayer)
    )]
    #[case(
        "You can't cast spells with the same name as a card exiled with ~.",
        Some(Ref::You)
    )]
    #[case(
        "Your opponents can't cast spells with the same name as the chosen card.",
        None
    )]
    #[case(
        "Each opponent can't cast spells with the same name as the exiled card.",
        None
    )]
    #[case(
        "Your opponents can't cast creature spells with the same name as the exiled card.",
        None
    )]
    fn lx1a_the_cast_prohibition_names_its_players(#[case] text: &str, #[case] who: Option<Ref>) {
        let found = lx1a_rule_read(text);
        assert_eq!(found, who.map(cast_rule), "{text}");
        if let Some(found) = found {
            let mut texts = Vec::new();
            lx1a_wordings(&found, &mut texts);
            assert!(
                texts.iter().any(|written| same_text(written, text)),
                "{texts:?}"
            );
        }
    }

    #[test]
    fn lx1a_the_hand_pick_is_written_in_each_printed_form() {
        let mut texts = vec![
            "Target opponent reveals their hand. You choose a nonland card from it and exile that card.".to_owned(),
        ];
        lx1a_wordings(
            &Ability::Keyword(crate::keyword::Keyword::Flying),
            &mut texts,
        );
        assert_eq!(
            texts[1..],
            [
                "Target opponent reveals their hand and you choose a nonland card from it. Exile that card.",
                "Target opponent reveals their hand. You choose a nonland card from it. Exile that card.",
            ]
        );
    }
}
