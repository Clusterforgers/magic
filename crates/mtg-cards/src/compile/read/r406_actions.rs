//! Replacements of a keyword action ("If a creature you control would explore, instead it explores, then it explores
//! again.", "If you would proliferate, proliferate twice instead."): read by trying the step lists the writer words.

use super::amounts::count_word;
use super::triggers::subject_text;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::r406_actions::r406_is_replaced;
use crate::refs::{Ref, Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};
use crate::value::Amount;

/// The wordings of a replacement of a keyword action, each without its capital and closing period.
pub(super) fn r406_texts(when: &ReplaceWhen, by: &ReplaceBy) -> Vec<String> {
    let ReplaceBy::Instead(steps) = by else {
        return Vec::new();
    };
    let (head, verb, again) = match when {
        ReplaceWhen::Explores { what } => match subject_text(what) {
            Some(subject) => (
                format!("if {subject} would explore"),
                "explores",
                "that creature",
            ),
            None => return Vec::new(),
        },
        ReplaceWhen::Connives { what } => match subject_text(what) {
            Some(subject) => (
                format!("if {subject} would connive"),
                "connives",
                "that creature",
            ),
            None => return Vec::new(),
        },
        ReplaceWhen::Proliferates { who: Ref::You } => {
            ("if you would proliferate".to_owned(), "proliferate", "you")
        }
        _ => return Vec::new(),
    };
    let replaced = |step: &Effect| r406_is_replaced(when, step);
    let mut found = Vec::new();
    if let [first, second] = steps.as_slice()
        && replaced(first)
        && replaced(second)
    {
        if matches!(when, ReplaceWhen::Proliferates { .. }) {
            found.push(format!("{head}, proliferate twice instead"));
        } else {
            found.push(format!("{head}, instead it {verb}, then it {verb} again"));
            found.push(format!("{head}, it {verb} twice instead"));
        }
        return found;
    }
    let mut words = Vec::new();
    for step in steps {
        if replaced(step) {
            words.push(format!("{again} {verb}"));
        } else {
            match around(step) {
                Some(text) => words.push(text),
                None => return Vec::new(),
            }
        }
    }
    found.push(format!("{head}, instead {}", words.join(", then ")));
    found
}

/// A step around the action, said by the controller of the replacement: "you scry 1", "you draw a card".
fn around(step: &Effect) -> Option<String> {
    let count = |amount: &Amount| match amount {
        Amount::Fixed(number) => u32::try_from(*number).ok(),
        _ => None,
    };
    Some(match step {
        Effect::Scry {
            who: Ref::You,
            count: n,
        } => format!("you scry {}", count(n)?),
        Effect::Surveil {
            who: Ref::You,
            count: n,
        } => format!("you surveil {}", count(n)?),
        Effect::Draw {
            who: Ref::You,
            count: n,
            bind: None,
        } => match count(n)? {
            1 => "you draw a card".to_owned(),
            many => format!("you draw {} cards", count_word(many)?),
        },
        _ => return None,
    })
}

/// The replacement of a keyword action the text words, among the step lists the writer knows.
pub(super) fn r406_read(text: &str) -> Option<Ability> {
    let lowered = text.strip_prefix("If ")?;
    let (subject, rest) = lowered.split_once(" would ")?;
    let when = if rest.starts_with("explore,") || rest.starts_with("connive,") {
        let tokens = super::tokens::tokenize(subject);
        let mut r = super::tokens::Reader::new(&tokens, false);
        let what: Selector = super::triggers::subject(&mut r)?;
        r.end().then_some(())?;
        if rest.starts_with("explore,") {
            ReplaceWhen::Explores { what }
        } else {
            ReplaceWhen::Connives { what }
        }
    } else if subject == "you" && rest.starts_with("proliferate,") {
        ReplaceWhen::Proliferates { who: Ref::You }
    } else {
        return None;
    };
    let action = match &when {
        ReplaceWhen::Explores { .. } => Effect::Explore {
            what: Ref::Triggering,
        },
        ReplaceWhen::Connives { .. } => Effect::Connive {
            what: Ref::Triggering,
            count: Amount::one(),
        },
        _ => Effect::Proliferate { who: Ref::You },
    };
    let mut lists = vec![vec![action.clone(), action.clone()]];
    for n in 1..=3_i32 {
        let fixed = Amount::Fixed(n);
        for before in [
            Effect::Scry {
                who: Ref::You,
                count: fixed.clone(),
            },
            Effect::Surveil {
                who: Ref::You,
                count: fixed.clone(),
            },
            Effect::Draw {
                who: Ref::You,
                count: fixed.clone(),
                bind: None,
            },
        ] {
            lists.push(vec![before, action.clone()]);
        }
    }
    lists.into_iter().find_map(|steps| {
        let ability = Ability::Replacement {
            when: when.clone(),
            by: ReplaceBy::Instead(steps),
            condition: None,
            from: ZoneKind::Battlefield,
        };
        super::render::variants(&ability)
            .iter()
            .any(|written| super::tokens::same_text(written, text))
            .then_some(ability)
    })
}

/// The two prohibitions of "Players can't cast spells from graveyards or activate abilities of cards in graveyards.",
/// in the order the sentence names them.
fn r406_graveyard_bans() -> [Ability; 2] {
    use crate::modify::{GameRule, Prohibited};
    use crate::refs::Pred;
    let in_graveyard = || Selector(vec![Pred::In(ZoneKind::Graveyard)]);
    let ban = |action| Ability::Rule {
        rule: GameRule::Cant {
            who: Ref::EachPlayer,
            action,
        },
        condition: None,
        from: ZoneKind::Battlefield,
    };
    [
        ban(Prohibited::Cast {
            filter: in_graveyard(),
            more_than: None,
            unless: None,
        }),
        ban(Prohibited::Activate(in_graveyard())),
    ]
}

/// The wording of two prohibitions that read as one sentence, with a capital and a period.
pub(super) fn r406_pair_texts(first: &Ability, second: &Ability) -> Vec<String> {
    let [cast, activate] = r406_graveyard_bans();
    if *first == cast && *second == activate {
        return vec![
            "Players can't cast spells from graveyards or activate abilities of cards in graveyards."
                .to_owned(),
        ];
    }
    Vec::new()
}

/// The two prohibitions the sentence words, read whole.
pub(super) fn r406_pair_read(text: &str) -> Option<Vec<Ability>> {
    super::tokens::same_text(
        text,
        "Players can't cast spells from graveyards or activate abilities of cards in graveyards.",
    )
    .then(|| r406_graveyard_bans().into())
}
