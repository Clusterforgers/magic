//! r47-04 wordings: "Whenever ~ deals combat damage to a creature, if that creature was dealt excess damage this turn,
//! you get X {E}, where X is that excess damage." (the excess counted from the turn's log of that creature); a face-down
//! pile that is manifested or cloaked ("Exile all creature cards from target player's graveyard in a face-down pile,
//! shuffle that pile, then manifest those cards.", "exile it and the top card of your library in a face-down pile, ...",
//! "Exile target nontoken creature you own and the top two cards of your library ..., then cloak those cards. They
//! enter tapped.") or put back on top of the library ("exile it and the top six cards of your library in a face-down
//! pile. If you do, shuffle that pile and put it back on top of your library."); "Each player chooses any number of
//! creatures they control with total power 4 or less, then sacrifices all other creatures they control."; "fortified
//! land" (the land a Fortification is attached to), read and written as its twin "enchanted land".

use mtg_core::card::CardType;

use super::ClauseKind;
use super::amounts::{count_word, number_word};
use super::selectors::{Form, phrase, phrase_text};
use super::tokens::{Reader, tokenize};
use crate::ability::Ability;
use crate::effect::{Destination, Effect, Order, Place};
use crate::refs::{Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use crate::trigger::TriggerPattern;
use crate::value::{Amount, Cmp, Condition, CounterKind, TallyEvent};

/// The head of a trigger on a creature this one dealt damage to that was dealt excess damage this turn.
const R4704_EXCESS_HEAD: &str = ", if that creature was dealt excess damage this turn, you get X {E}, where X is that excess damage.";

/// The excess damage dealt this turn to the creature the trigger is about.
fn r4704_that_excess() -> Amount {
    Amount::Tally {
        event: TallyEvent::DealtExcess,
        who: Ref::EachPlayer,
        filter: Selector(vec![Pred::Is(Ref::Triggering)]),
    }
}

/// "Whenever ~ deals [combat ]damage to a creature, if that creature was dealt excess damage this turn, you get X {E},
/// where X is that excess damage."
fn r4704_excess_energy(combat: Option<bool>) -> Ability {
    Ability::Triggered {
        when: TriggerPattern::DealsDamage {
            source: Selector::this(),
            to: Some(Ref::Each(Selector(vec![Pred::Type(CardType::Creature)]))),
            combat,
        },
        intervening_if: Some(Condition::Matches {
            subject: Ref::Triggering,
            selector: Selector(vec![Pred::DidThisTurn {
                event: TallyEvent::DealtExcess,
                by: None,
                to: None,
                combat: false,
            }]),
        }),
        limit: None,
        from: ZoneKind::Battlefield,
        targets: Vec::new(),
        effects: vec![Effect::PutCounters {
            on: Ref::You,
            counter: CounterKind::Other("energy".to_owned()),
            count: r4704_that_excess(),
        }],
    }
}

/// The trigger heads `r4704_excess_energy` is written with.
fn r4704_excess_heads() -> [(Option<bool>, &'static str); 2] {
    [
        (Some(true), "Whenever ~ deals combat damage to a creature"),
        (None, "Whenever ~ deals damage to a creature"),
    ]
}

/// The words between what a face-down pile holds and what is done with it.
const R4704_PILE: &str = " in a face-down pile, shuffle that pile, then ";
/// The name the exiled object of a pile is bound to.
const R4704_PILE_NAME: &str = "pile";
/// The name the exiled library cards of a pile are bound to.
const R4704_PILE_TOP: &str = "pile_top";
/// The sentence after a pile that is put back on top of the library.
const R4704_PILE_BACK: &str =
    " in a face-down pile. If you do, shuffle that pile and put it back on top of your library.";

/// What a pile holds besides the cards of the library and what is done with it.
#[derive(Debug, Clone, PartialEq)]
struct R4704Pile {
    /// The object or objects exiled.
    what: Ref,
    /// How many cards from the top of the controller's library join it.
    top: u32,
    /// Cloaked rather than manifested.
    cloak: bool,
    /// They enter tapped.
    tapped: bool,
}

/// The steps of a pile: exiled face down, then manifested or cloaked.
fn r4704_pile_steps(pile: &R4704Pile) -> Vec<Effect> {
    let mut steps = vec![Effect::ExileHidden {
        what: pile.what.clone(),
        held: false,
        sealed: true,
        bind: Some(R4704_PILE_NAME.to_owned()),
    }];
    let mut cards = Ref::Bound(R4704_PILE_NAME.to_owned());
    if pile.top > 0 {
        steps.push(Effect::ExileTop {
            who: Ref::You,
            count: Amount::Fixed(i32::try_from(pile.top).unwrap_or(i32::MAX)),
            bind: R4704_PILE_TOP.to_owned(),
            sealed: true,
        });
        cards = Ref::Union(vec![cards, Ref::Bound(R4704_PILE_TOP.to_owned())]);
    }
    steps.push(Effect::ManifestCards {
        who: Ref::You,
        what: cards,
        cloak: pile.cloak,
        tapped: pile.tapped,
        bind: None,
    });
    steps
}

/// The pile whose steps `effects` are.
fn r4704_pile_of(effects: &[Effect]) -> Option<R4704Pile> {
    let Some(Effect::ManifestCards {
        who: Ref::You,
        cloak,
        tapped,
        bind: None,
        ..
    }) = effects.last()
    else {
        return None;
    };
    let Some(Effect::ExileHidden { what, .. }) = effects.first() else {
        return None;
    };
    let top = match effects.get(1) {
        Some(Effect::ExileTop {
            count: Amount::Fixed(n),
            ..
        }) => u32::try_from(*n).ok()?,
        _ => 0,
    };
    let pile = R4704Pile {
        what: what.clone(),
        top,
        cloak: *cloak,
        tapped: *tapped,
    };
    (r4704_pile_steps(&pile) == effects).then_some(pile)
}

/// " and the top card of your library" for a count of library cards; nothing for none.
fn r4704_top_words(top: u32) -> Option<String> {
    Some(match top {
        0 => String::new(),
        1 => " and the top card of your library".to_owned(),
        n => format!(" and the top {} cards of your library", count_word(n)?),
    })
}

/// The words after a pile: what is done with it.
fn r4704_pile_tail(pile: &R4704Pile) -> String {
    format!(
        "{R4704_PILE}{} those cards.{}",
        if pile.cloak { "cloak" } else { "manifest" },
        if pile.tapped {
            " They enter tapped."
        } else {
            ""
        }
    )
}

/// The count of library cards a pile's words name, and the words before them.
fn r4704_top_read(text: &str) -> Option<(&str, u32)> {
    if let Some(head) = text.strip_suffix(" and the top card of your library") {
        return Some((head, 1));
    }
    let (head, rest) = text.rsplit_once(" and the top ")?;
    let word = rest.strip_suffix(" cards of your library")?;
    Some((head, number_word(word).filter(|n| *n > 1)?))
}

/// The selector of a phrase read whole in a form.
fn r4704_phrase(words: &str, form: Form) -> Option<Selector> {
    let tokens = tokenize(words);
    let mut r = Reader::new(&tokens, false);
    let selector = phrase(&mut r, form)?;
    r.end().then_some(selector)
}

/// A spell of a pile: "Exile all <cards> from target player's graveyard" or "Exile target <object> and the top N
/// cards of your library", then the pile's words.
fn r4704_pile_spell_read(text: &str) -> Option<Ability> {
    let (head, rest) = text.split_once(R4704_PILE)?;
    let (cloak, tapped) = match rest {
        "manifest those cards." => (false, false),
        "cloak those cards." => (true, false),
        "manifest those cards. They enter tapped." => (false, true),
        "cloak those cards. They enter tapped." => (true, true),
        _ => return None,
    };
    let (head, top) = r4704_top_read(head).unwrap_or((head, 0));
    if let Some(cards) = head
        .strip_prefix("Exile all ")
        .and_then(|rest| rest.strip_suffix(" cards from target player's graveyard"))
    {
        let mut selector = r4704_phrase(cards, Form::Each)?;
        selector.0.push(Pred::In(ZoneKind::Graveyard));
        selector.0.push(Pred::OwnedBy(Ref::Target(0)));
        let pile = R4704Pile {
            what: Ref::Each(selector),
            top,
            cloak,
            tapped,
        };
        return Some(r4704_pile_spell(Targetable::Player, &pile));
    }
    let object = head.strip_prefix("Exile target ")?;
    let selector = match object.strip_suffix(" you own") {
        Some(object) => {
            let mut selector = r4704_phrase(object, Form::Target)?;
            selector.0.push(Pred::OwnedBy(Ref::You));
            selector
        }
        None => r4704_phrase(object, Form::Target)?,
    };
    let pile = R4704Pile {
        what: Ref::Target(0),
        top,
        cloak,
        tapped,
    };
    Some(r4704_pile_spell(Targetable::Object(selector), &pile))
}

/// A spell of one target whose steps are a pile.
fn r4704_pile_spell(what: Targetable, pile: &R4704Pile) -> Ability {
    Ability::Spell {
        targets: vec![TargetSpec {
            what,
            bounds: crate::refs::Bounds::default(),
            divide: None,
        }],
        effects: r4704_pile_steps(pile),
    }
}

/// The words of a spell of a pile.
fn r4704_pile_spell_text(ability: &Ability) -> Option<String> {
    let Ability::Spell { targets, effects } = ability else {
        return None;
    };
    let [spec] = targets.as_slice() else {
        return None;
    };
    if spec.divide.is_some() || spec.bounds != crate::refs::Bounds::default() {
        return None;
    }
    let pile = r4704_pile_of(effects)?;
    let top = r4704_top_words(pile.top)?;
    let head = match (&spec.what, &pile.what) {
        (Targetable::Player, Ref::Each(selector)) => {
            let [
                cards @ ..,
                Pred::In(ZoneKind::Graveyard),
                Pred::OwnedBy(Ref::Target(0)),
            ] = selector.0.as_slice()
            else {
                return None;
            };
            format!(
                "Exile all {} cards from target player's graveyard",
                phrase_text(&Selector(cards.to_vec()), Form::Each)?
            )
        }
        (Targetable::Object(selector), Ref::Target(0)) => match selector.0.split_last() {
            Some((Pred::OwnedBy(Ref::You), rest)) => format!(
                "Exile target {} you own",
                phrase_text(&Selector(rest.to_vec()), Form::Target)?
            ),
            _ => format!("Exile target {}", phrase_text(selector, Form::Target)?),
        },
        _ => return None,
    };
    Some(format!("{head}{top}{}", r4704_pile_tail(&pile)))
}

/// A trigger of a pile of its source: "<head>, exile it and the top card of your library in a face-down pile, ...".
fn r4704_pile_trigger_read(text: &str, read: &impl Fn(&str) -> Option<Ability>) -> Option<Ability> {
    let (head, back) = if let Some(head) = text.strip_suffix(R4704_PILE_BACK) {
        (head, true)
    } else {
        (text.split_once(R4704_PILE)?.0, false)
    };
    let (head, top) = r4704_top_read(head)?;
    let head = head.strip_suffix(", exile it")?;
    let head = head.replacen("this creature", "~", 1);
    let twin = if head == R4704_YOUR_GRAVEYARD {
        r4704_exiles_itself(r4704_your_graveyard())
    } else {
        let rest = head
            .strip_prefix("Whenever ")
            .or_else(|| head.strip_prefix("When "))?;
        read(&format!("Whenever {rest}, exile ~."))?
    };
    let Ability::Triggered {
        when,
        intervening_if: None,
        limit,
        from,
        targets,
        effects,
    } = twin
    else {
        return None;
    };
    (targets.is_empty()
        && effects
            == [Effect::Exile {
                what: Ref::This,
                bind: None,
                duration: None,
            }])
    .then_some(())?;
    let effects = if back {
        r4704_pile_back(top)
    } else {
        let rest = text.split_once(R4704_PILE)?.1;
        let cloak = match rest {
            "manifest those cards." => false,
            "cloak those cards." => true,
            _ => return None,
        };
        r4704_pile_steps(&R4704Pile {
            what: Ref::This,
            top,
            cloak,
            tapped: false,
        })
    };
    Some(Ability::Triggered {
        when,
        intervening_if: None,
        limit,
        from,
        targets,
        effects,
    })
}

/// "When ~ is put into your graveyard from the battlefield", which no other writer words.
const R4704_YOUR_GRAVEYARD: &str = "When ~ is put into your graveyard from the battlefield";

/// The event of `R4704_YOUR_GRAVEYARD`.
fn r4704_your_graveyard() -> TriggerPattern {
    TriggerPattern::ZoneChange {
        what: Selector(vec![Pred::This, Pred::OwnedBy(Ref::You)]),
        from: Some(ZoneKind::Battlefield),
        to: Some(ZoneKind::Graveyard),
    }
}

/// A trigger on `when` that exiles its source.
fn r4704_exiles_itself(when: TriggerPattern) -> Ability {
    Ability::Triggered {
        when,
        intervening_if: None,
        limit: None,
        from: ZoneKind::Battlefield,
        targets: Vec::new(),
        effects: vec![Effect::Exile {
            what: Ref::This,
            bind: None,
            duration: None,
        }],
    }
}

/// The steps of a pile of the source and the top cards of the library put back on top in a random order: done only
/// when the source is still in a graveyard and the library holds that many cards (otherwise nothing is exiled).
fn r4704_pile_back(top: u32) -> Vec<Effect> {
    let count = i32::try_from(top).unwrap_or(i32::MAX);
    let card = Ref::Arrived {
        of: Box::new(Ref::This),
        zone: ZoneKind::Graveyard,
    };
    vec![Effect::If {
        condition: Condition::All(vec![
            Condition::CountOf {
                subject: card.clone(),
                selector: Selector(vec![Pred::In(ZoneKind::Graveyard)]),
                op: Cmp::Ge,
                right: Amount::Fixed(1),
            },
            Condition::Compare {
                left: Amount::Count(Selector(vec![
                    Pred::In(ZoneKind::Library),
                    Pred::OwnedBy(Ref::You),
                ])),
                op: Cmp::Ge,
                right: Amount::Fixed(count),
            },
        ]),
        then: vec![
            Effect::ExileHidden {
                what: card,
                held: false,
                sealed: true,
                bind: Some(R4704_PILE_NAME.to_owned()),
            },
            Effect::ExileTop {
                who: Ref::You,
                count: Amount::Fixed(count),
                bind: R4704_PILE_TOP.to_owned(),
                sealed: true,
            },
            Effect::Move {
                what: Ref::Union(vec![
                    Ref::Bound(R4704_PILE_NAME.to_owned()),
                    Ref::Bound(R4704_PILE_TOP.to_owned()),
                ]),
                to: Destination::Library {
                    place: Place::Top,
                    order: Some(Order::Random),
                },
                bind: None,
            },
        ],
        otherwise: Vec::new(),
    }]
}

/// The words of a trigger of a pile of its source, written from its twin that exiles it.
fn r4704_pile_trigger_texts(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
) -> Vec<String> {
    let Ability::Triggered {
        when,
        intervening_if: None,
        limit,
        from,
        targets,
        effects,
    } = ability
    else {
        return Vec::new();
    };
    if !targets.is_empty() {
        return Vec::new();
    }
    let (top, tail) = if let Some(pile) = r4704_pile_of(effects) {
        if pile.what != Ref::This || pile.tapped || pile.top == 0 {
            return Vec::new();
        }
        (pile.top, r4704_pile_tail(&pile))
    } else if let Some(top) = (1..=20).find(|top| r4704_pile_back(*top) == *effects) {
        (top, R4704_PILE_BACK.to_owned())
    } else {
        return Vec::new();
    };
    let Some(words) = r4704_top_words(top) else {
        return Vec::new();
    };
    let twin = Ability::Triggered {
        when: when.clone(),
        intervening_if: None,
        limit: *limit,
        from: *from,
        targets: Vec::new(),
        effects: vec![Effect::Exile {
            what: Ref::This,
            bind: None,
            duration: None,
        }],
    };
    let heads: Vec<String> = if *when == r4704_your_graveyard() {
        vec![R4704_YOUR_GRAVEYARD.to_owned()]
    } else {
        written(&twin)
            .iter()
            .filter_map(|text| text.strip_suffix(", exile it."))
            .map(str::to_owned)
            .collect()
    };
    heads
        .iter()
        .map(|head| format!("{head}, exile it{words}{tail}"))
        .collect()
}

/// The sentence of a choice of permanents kept within a total of power, around the plural noun and the most.
fn r4704_keep_power_text(what: &Selector, most: i32) -> Option<String> {
    let noun = phrase_text(what, Form::All)?;
    Some(format!(
        "Each player chooses any number of {noun} they control with total power {most} or less, then sacrifices all \
         other {noun} they control."
    ))
}

/// The spell `r4704_keep_power_text` words.
fn r4704_keep_power_read(text: &str) -> Option<Ability> {
    let rest = text.strip_prefix("Each player chooses any number of ")?;
    let (noun, rest) = rest.split_once(" they control with total power ")?;
    let (most, rest) = rest.split_once(" or less, then sacrifices all other ")?;
    (rest.strip_suffix(" they control.")? == noun).then_some(())?;
    let most: i32 = most.parse().ok()?;
    let what = r4704_phrase(noun, Form::All)?;
    Some(Ability::Spell {
        targets: Vec::new(),
        effects: vec![Effect::KeepTotalPower {
            what,
            most: Amount::Fixed(most),
        }],
    })
}

/// The words of a spell that keeps permanents within a total of power.
fn r4704_keep_power_texts(ability: &Ability) -> Option<String> {
    let Ability::Spell { targets, effects } = ability else {
        return None;
    };
    let [
        Effect::KeepTotalPower {
            what,
            most: Amount::Fixed(most),
        },
    ] = effects.as_slice()
    else {
        return None;
    };
    targets.is_empty().then_some(())?;
    r4704_keep_power_text(what, *most)
}

/// The words for the land a Fortification is attached to, and those of its twin, capitalized and not.
const R4704_FORTIFIED: [(&str, &str); 2] = [
    ("Fortified land", "Enchanted land"),
    ("fortified land", "enchanted land"),
];

/// The ability of a wording of this pack; `read` reads a clause of the same kind.
pub(super) fn r4704_read(
    text: &str,
    kind: ClauseKind,
    read: impl Fn(&str) -> Option<Ability>,
) -> Option<Ability> {
    if R4704_FORTIFIED
        .iter()
        .any(|(fortified, _)| text.contains(fortified))
    {
        let twin = R4704_FORTIFIED
            .iter()
            .fold(text.to_owned(), |text, (fortified, enchanted)| {
                text.replace(fortified, enchanted)
            });
        return read(&twin);
    }
    if kind == ClauseKind::Triggered {
        for (combat, head) in r4704_excess_heads() {
            if text.strip_prefix(head) == Some(R4704_EXCESS_HEAD) {
                return Some(r4704_excess_energy(combat));
            }
        }
        if let Some(found) = r4704_pile_trigger_read(text, &read) {
            return Some(found);
        }
    }
    if kind == ClauseKind::Spell
        && let Some(found) = r4704_pile_spell_read(text).or_else(|| r4704_keep_power_read(text))
    {
        return Some(found);
    }
    None
}

/// The wordings of the abilities of this pack.
pub(super) fn r4704_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    for (combat, head) in r4704_excess_heads() {
        if *ability == r4704_excess_energy(combat) {
            texts.push(format!("{head}{R4704_EXCESS_HEAD}"));
        }
    }
    let fortified: Vec<String> = texts
        .iter()
        .filter(|text| {
            R4704_FORTIFIED
                .iter()
                .any(|(_, enchanted)| text.contains(enchanted))
        })
        .map(|text| {
            R4704_FORTIFIED
                .iter()
                .fold(text.clone(), |text, (fortified, enchanted)| {
                    text.replace(enchanted, fortified)
                })
        })
        .collect();
    texts.extend(fortified);
    texts.extend(r4704_pile_spell_text(ability));
    texts.extend(r4704_keep_power_texts(ability));
    texts.extend(r4704_pile_trigger_texts(ability, written));
}
