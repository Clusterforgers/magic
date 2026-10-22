//! The top cards of the library and what a player takes "from among them": "Look at the top five cards of your
//! library. You may reveal a creature card from among them and put it into your hand. Put the rest on the bottom of
//! your library in a random order.", "Reveal the top four cards of your library. You may put a land card from among them
//! onto the battlefield tapped. Put the rest into your graveyard." and "Reveal the top four cards of your library. Put
//! all land cards revealed this way into your hand and the rest into your graveyard." Read from the wording and
//! written back the same way.

use super::library::{
    KEPT, TOP, filter, filter_text, rest_destination, rest_text, top_cards, top_text, what,
    what_text,
};
use super::tokens::Reader;
use crate::effect::{Destination, Effect};
use crate::refs::{Bounds, Pred, Ref, Selector, ZoneKind};

/// The cards of the library named `TOP` that match a filter.
fn among_top(filter: Selector) -> Selector {
    let mut preds = vec![Pred::In(ZoneKind::Library), Pred::Is(top())];
    preds.extend(filter.0);
    Selector(preds)
}

/// The filter of a selector made by `among_top`.
fn filter_of(selector: &Selector) -> Option<Selector> {
    match selector.0.as_slice() {
        [Pred::In(ZoneKind::Library), Pred::Is(Ref::Bound(name)), rest @ ..] if name == TOP => {
            Some(Selector(rest.to_vec()))
        }
        _ => None,
    }
}

/// The cards looked at.
fn top() -> Ref {
    Ref::Bound(TOP.to_owned())
}

/// Where the card taken goes: "into your hand", "onto the battlefield", "onto the battlefield tapped".
fn taken_destination(r: &mut Reader<'_>) -> Option<Destination> {
    if r.words("into your hand") {
        return Some(Destination::Hand);
    }
    r.need("onto the battlefield")?;
    Some(Destination::Battlefield {
        tapped: r.word("tapped"),
        under: None,
        counters: Vec::new(),
    })
}

/// "it" or "that card" for one card, "them" or "those cards" for several.
fn pronoun(r: &mut Reader<'_>, plural: bool) -> Option<()> {
    let found = if plural {
        r.word("them") || r.words("those cards")
    } else {
        r.word("it") || r.words("that card")
    };
    found.then_some(())
}

/// The cards a player may take at most: "a creature card" is up to one, "up to two" up to two.
fn optional(bounds: Bounds) -> Bounds {
    Bounds { min: 0, ..bounds }
}

/// "Look at the top five cards of your library. You may reveal an artifact card from among them and put it into your
/// hand. Put the rest on the bottom of your library in a random order.", and the other shapes of the module.
pub fn among(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let looked = r.words("look at");
    if !looked {
        r.need("reveal")?;
    }
    let count = top_cards(r)?;
    r.need("of your library")?;
    r.punct('.').then_some(())?;
    let mut effects = vec![Effect::LookAtTop {
        who: Ref::You,
        count,
        bind: TOP.to_owned(),
    }];
    if !looked {
        effects.push(Effect::Reveal { what: top() });
    }
    if r.words("put all") {
        let selector = among_top(filter(r, true)?);
        (r.words("revealed this way") || r.words("from among them")).then_some(())?;
        let to = taken_destination(r)?;
        r.need("and the rest")?;
        effects.push(Effect::Move {
            what: Ref::Each(selector),
            to,
        });
        effects.push(Effect::Move {
            what: top(),
            to: rest_destination(r)?,
        });
        return Some(effects);
    }
    let may = r.words("you may");
    let reveal = may && looked && r.word("reveal");
    if !reveal {
        r.need("put")?;
    }
    let (filter, bounds) = what(r)?;
    // "You may put a land card" is optional; "put up to two land cards" is optional by its count.
    (!may || reveal || bounds.is_one()).then_some(())?;
    r.need("from among them")?;
    let plural = !bounds.is_one();
    let bounds = if may { optional(bounds) } else { bounds };
    let to = if reveal {
        r.need("and put")?;
        pronoun(r, plural)?;
        r.need("into your hand")?;
        Destination::Hand
    } else {
        taken_destination(r)?
    };
    effects.push(Effect::Choose {
        who: Ref::You,
        from: Ref::Each(among_top(filter)),
        count: bounds,
        bind: KEPT.to_owned(),
        rest_as: None,
    });
    if reveal {
        effects.push(Effect::Reveal {
            what: Ref::Bound(KEPT.to_owned()),
        });
    }
    effects.push(Effect::Move {
        what: Ref::Bound(KEPT.to_owned()),
        to,
    });
    r.punct('.').then_some(())?;
    r.need("put the rest")?;
    effects.push(Effect::Move {
        what: top(),
        to: rest_destination(r)?,
    });
    Some(effects)
}

