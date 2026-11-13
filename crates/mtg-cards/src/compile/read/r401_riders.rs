//! A cast permission with riders on the spells cast through it: "Once during each of your turns, you may cast an
//! artifact spell from your graveyard. If you cast a spell this way, that artifact enters tapped." The permission is any
//! static the other readers word; the second sentence adds `CastRider`s, and the writer words the permission without its
//! riders and then the riders.

use mtg_core::card::{CardType, Subtype};

use super::counters;
use super::selectors::{keyword, keyword_text};
use super::tokens::Reader;
use super::triggers::article;
use crate::ability::Ability;
use crate::modify::{CastPermission, CastRider, GameRule};
use crate::refs::Pred;
use crate::value::{Condition, CounterKind};

/// The permanent types a rider may name its spell by ("that artifact", "that creature").
const NOUNS: [(CardType, &str); 4] = [
    (CardType::Artifact, "artifact"),
    (CardType::Creature, "creature"),
    (CardType::Enchantment, "enchantment"),
    (CardType::Planeswalker, "planeswalker"),
];

/// A permission sentence `base` reads, its period, then the sentence of its riders.
pub fn r401_riders(
    r: &mut Reader<'_>,
    base: impl Fn(&mut Reader<'_>) -> Option<Ability>,
) -> Option<Ability> {
    let during = r.words("during your turn");
    if during {
        r.punct(',').then_some(())?;
    }
    let ability = r
        .attempt(|r| {
            let found = base(r)?;
            r.at_punct('.').then_some(found)
        })
        .or_else(|| r401_from_graveyard(r))?;
    r.punct('.').then_some(())?;
    let Ability::Rule {
        rule: GameRule::MayCast(permission),
        condition,
        from,
    } = ability
    else {
        return None;
    };
    if !permission.riders.is_empty() || (during && permission.when.is_some()) {
        return None;
    }
    let riders = rider_sentence(r, &permission)?;
    Some(Ability::Rule {
        rule: GameRule::MayCast(CastPermission {
            riders,
            when: if during {
                Some(Condition::YourTurn)
            } else {
                permission.when
            },
            ..permission
        }),
        condition,
        from,
    })
}

/// Whether the rest of the clause says "this way", read without moving: the cheap test before the permissions are tried.
pub fn r401_says_this_way(r: &mut Reader<'_>) -> Option<()> {
    r.lookahead(|r| {
        while !r.end() {
            if r.words("this way") {
                return Some(());
            }
            r.skip();
        }
        None
    })
}

/// "you may cast creature spells with power or toughness 1 or less from your graveyard": the permission no other reader
/// words, read only before a sentence of riders.
fn r401_from_graveyard(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("you may cast")?;
    let spells = super::selectors::phrase(r, super::selectors::Form::All)?;
    let [Pred::Spell, kinds @ ..] = spells.0.as_slice() else {
        return None;
    };
    // The phrase may take "from your graveyard" in as the zone of the spell's card.
    let kinds = if let [
        kinds @ ..,
        Pred::In(crate::refs::ZoneKind::Graveyard),
        Pred::OwnedBy(crate::refs::Ref::You),
    ] = kinds
    {
        kinds
    } else {
        r.need("from your graveyard")?;
        kinds
    };
    (!kinds.is_empty()).then_some(())?;
    Some(Ability::Rule {
        rule: GameRule::MayCast(CastPermission {
            what: crate::refs::Selector(kinds.to_vec()),
            from: Some(crate::refs::ZoneKind::Graveyard),
            ..CastPermission::default()
        }),
        condition: None,
        from: crate::refs::ZoneKind::Battlefield,
    })
}

/// The wording of the permission `r401_from_graveyard` reads, none for another.
fn r401_from_graveyard_text(ability: &Ability) -> Option<String> {
    let Ability::Rule {
        rule: GameRule::MayCast(permission),
        condition: None,
        from: crate::refs::ZoneKind::Battlefield,
    } = ability
    else {
        return None;
    };
    let plain = CastPermission {
        what: permission.what.clone(),
        from: Some(crate::refs::ZoneKind::Graveyard),
        ..CastPermission::default()
    };
    (*permission == plain && !permission.what.0.is_empty()).then_some(())?;
    // The phrase writer has no wording for "with power or toughness N or less", which follows the noun.
    let either = permission
        .what
        .0
        .iter()
        .find_map(r401_power_or_toughness_text);
    let mut preds = vec![Pred::Spell];
    preds.extend(
        permission
            .what
            .0
            .iter()
            .filter(|pred| r401_power_or_toughness_text(pred).is_none())
            .cloned(),
    );
    if let Some(either) = either {
        let spells = super::selectors::phrase_text(
            &crate::refs::Selector(preds),
            super::selectors::Form::All,
        )?;
        return Some(format!(
            "You may cast {spells} {either} from your graveyard"
        ));
    }
    let spells = super::selectors::phrase_text(
        &crate::refs::Selector(preds.clone()),
        super::selectors::Form::All,
    )
    .map(|spells| format!("{spells} from your graveyard"));
    preds.extend([
        Pred::In(crate::refs::ZoneKind::Graveyard),
        Pred::OwnedBy(crate::refs::Ref::You),
    ]);
    let spells = spells.or_else(|| {
        super::selectors::phrase_text(&crate::refs::Selector(preds), super::selectors::Form::All)
    })?;
    Some(format!("You may cast {spells}"))
}

/// "If you cast a spell this way, that artifact enters tapped", "If a spell cast this way would be put into your
/// graveyard, exile it instead".
fn rider_sentence(r: &mut Reader<'_>, permission: &CastPermission) -> Option<Vec<CastRider>> {
    let exiled = r.attempt(|r| {
        r.need("if a spell cast this way would be put into your graveyard")?;
        r.punct(',').then_some(())?;
        r.need("exile it instead")
    });
    if exiled.is_some() {
        return Some(vec![CastRider::ExiledInstead]);
    }
    r.need("if you cast")?;
    (r.word("a") || r.word("an")).then_some(())?;
    let kind = noun(permission);
    if !r.word("spell") {
        let named = r.take_word()?;
        (Some(named) == kind && r.word("spell")).then_some(())?;
    }
    r.need("this way")?;
    r.punct(',').then_some(())?;
    if !r.word("it") {
        r.need("that")?;
        let named = r.take_word()?;
        (Some(named) == kind).then_some(())?;
    }
    let mut riders = vec![entering(r)?];
    if r.words("and is a") || r.words("and is an") {
        let word = super::selectors::subtype(r)?;
        r.need("in addition to its other types")?;
        riders.push(CastRider::AlsoSubtype(Subtype(word)));
    }
    Some(riders)
}

/// What the permanent gets: "enters tapped", "enters with a finality counter on it", "gains haste until end of turn".
fn entering(r: &mut Reader<'_>) -> Option<CastRider> {
    if r.words("enters tapped") {
        return Some(CastRider::EntersTapped);
    }
    if r.words("enters with") {
        // A finality counter is a counter of the rules the counter readers leave alone; the engine runs it (CR 122.1g).
        let counter = if r.words("a finality counter") {
            CounterKind::Other("finality".to_owned())
        } else {
            let (counter, number) = counters::counters(r)?;
            (number == 1).then_some(counter)?
        };
        r.need("on it")?;
        return Some(CastRider::EntersWithCounter(counter));
    }
    r.need("gains")?;
    let gained = keyword(r)?;
    r.need("until end of turn")?;
    Some(CastRider::GainsThisTurn(gained))
}

/// The one permanent type the permission's cards have, as a rider names their spell; none for another selector.
fn noun(permission: &CastPermission) -> Option<&'static str> {
    let mut named = permission.what.0.iter().filter_map(|pred| match pred {
        Pred::Type(card_type) => NOUNS
            .iter()
            .find(|(kind, _)| kind == card_type)
            .map(|(_, word)| *word),
        _ => None,
    });
    let first = named.next()?;
    named.next().is_none().then_some(first)
}

