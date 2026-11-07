//! What a rule forbids (`GameRule::Cant`), written back: "This creature can't attack unless defending player controls an
//! Island.", "Skip your draw step.", "Players skip their upkeep steps.", "Each player can't cast more than one spell each
//! turn.", "Your opponents can't cast creature spells.", "Players can't play lands.", "This creature can't be the target
//! of black spells or abilities from black sources.", "You can't get poison counters.", "Creatures you control can't
//! have -1/-1 counters put on them.", "Activated abilities of artifacts can't be activated." and "Creatures entering the
//! battlefield don't cause abilities to trigger." No template reads them: only the proposal checker writes them.

use mtg_core::card::CardType;

use super::counters;
use super::render::capitalize;
use super::selectors::{Form, color_name as color_word, phrase_text};
use super::triggers::subject_text;
use crate::ability::ActivationKind;
use crate::modify::Prohibited;
use crate::refs::{Pred, Ref, Selector};
use crate::trigger::{Step, TriggerPattern};
use crate::value::{Amount, Cmp, Condition, CounterKind};

/// The end of the sentence of a prohibition on enter triggers.
const ENTERING: &str = "entering don't cause abilities to trigger";

/// The players a prohibition names, as the subject of "can't": "You", "Players", "Your opponents".
fn players(who: &Ref) -> Option<&'static str> {
    match who {
        Ref::You => Some("You"),
        Ref::EachPlayer => Some("Players"),
        Ref::Opponents => Some("Your opponents"),
        _ => None,
    }
}

/// The wordings of a prohibition with the condition it holds under, each without its period.
pub(super) fn r9w_cant_texts(
    who: &Ref,
    action: &Prohibited,
    condition: Option<&Condition>,
) -> Vec<String> {
    let text = match condition {
        None => {
            let mut texts: Vec<String> = plain(who, action).into_iter().collect();
            texts.extend(cast_passive(who, action));
            // The older wording of the same rule names the battlefield.
            if let Some(text) = texts.first().and_then(|text| text.strip_suffix(ENTERING)) {
                texts.push(format!(
                    "{text}entering the battlefield don't cause abilities to trigger"
                ));
            }
            return texts;
        }
        // "Your opponents can't cast spells during your turn."
        Some(Condition::YourTurn) => match (who, action) {
            (
                Ref::Opponents,
                Prohibited::Cast {
                    filter,
                    more_than: None,
                    unless: None,
                },
            ) if filter.0.is_empty() => {
                Some("Your opponents can't cast spells during your turn".to_owned())
            }
            _ => None,
        },
        Some(_) => None,
    };
    text.into_iter().collect()
}

/// A prohibition that always holds.
fn plain(who: &Ref, action: &Prohibited) -> Option<String> {
    match action {
        Prohibited::Attack { unless } if *who == Ref::This => attack(unless),
        Prohibited::SkipStep(step) => skip(who, *step),
        Prohibited::Cast {
            filter,
            more_than,
            unless: None,
        } => cast(who, filter, *more_than),
        Prohibited::PlayLand => Some(format!("{} can't play lands", players(who)?)),
        Prohibited::BeTargeted(sources) if *who == Ref::This => {
            Some(format!("~ can't be the target of {}", targeted(sources)?))
        }
        Prohibited::BeTargeted(sources) => Some(format!(
            "{} can't be the targets of {}",
            group(who)?,
            targeted(sources)?
        )),
        Prohibited::Transform => Some(format!("{} can't transform", group(who)?)),
        // Mana abilities stay open: "unless they're mana abilities".
        Prohibited::Activate(sources) if sources.0.is_empty() => {
            let Ref::Each(of) = who else {
                return None;
            };
            Some(format!(
                "Activated abilities of {} can't be activated unless they're mana abilities",
                sources_text(of)?
            ))
        }
        Prohibited::PutCounterOf(CounterKind::Poison) if *who == Ref::You => {
            Some("You can't get poison counters".to_owned())
        }
        Prohibited::PutCounterOf(kind) => {
            let group = group(who)?;
            Some(format!(
                "{group} can't have {} counters put on them",
                counters::kind_text(kind)?
            ))
        }
        Prohibited::PutCounter
            if *who
                == Ref::Each(Selector(vec![
                    Pred::This,
                    Pred::In(crate::refs::ZoneKind::Battlefield),
                ])) =>
        {
            Some("~ can't have counters put on it".to_owned())
        }
        Prohibited::ActivateKind { kind, sources } if *who == Ref::EachPlayer => {
            activate(*kind, sources)
        }
        Prohibited::Trigger(TriggerPattern::ZoneChange {
            what,
            from: None,
            to: Some(crate::refs::ZoneKind::Battlefield),
        }) if *who == Ref::EachPlayer && !what.0.is_empty() => Some(format!(
            "{} {ENTERING}",
            capitalize(&phrase_text(what, Form::All)?)
        )),
        _ => None,
    }
}

