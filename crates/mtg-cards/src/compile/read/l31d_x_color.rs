//! "Spend only black mana on X." (`CostItem::XColor`): after the effect of an activated ability whose cost holds `{X}`,
//! a part of that cost; as a line of its own on a spell, a required additional cost that pays nothing. Its readers and
//! writers.

use mtg_core::color::Color;

use crate::ability::Ability;
use crate::compile::classify::ClauseKind;
use crate::cost::{Cost, CostItem};

/// The colors and their words.
const COLORS: [(Color, &str); 5] = [
    (Color::White, "white"),
    (Color::Blue, "blue"),
    (Color::Black, "black"),
    (Color::Red, "red"),
    (Color::Green, "green"),
];

/// "Spend only black mana on X."
fn sentence(color: Color) -> Option<String> {
    let (_, word) = COLORS.iter().find(|(c, _)| *c == color)?;
    Some(format!("Spend only {word} mana on X."))
}

/// The text before the sentence that ends `text` and the color it names.
fn split(text: &str) -> Option<(&str, Color)> {
    let text = text.trim_end();
    COLORS.iter().find_map(|(color, word)| {
        let head = text.strip_suffix(&format!("Spend only {word} mana on X."))?;
        Some((head.trim_end(), *color))
    })
}

/// The color of X a cost names, if any.
fn x_color(cost: &Cost) -> Option<Color> {
    cost.0.iter().find_map(|item| match item {
        CostItem::XColor(color) => Some(*color),
        _ => None,
    })
}

/// The ability `text` says with "Spend only black mana on X." at its end: the spell line alone, or an activated ability
/// read from the text before it (`read`) whose cost holds `{X}`.
pub(super) fn l31d_x_color_read(
    text: &str,
    kind: ClauseKind,
    read: impl Fn(&str) -> Option<Ability>,
) -> Option<Ability> {
    let (head, color) = split(text)?;
    match kind {
        ClauseKind::Spell | ClauseKind::Static if head.is_empty() => {
            Some(Ability::AdditionalCost {
                cost: Cost(vec![CostItem::XColor(color)]),
                required: true,
                repeatable: false,
            })
        }
        ClauseKind::Activated if !head.is_empty() => {
            let mut ability = read(head)?;
            let Ability::Activated { cost, .. } = &mut ability else {
                return None;
            };
            (cost.has_x() && x_color(cost).is_none()).then_some(())?;
            cost.0.push(CostItem::XColor(color));
            Some(ability)
        }
        _ => None,
    }
}

/// The words of an ability that colors its X: the spell line, or the activated ability written without the part and the
/// sentence after it; `None` for any other ability.
pub(super) fn l31d_x_color_variants(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
) -> Option<Vec<String>> {
    match ability {
        Ability::AdditionalCost {
            cost,
            required: true,
            repeatable: false,
        } => match cost.0.as_slice() {
            [CostItem::XColor(color)] => Some(sentence(*color).into_iter().collect()),
            _ => None,
        },
        Ability::Activated { cost, .. } => {
            let line = sentence(x_color(cost)?)?;
            let mut twin = ability.clone();
            if let Ability::Activated { cost, .. } = &mut twin {
                cost.0.retain(|item| !matches!(item, CostItem::XColor(_)));
            }
            Some(
                written(&twin)
                    .into_iter()
                    .filter(|text| text.contains("{X}"))
                    .map(|text| format!("{text} {line}"))
                    .collect(),
            )
        }
        _ => None,
    }
}
