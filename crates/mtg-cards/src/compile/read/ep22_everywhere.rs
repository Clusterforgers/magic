//! "Creatures you control are artifacts in addition to their other types. The same is true for creature spells you control
//! and creature cards you own that aren't on the battlefield.": a static ability over every zone (an `AnyOf` of each zone),
//! written as the sentence about the permanents and the sentence that extends it.

use crate::ability::Ability;
use crate::refs::{Pred, Ref, Selector, ZoneKind};

/// Every zone of a game, as the selector of a static ability over all of them lists it.
pub const EVERY_ZONE: [ZoneKind; 7] = [
    ZoneKind::Battlefield,
    ZoneKind::Stack,
    ZoneKind::Hand,
    ZoneKind::Library,
    ZoneKind::Graveyard,
    ZoneKind::Exile,
    ZoneKind::Command,
];

/// Adds the other printed wordings of ep22's writers to `texts`: "Put an additional counter of that kind on that
/// permanent" for "Give that permanent another counter of that kind", "For each flip you win, S" for "S for each coin
/// that comes up heads", a static ability over every zone, and "This effect reduces only the amount of colored mana you
/// pay." after a reduction of coloured symbols alone.
pub(super) fn ep22_variants(ability: &Ability, texts: &mut Vec<String>) {
    let additional: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let (head, rest) = text.split_once(". Give that ")?;
            let (noun, tail) = rest.split_once(" another counter of that kind")?;
            Some(format!(
                "{head}. Put an additional counter of that kind on that {noun}{tail}"
            ))
        })
        .collect();
    texts.extend(additional);
    let won: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let (before, tail) = text.split_once(" for each coin that comes up heads")?;
            let (head, steps) = before.rsplit_once(". ")?;
            let mut chars = steps.chars();
            let steps = chars
                .next()?
                .to_lowercase()
                .chain(chars)
                .collect::<String>();
            Some(format!("{head}. For each flip you win, {steps}{tail}"))
        })
        .collect();
    texts.extend(won);
    texts.extend(self::texts(ability));
    if let Ability::CostModifier {
        change: crate::ability::CostChange::Reduce,
        mana,
        ..
    } = ability
        && !mana.has_x()
        && mana.rest().generic() == 0
        && !mana.rest().symbols().is_empty()
    {
        let colored: Vec<String> = texts
            .iter()
            .map(|text| {
                let text = text.strip_suffix('.').unwrap_or(text);
                format!("{text}. This effect reduces only the amount of colored mana you pay.")
            })
            .collect();
        texts.extend(colored);
    }
}

/// The wordings of a static ability of one card type you control in every zone; empty for any other ability.
pub(super) fn texts(ability: &Ability) -> Vec<String> {
    let Ability::Static {
        affected: Ref::Each(selector),
        mods,
        condition,
        defining,
        from,
    } = ability
    else {
        return Vec::new();
    };
    let every = |pred: &Pred| {
        matches!(pred, Pred::AnyOf(list) if list.len() == EVERY_ZONE.len()
            && EVERY_ZONE.iter().all(|zone| list.contains(&Pred::In(*zone))))
    };
    if !selector.0.iter().any(every) {
        return Vec::new();
    }
    let rest: Vec<Pred> = selector.0.iter().filter(|p| !every(p)).cloned().collect();
    let noun = match rest.as_slice() {
        [Pred::Type(kind), Pred::ControlledBy(Ref::You)]
        | [Pred::ControlledBy(Ref::You), Pred::Type(kind)] => format!("{kind:?}").to_lowercase(),
        _ => return Vec::new(),
    };
    let plain = Ability::Static {
        affected: Ref::Each(Selector(rest)),
        mods: mods.clone(),
        condition: condition.clone(),
        defining: *defining,
        from: *from,
    };
    let written = super::render::written_variants(&plain);
    // "Lands you control and land cards you own that aren't on the battlefield are Deserts": a land is never a spell, so
    // the land cards outside the battlefield are every other zone.
    let lands: Vec<String> = written
        .iter()
        .filter(|_| noun == "land")
        .filter_map(|text| {
            let rest = text.strip_prefix("Lands you control ")?;
            Some(format!(
                "Lands you control and land cards you own that aren't on the battlefield {rest}"
            ))
        })
        .collect();
    written
        .into_iter()
        .map(|text| {
            let text = text.strip_suffix('.').unwrap_or(&text);
            format!(
                "{text}. The same is true for {noun} spells you control and {noun} cards you own that aren't on the battlefield."
            )
        })
        .chain(lands)
        .collect()
}