/// The other wordings of a prohibition on every player's spells: "Creature spells can't be cast.", "Each player who has
/// cast a nonartifact spell this turn can't cast additional nonartifact spells."
fn cast_passive(who: &Ref, action: &Prohibited) -> Vec<String> {
    let Prohibited::Cast {
        filter,
        more_than,
        unless: None,
    } = action
    else {
        return Vec::new();
    };
    if *who != Ref::EachPlayer {
        return Vec::new();
    }
    let text =
        match more_than {
            None if !filter.0.is_empty() => {
                spells(filter, false).map(|spells| format!("{} can't be cast", capitalize(&spells)))
            }
            Some(1) if !filter.0.is_empty() => spells(filter, true).zip(spells(filter, false)).map(
                |(one, many)| {
                    format!(
                        "Each player who has cast {} {one} this turn can't cast additional {many}",
                        super::triggers::article(&one)
                    )
                },
            ),
            _ => None,
        };
    text.into_iter().collect()
}

/// The permanents a prohibition names as a group: "Creatures you control".
fn group(who: &Ref) -> Option<String> {
    let Ref::Each(selector) = who else {
        return None;
    };
    Some(capitalize(&phrase_text(selector, Form::All)?))
}

/// "~ can't attack unless defending player controls an Island", "... is the monarch", "~ can't attack if defending
/// player controls an untapped land": the condition is about the defending player, "you" in the data.
fn attack(unless: &Condition) -> Option<String> {
    let (word, held) = match unless {
        Condition::Not(inner) => ("if", &**inner),
        other => ("unless", other),
    };
    let fact = match held {
        Condition::Monarch(Ref::You) if word == "unless" => "is the monarch".to_owned(),
        Condition::Compare {
            left: Amount::Count(of),
            op: Cmp::Ge,
            right: Amount::Fixed(1),
        } => {
            let yours = Pred::ControlledBy(Ref::You);
            let at = of.0.iter().position(|pred| *pred == yours)?;
            let mut rest = of.0.clone();
            rest.remove(at);
            (!rest.contains(&yours)).then_some(())?;
            format!("controls {}", subject_text(&Selector(rest))?)
        }
        _ => return None,
    };
    Some(format!("~ can't attack {word} defending player {fact}"))
}

/// "Skip your draw step.", "Players skip their untap steps."
fn skip(who: &Ref, step: Step) -> Option<String> {
    let name = match step {
        Step::Untap => "untap",
        Step::Upkeep => "upkeep",
        Step::Draw => "draw",
        _ => return None,
    };
    match who {
        Ref::You => Some(format!("Skip your {name} step")),
        Ref::EachPlayer => Some(format!("Players skip their {name} steps")),
        _ => None,
    }
}

/// "Your opponents can't cast creature spells.", "Each player can't cast more than one spell each turn.", "Players
/// can cast spells only during their own turns."
fn cast(who: &Ref, filter: &Selector, more_than: Option<u32>) -> Option<String> {
    if let Some(kind) = super::r9w_players::r9w_off_turn(who) {
        (kind == "player" && filter.0.is_empty() && more_than.is_none()).then_some(())?;
        return Some("Players can cast spells only during their own turns".to_owned());
    }
    let spells = spells(filter, more_than.is_some())?;
    match more_than {
        None => Some(format!("{} can't cast {spells}", players(who)?)),
        Some(1) => {
            let subject = match who {
                Ref::You => "You",
                Ref::EachPlayer => "Each player",
                _ => return None,
            };
            Some(format!(
                "{subject} can't cast more than one {spells} each turn"
            ))
        }
        Some(_) => None,
    }
}