/// The wordings of a permission with riders: each wording of it without them, then the sentence of its riders; `None`
/// for any other ability, so the other writers word it.
pub fn r401_rider_texts(
    ability: &Ability,
    written: impl Fn(&Ability) -> Vec<String>,
) -> Option<Vec<String>> {
    let Ability::Rule {
        rule: GameRule::MayCast(permission),
        condition,
        from,
    } = ability
    else {
        return None;
    };
    if permission.riders.is_empty() {
        return None;
    }
    let during = permission.when.as_ref() == Some(&Condition::YourTurn);
    let bare = Ability::Rule {
        rule: GameRule::MayCast(CastPermission {
            riders: Vec::new(),
            when: if during {
                None
            } else {
                permission.when.clone()
            },
            ..permission.clone()
        }),
        condition: condition.clone(),
        from: *from,
    };
    let sentences = rider_texts(permission);
    let mut bases = written(&bare);
    if bases.is_empty() {
        bases.extend(r401_from_graveyard_text(&bare));
    }
    Some(
        bases
            .iter()
            .flat_map(|text| {
                let text = super::tokens::without_period(text);
                let text = if during {
                    format!("During your turn, {}", lowered(text))
                } else {
                    text.to_owned()
                };
                sentences
                    .iter()
                    .map(move |sentence| format!("{text}. {sentence}"))
            })
            .collect(),
    )
}

