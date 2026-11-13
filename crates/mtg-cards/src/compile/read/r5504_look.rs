//! r55-04 choices made after a look as a permanent enters: "As this artifact enters, look at an opponent's hand, then
//! choose any card name." (`ReplaceBy::ChooseAfter`), read and written as one sentence.

use super::tokens::same_text;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Ref, ZoneKind};
use crate::trigger::{EntersChoice, ReplaceBy, ReplaceWhen};

/// The name the opponent whose hand is looked at is bound to.
const OPPONENT: &str = "opponent";

/// The steps of "look at an opponent's hand": an opponent chosen, then their hand looked at.
fn look_at_an_opponents_hand() -> Vec<Effect> {
    vec![
        Effect::Choose {
            who: Ref::You,
            from: Ref::Opponents,
            count: crate::refs::Bounds::default(),
            bind: OPPONENT.to_owned(),
            rest_as: None,
        },
        Effect::LookAtHand {
            of: Ref::Bound(OPPONENT.to_owned()),
        },
    ]
}

/// The words of a choice made after the look, each wording.
fn choice_words(choice: EntersChoice) -> Vec<String> {
    match choice {
        EntersChoice::CardName => vec!["any card name".to_owned(), "a card name".to_owned()],
        EntersChoice::NonlandCardName => vec!["a nonland card name".to_owned()],
        EntersChoice::CardType | EntersChoice::CardTypeOf(_) => {
            super::e15b_entering::choose_type_texts(choice)
                .into_iter()
                .filter_map(|text| text.strip_prefix("As ~ enters, choose ").map(str::to_owned))
                .collect()
        }
        _ => Vec::new(),
    }
}

/// "As ~ enters, look at an opponent's hand, then choose any card name": the wordings of the shapes written.
pub(super) fn r5504_texts(first: &[Effect], choice: EntersChoice) -> Vec<String> {
    if first != look_at_an_opponents_hand().as_slice() {
        return Vec::new();
    }
    choice_words(choice)
        .into_iter()
        .map(|what| format!("As ~ enters, look at an opponent's hand, then choose {what}"))
        .collect()
}

/// The choices a look may come before, in the order they are tried.
const CHOICES: [EntersChoice; 3] = [
    EntersChoice::CardName,
    EntersChoice::NonlandCardName,
    EntersChoice::CardType,
];

/// The replacement "As ~ enters, look at an opponent's hand, then choose any card name." reads as; `text` has `~` for
/// the card's own name.
pub(super) fn r5504_read(text: &str) -> Option<Ability> {
    let rest = text.strip_prefix("As ~ enters, look at an opponent's hand, then choose ")?;
    let choice = CHOICES.into_iter().find(|choice| {
        choice_words(*choice)
            .iter()
            .any(|what| same_text(rest, &format!("{what}.")) || same_text(rest, what))
    })?;
    Some(Ability::Replacement {
        when: ReplaceWhen::Enters {
            what: crate::refs::Selector::this(),
        },
        by: ReplaceBy::ChooseAfter {
            first: look_at_an_opponents_hand(),
            choice,
        },
        condition: None,
        from: ZoneKind::Battlefield,
    })
}

/// The sentences of the cost increases of a card name chosen as the permanent entered: "Spells your opponents cast with
/// the chosen name cost {2} more to cast." and "Activated abilities of sources with the chosen name cost {2} more to
/// activate unless they're mana abilities."
pub(super) fn r5504_wordings(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::CostModifier {
        applies_to,
        change: crate::ability::CostChange::Increase,
        mana,
        per,
        condition: None,
        activating,
        targeting: None,
        least: 0,
        life: 0,
    } = ability
    else {
        return;
    };
    if !per.is_one() {
        return;
    }
    let spell = [
        crate::refs::Pred::Spell,
        crate::refs::Pred::ControlledBy(Ref::Opponents),
        crate::refs::Pred::ChosenName,
    ];
    let text = match activating {
        None if applies_to.0 == spell => {
            format!("Spells your opponents cast with the chosen name cost {mana} more to cast.")
        }
        Some(crate::ability::ActivationKind::Any)
            if applies_to.0 == [crate::refs::Pred::ChosenName] =>
        {
            format!(
                "Activated abilities of sources with the chosen name cost {mana} more to activate unless they're mana \
                 abilities."
            )
        }
        _ => return,
    };
    if !texts.contains(&text) {
        texts.push(text);
    }
}