/// Where the card taken goes, as the card words it.
fn taken_text(to: &Destination) -> Option<String> {
    match to {
        Destination::Hand => Some("into your hand".to_owned()),
        Destination::Battlefield {
            tapped,
            under: None,
            counters,
        } if counters.is_empty() => Some(format!(
            "onto the battlefield{}",
            if *tapped { " tapped" } else { "" }
        )),
        _ => None,
    }
}

/// The sentences the effects at the front of the list are when they are one of the shapes of the module, and how many
/// effects they take.
pub fn sequence(effects: &[Effect]) -> Option<(String, usize)> {
    let [
        Effect::LookAtTop {
            who: Ref::You,
            count,
            bind,
        },
        tail @ ..,
    ] = effects
    else {
        return None;
    };
    (bind == TOP).then_some(())?;
    let (verb, tail, mut used) = match tail {
        [Effect::Reveal { what }, tail @ ..] if *what == top() => ("reveal", tail, 2),
        _ => ("look at", tail, 1),
    };
    let intro = format!("{verb} {} of your library", top_text(count)?);
    match tail {
        [
            Effect::Move {
                what: Ref::Each(selector),
                to,
            },
            Effect::Move {
                what: Ref::Bound(left),
                to: rest,
            },
            ..,
        ] if left == TOP => {
            let kind = filter_text(&filter_of(selector)?, true)?;
            used += 2;
            let among = if verb == "reveal" {
                "revealed this way"
            } else {
                "from among them"
            };
            Some((
                format!(
                    "{intro}. Put all {kind} {among} {} and the rest {}",
                    taken_text(to)?,
                    rest_text(rest)?
                ),
                used,
            ))
        }
        [
            Effect::Choose {
                who: Ref::You,
                from: Ref::Each(selector),
                count: bounds,
                bind,
                rest_as: None,
            },
            kept @ ..,
        ] if bind == KEPT => {
            let (revealed, kept) = match kept {
                [Effect::Reveal { what }, kept @ ..] if *what == Ref::Bound(KEPT.to_owned()) => {
                    (true, kept)
                }
                _ => (false, kept),
            };
            let [
                Effect::Move {
                    what: Ref::Bound(moved),
                    to,
                },
                Effect::Move {
                    what: Ref::Bound(left),
                    to: rest,
                },
                ..,
            ] = kept
            else {
                return None;
            };
            (moved == KEPT && left == TOP).then_some(())?;
            let filter = filter_of(selector)?;
            let one = Bounds::default();
            let may_one = optional(one.clone());
            let take = if revealed {
                let single = *bounds == may_one;
                let cards = what_text(&filter, if single { &one } else { bounds })?;
                (*to == Destination::Hand).then_some(())?;
                format!(
                    "You may reveal {cards} from among them and put {} into your hand",
                    if single { "it" } else { "them" }
                )
            } else if *bounds == may_one {
                let cards = what_text(&filter, &one)?;
                format!("You may put {cards} from among them {}", taken_text(to)?)
            } else {
                let cards = what_text(&filter, bounds)?;
                format!("Put {cards} from among them {}", taken_text(to)?)
            };
            used += 3 + usize::from(revealed);
            Some((
                format!("{intro}. {take}. Put the rest {}", rest_text(rest)?),
                used,
            ))
        }
        _ => None,
    }
}