/// The text with its first letter in lower case.
fn lowered(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(chars).collect()
    })
}

/// The sentences that word the riders of the permission.
fn rider_texts(permission: &CastPermission) -> Vec<String> {
    if permission.riders == [CastRider::ExiledInstead] {
        return vec![
            "If a spell cast this way would be put into your graveyard, exile it instead"
                .to_owned(),
        ];
    }
    let [first, rest @ ..] = permission.riders.as_slice() else {
        return Vec::new();
    };
    let Some(gets) = entering_text(first) else {
        return Vec::new();
    };
    let also = match rest {
        [] => String::new(),
        [CastRider::AlsoSubtype(Subtype(kind))] => {
            format!(
                " and is {} {kind} in addition to its other types",
                article(kind)
            )
        }
        _ => return Vec::new(),
    };
    let kind = noun(permission);
    let heads = std::iter::once("a spell".to_owned())
        .chain(kind.map(|kind| format!("{} {kind} spell", article(kind))));
    let objects = std::iter::once("it".to_owned()).chain(kind.map(|kind| format!("that {kind}")));
    let objects: Vec<String> = objects.collect();
    heads
        .flat_map(|head| {
            objects
                .iter()
                .map(|object| format!("If you cast {head} this way, {object} {gets}{also}"))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// What the permanent gets, as `entering` reads it.
fn entering_text(rider: &CastRider) -> Option<String> {
    match rider {
        CastRider::EntersTapped => Some("enters tapped".to_owned()),
        CastRider::EntersWithCounter(counter) => {
            let kind = counters::kind_text(counter)?;
            Some(format!(
                "enters with {} {kind} counter on it",
                article(kind)
            ))
        }
        CastRider::GainsThisTurn(gained) => Some(format!(
            "gains {} until end of turn",
            keyword_text(gained)?.to_lowercase()
        )),
        CastRider::AlsoSubtype(_) | CastRider::ExiledInstead => None,
    }
}

/// The suffix `r401_with_power_or_toughness` reads, else the suffixes of `w21a_atoms`.
pub(super) fn r401_suffix(r: &mut Reader<'_>) -> Option<Pred> {
    r401_with_power_or_toughness(r).or_else(|| super::w21a_atoms::w21a_suffix(r))
}

/// "with power or toughness 1 or less": either stat compared with one number.
pub(super) fn r401_with_power_or_toughness(r: &mut Reader<'_>) -> Option<Pred> {
    r.attempt(|r| {
        r.need("with")?;
        r401_power_or_toughness(r)
    })
}

/// "power or toughness 1 or less", after "with": either stat compared with one number.
pub(super) fn r401_power_or_toughness(r: &mut Reader<'_>) -> Option<Pred> {
    r.need("power or toughness")?;
    let to = crate::value::Amount::Fixed(i32::try_from(r.number()?).ok()?);
    let op = if r.words("or less") {
        crate::value::Cmp::Le
    } else {
        r.need("or greater")?;
        crate::value::Cmp::Ge
    };
    let compare = |stat| Pred::Compare {
        stat,
        op,
        to: to.clone(),
    };
    Some(Pred::AnyOf(vec![
        compare(crate::value::StatKind::Power),
        compare(crate::value::StatKind::Toughness),
    ]))
}

/// The wording of the predicate `r401_power_or_toughness` reads, with its "with"; none for another.
pub(super) fn r401_power_or_toughness_text(pred: &Pred) -> Option<String> {
    use crate::value::{Amount, Cmp, StatKind};
    let Pred::AnyOf(either) = pred else {
        return None;
    };
    let [
        Pred::Compare {
            stat: StatKind::Power,
            op,
            to: Amount::Fixed(number),
        },
        Pred::Compare {
            stat: StatKind::Toughness,
            op: same_op,
            to: Amount::Fixed(same_number),
        },
    ] = either.as_slice()
    else {
        return None;
    };
    (op == same_op && number == same_number).then_some(())?;
    let tail = match op {
        Cmp::Le => "or less",
        Cmp::Ge => "or greater",
        _ => return None,
    };
    Some(format!("with power or toughness {number} {tail}"))
}
