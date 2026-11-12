//! Keywords of the cards exiled with a permanent: "As long as a card exiled with this creature has flying, this creature
//! has flying. The same is true for fear, first strike, ..." and "As long as a card exiled with ~ has flying, equipped
//! creature has flying. The same is true for ...". Written from the data.

use crate::ability::Ability;
use crate::modify::Modification;
use crate::refs::{Pred, Ref, Selector, ZoneKind};

/// Adds the wordings of a static ability of an attachment: a Fortification's "fortified land" where an Equipment says
/// "equipped creature", and "Enchanted creature is a Flagbearer." for a creature type an Aura adds.
pub(super) fn r373_attached_wordings(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Static {
        affected: Ref::Attached,
        mods,
        condition: None,
        from: ZoneKind::Battlefield,
        ..
    } = ability
    else {
        return;
    };
    let fortified: Vec<String> = texts
        .iter()
        .filter_map(|text| text.strip_prefix("Equipped creature "))
        .map(|rest| format!("Fortified land {rest}"))
        .collect();
    for text in fortified {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
    if let [Modification::AddSubtype(subtype)] = mods.as_slice()
        && subtype.is_creature_type()
    {
        let article = if subtype.0.starts_with(['A', 'E', 'I', 'O', 'U']) {
            "an"
        } else {
            "a"
        };
        texts.push(format!("Enchanted creature is {article} {}.", subtype.0));
    }
}

/// Adds the wording of a static ability that gives the keywords of the cards exiled with its source.
pub(super) fn r373_exiled_wordings(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Static {
        affected,
        mods,
        condition: None,
        from: ZoneKind::Battlefield,
        ..
    } = ability
    else {
        return;
    };
    let [Modification::KeywordsOf { from, names }] = mods.as_slice() else {
        return;
    };
    let exiled_with_this = Ref::Each(Selector(vec![
        Pred::In(ZoneKind::Exile),
        Pred::ExiledWith(Ref::This),
    ]));
    if *from != exiled_with_this {
        return;
    }
    let subject = match affected {
        Ref::This => "~",
        Ref::Attached => "equipped creature",
        _ => return,
    };
    let Some((first, rest)) = names.split_first() else {
        return;
    };
    let first = first.to_lowercase();
    let rest: Vec<String> = rest.iter().map(|name| name.to_lowercase()).collect();
    let mut sentence =
        format!("As long as a card exiled with ~ has {first}, {subject} has {first}.");
    if !rest.is_empty() {
        let listed = match rest.as_slice() {
            [one] => one.clone(),
            [one, two] => format!("{one} and {two}"),
            [init @ .., last] => format!("{}, and {last}", init.join(", ")),
            [] => return,
        };
        sentence.push_str(&format!(" The same is true for {listed}."));
    }
    texts.push(sentence);
}