/// The spells of a filter: "spells", "creature spells", "blue creature spells", "spells of the chosen color", "spell"
/// for one.
fn spells(filter: &Selector, one: bool) -> Option<String> {
    let noun = if one { "spell" } else { "spells" };
    match filter.0.as_slice() {
        [] => return Some(noun.to_owned()),
        [Pred::ChosenColor] if !one => return Some("spells of the chosen color".to_owned()),
        [Pred::ChosenName] if !one => return Some("spells with the chosen name".to_owned()),
        // A land is never cast, so a nonland name is the same rule.
        [Pred::ChosenName, Pred::Not(land)] if !one && **land == Pred::Type(CardType::Land) => {
            return Some("spells with the chosen name".to_owned());
        }
        _ => {}
    }
    // Only kinds of card: a zone, a controller or a fact of the game would read as something else.
    let kinds = filter.0.iter().all(|pred| match pred {
        Pred::Type(_) | Pred::Subtype(_) | Pred::Color(_) => true,
        Pred::Not(inner) => matches!(**inner, Pred::Type(_) | Pred::Subtype(_) | Pred::Color(_)),
        _ => false,
    });
    kinds.then_some(())?;
    let mut preds = vec![Pred::Spell];
    preds.extend(filter.0.iter().cloned());
    phrase_text(&Selector(preds), if one { Form::Each } else { Form::All })
}

/// What a permanent can't be the target of: "black spells or abilities from black sources", "nongreen spells your
/// opponents control or abilities from nongreen sources your opponents control", "Aura spells", "blue or black
/// spells", "spells".
fn targeted(sources: &Selector) -> Option<String> {
    let color = |pred: &Pred| match pred {
        Pred::Color(color) => Some(color_word(*color).to_owned()),
        Pred::Not(inner) => match **inner {
            Pred::Color(color) => Some(format!("non{}", color_word(color))),
            _ => None,
        },
        _ => None,
    };
    Some(match sources.0.as_slice() {
        [Pred::Spell] => "spells".to_owned(),
        [kind] => {
            let kind = color(kind)?;
            format!("{kind} spells or abilities from {kind} sources")
        }
        [kind, Pred::ControlledBy(Ref::Opponents)] => {
            let kind = color(kind)?;
            format!(
                "{kind} spells your opponents control or abilities from {kind} sources your opponents control"
            )
        }
        [kind, Pred::Spell] => format!("{} spells", kind_words(kind)?),
        _ => return None,
    })
}

/// The sources of activated abilities: "sources with the chosen name", "artifacts and creatures".
fn sources_text(sources: &Selector) -> Option<String> {
    if sources.0 == [Pred::ChosenName] {
        return Some("sources with the chosen name".to_owned());
    }
    phrase_text(sources, Form::All)
}

/// A color or a subtype of spell as one word, or colors joined by "or": "black", "Aura", "blue or black".
fn kind_words(pred: &Pred) -> Option<String> {
    match pred {
        Pred::Color(color) => Some(color_word(*color).to_owned()),
        Pred::Subtype(subtype) => Some(subtype.0.clone()),
        Pred::AnyOf(parts) => {
            let colors: Option<Vec<&str>> = parts
                .iter()
                .map(|part| match part {
                    Pred::Color(color) => Some(color_word(*color)),
                    _ => None,
                })
                .collect();
            let colors = colors?;
            (colors.len() == 2).then(|| colors.join(" or "))
        }
        _ => None,
    }
}

/// "Players can't cycle cards.", "Players can't activate planeswalkers' loyalty abilities.", "Activated abilities of
/// artifacts can't be activated."
fn activate(kind: ActivationKind, sources: &Selector) -> Option<String> {
    match kind {
        ActivationKind::Cycling if sources.0.is_empty() => {
            Some("Players can't cycle cards".to_owned())
        }
        ActivationKind::Loyalty if sources.0 == [Pred::Type(CardType::Planeswalker)] => {
            Some("Players can't activate planeswalkers' loyalty abilities".to_owned())
        }
        // Every ability, mana abilities included, as the card prints it without an exception.
        ActivationKind::Every
            if sources.0 == [Pred::Type(CardType::Creature), Pred::Is(Ref::Attached)] =>
        {
            Some("Enchanted creature's activated abilities can't be activated".to_owned())
        }
        ActivationKind::Every if !sources.0.is_empty() => Some(format!(
            "Activated abilities of {} can't be activated",
            sources_text(sources)?
        )),
        _ => None,
    }
}
