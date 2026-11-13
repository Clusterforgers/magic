//! r55-10 wordings of a keyword cost a static ability gives with the cost read off each card it reaches: "Each instant
//! and sorcery spell you cast has replicate. The replicate cost is equal to its mana cost.", "Each enchantment card in
//! your hand has miracle. Its miracle cost is equal to its mana cost reduced by {4}." The subject is read through a twin
//! that gives flying in place of the keyword; the cost is `OwnManaCost` or `OwnManaCostLess`.

use crate::ability::Ability;
use crate::cost::{Cost, CostItem};
use crate::keyword::Keyword;
use crate::modify::Modification;

/// The keywords with a cost a static ability gives this way, by their words.
const R5510_KEYWORDS: [&str; 6] = [
    "replicate",
    "emerge",
    "miracle",
    "foretell",
    "escape",
    "flashback",
];

/// The keyword of that name with the cost.
fn r5510_keyword(name: &str, cost: Cost) -> Option<Keyword> {
    Some(match name {
        "replicate" => Keyword::Replicate(cost),
        "emerge" => Keyword::Emerge(cost),
        "miracle" => Keyword::Miracle(cost),
        "foretell" => Keyword::Foretell(cost),
        "escape" => Keyword::Escape(cost),
        "flashback" => Keyword::Flashback(cost),
        _ => return None,
    })
}

/// The keyword of that name with the cost {9}, the twin a printed cost of its own words.
fn r5510_nine(name: &str) -> Option<Keyword> {
    let nine = Cost::mana("{9}".parse().ok()?);
    r5510_keyword(name, nine)
}

/// The name and cost of such a keyword.
fn r5510_parts(keyword: &Keyword) -> Option<(&'static str, &Cost)> {
    match keyword {
        Keyword::Replicate(cost) => Some(("replicate", cost)),
        Keyword::Emerge(cost) => Some(("emerge", cost)),
        Keyword::Miracle(cost) => Some(("miracle", cost)),
        Keyword::Foretell(cost) => Some(("foretell", cost)),
        Keyword::Escape(cost) => Some(("escape", cost)),
        Keyword::Flashback(cost) => Some(("flashback", cost)),
        _ => None,
    }
}

/// The sentence after the grant, for a cost read off the card: "The replicate cost is equal to its mana cost", "Its
/// foretell cost is equal to its mana cost reduced by {2}".
fn r5510_cost_sentences(name: &str, cost: &Cost) -> Vec<String> {
    let tail = match cost.0.as_slice() {
        [CostItem::OwnManaCost] => "its mana cost".to_owned(),
        [CostItem::OwnManaCostLess(less)] if *less > 0 => {
            format!("its mana cost reduced by {{{less}}}")
        }
        _ => return Vec::new(),
    };
    vec![
        format!("The {name} cost is equal to {tail}"),
        format!("Its {name} cost is equal to {tail}"),
    ]
}

/// Reads "Colorless spells you cast from your hand with mana value 7 or greater have "Cascade, cascade."": the twin
/// that gives cascade once, then the keyword twice.
pub(super) fn r5510_read_twice(
    text: &str,
    read: impl Fn(&str) -> Option<Ability>,
) -> Option<Ability> {
    let text = text.trim().trim_end_matches('.');
    let head = text.strip_suffix(" have \"Cascade, cascade.\"")?;
    let twin = read(&format!("{head} have cascade"))?;
    let Ability::Static {
        affected,
        mut mods,
        condition,
        from,
        defining,
    } = twin
    else {
        return None;
    };
    (mods == [Modification::AddKeyword(Keyword::Cascade)]).then_some(())?;
    mods.push(Modification::AddKeyword(Keyword::Cascade));
    Some(Ability::Static {
        affected,
        mods,
        condition,
        from,
        defining,
    })
}

/// Adds the wording of a static ability that gives spells cascade twice, from its twin that gives it once.
pub(super) fn r5510_wordings_twice(
    ability: &Ability,
    written: impl Fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Ability::Static {
        affected,
        mods,
        condition,
        from,
        defining,
    } = ability
    else {
        return;
    };
    let cascade = Modification::AddKeyword(Keyword::Cascade);
    if mods.as_slice() != [cascade.clone(), cascade.clone()] {
        return;
    }
    let twin = Ability::Static {
        affected: affected.clone(),
        mods: vec![cascade],
        condition: condition.clone(),
        from: *from,
        defining: *defining,
    };
    for once in written(&twin) {
        if let Some(head) = once.trim_end_matches('.').strip_suffix(" have cascade") {
            let text = format!("{head} have \"Cascade, cascade.\"");
            if !texts.contains(&text) {
                texts.push(text);
            }
        }
    }
}

/// Reads the grant: the subject through a twin that gives flying, then the keyword with its cost read off the card.
pub(super) fn r5510_read(text: &str, read: impl Fn(&str) -> Option<Ability>) -> Option<Ability> {
    let text = text.trim().trim_end_matches('.');
    let (grant, sentence) = text.split_once(". ")?;
    let (subject, name) = grant.rsplit_once(" has ")?;
    let name = name.trim();
    R5510_KEYWORDS.contains(&name).then_some(())?;
    let rest = sentence
        .strip_prefix(&format!("The {name} cost is equal to its mana cost"))
        .or_else(|| sentence.strip_prefix(&format!("Its {name} cost is equal to its mana cost")))?;
    let item = if rest.is_empty() {
        CostItem::OwnManaCost
    } else {
        let number = rest.strip_prefix(" reduced by {")?.strip_suffix('}')?;
        CostItem::OwnManaCostLess(number.parse().ok().filter(|n| *n > 0)?)
    };
    let nine = r5510_nine(name)?;
    let (affected, condition, from, defining) = [
        (format!("{subject} has flying"), Keyword::Flying),
        (format!("{subject} has {name} {{9}}"), nine),
    ]
    .into_iter()
    .find_map(|(twin, given)| match read(&twin)? {
        Ability::Static {
            affected,
            mods,
            condition,
            from,
            defining,
        } if mods == [Modification::AddKeyword(given.clone())] => {
            Some((affected, condition, from, defining))
        }
        _ => None,
    })?;
    let keyword = r5510_keyword(name, Cost(vec![item]))?;
    Some(Ability::Static {
        affected,
        mods: vec![Modification::AddKeyword(keyword)],
        condition,
        from,
        defining,
    })
}

/// Adds the wordings of such a grant: the twin's subjects with the keyword and the sentence of its cost.
pub(super) fn r5510_wordings(
    ability: &Ability,
    written: impl Fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Ability::Static {
        affected,
        mods,
        condition,
        from,
        defining,
    } = ability
    else {
        return;
    };
    let [Modification::AddKeyword(keyword)] = mods.as_slice() else {
        return;
    };
    let Some((name, cost)) = r5510_parts(keyword) else {
        return;
    };
    let sentences = r5510_cost_sentences(name, cost);
    if sentences.is_empty() {
        return;
    }
    let Some(nine) = r5510_nine(name) else {
        return;
    };
    for (given, suffix) in [
        (Keyword::Flying, " has flying".to_owned()),
        (nine, format!(" has {name} {{9}}")),
    ] {
        let twin = Ability::Static {
            affected: affected.clone(),
            mods: vec![Modification::AddKeyword(given)],
            condition: condition.clone(),
            from: *from,
            defining: *defining,
        };
        for grant in written(&twin) {
            let Some(subject) = grant.trim_end_matches('.').strip_suffix(suffix.as_str()) else {
                continue;
            };
            for sentence in &sentences {
                let text = format!("{subject} has {name}. {sentence}");
                if !texts.contains(&text) {
                    texts.push(text);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a keyword whose cost is read off each card reads and writes back with its subject and the sentence of its
    //! cost, and a near miss (another keyword in the sentence, a cost of no number, a keyword without a cost) is refused.
    use super::{r5510_read, r5510_read_twice, r5510_wordings, r5510_wordings_twice};
    use crate::ability::Ability;
    use crate::compile::written_variants;
    use rstest::rstest;

    fn read(text: &str) -> Option<Ability> {
        r5510_read(text, |twin| {
            crate::compile::read::clause_with(
                twin,
                crate::compile::ClauseKind::Static,
                false,
                &crate::compile::read::Own::default(),
            )
        })
    }

    #[rstest]
    #[case::replicate(
        "Each instant and sorcery spell you cast has replicate. The replicate cost is equal to its mana cost."
    )]
    #[case::sliver_replicate(
        "Each Sliver spell you cast has replicate. The replicate cost is equal to its mana cost."
    )]
    #[case::emerge(
        "Each creature spell you cast has emerge. The emerge cost is equal to its mana cost."
    )]
    #[case::miracle(
        "Each enchantment card in your hand has miracle. Its miracle cost is equal to its mana cost reduced by {4}."
    )]
    fn r5510_reads_and_writes_back(#[case] text: &str) {
        let ability = read(text).unwrap_or_else(|| panic!("reads {text}"));
        let mut texts = Vec::new();
        r5510_wordings(&ability, written_variants, &mut texts);
        let wanted = text.trim_end_matches('.');
        assert!(texts.iter().any(|t| t == wanted), "{texts:?}");
    }

    #[rstest]
    #[case::other_keyword_in_sentence(
        "Each creature spell you cast has emerge. The replicate cost is equal to its mana cost."
    )]
    #[case::no_number(
        "Each enchantment card in your hand has miracle. Its miracle cost is equal to its mana cost reduced by {0}."
    )]
    #[case::keyword_without_cost(
        "Each creature spell you cast has flying. The flying cost is equal to its mana cost."
    )]
    #[case::other_cost(
        "Each creature spell you cast has emerge. The emerge cost is equal to its mana value."
    )]
    fn r5510_near_misses_are_refused(#[case] text: &str) {
        assert_eq!(read(text), None);
    }

    #[rstest]
    #[case::twice(
        "Colorless spells you cast from your hand with mana value 7 or greater have \"Cascade, cascade.\"",
        true
    )]
    #[case::once(
        "Colorless spells you cast from your hand with mana value 7 or greater have cascade.",
        false
    )]
    #[case::other_keywords(
        "Colorless spells you cast from your hand with mana value 7 or greater have \"Flying, cascade.\"",
        false
    )]
    fn r5510_cascade_twice(#[case] text: &str, #[case] reads: bool) {
        let found = r5510_read_twice(text, |twin| {
            crate::compile::read::clause_with(
                twin,
                crate::compile::ClauseKind::Static,
                false,
                &crate::compile::read::Own::default(),
            )
        });
        assert_eq!(found.is_some(), reads);
        if let Some(ability) = found {
            let mut texts = Vec::new();
            r5510_wordings_twice(&ability, written_variants, &mut texts);
            assert!(
                texts.iter().any(|t| t == text.trim_end_matches('.')),
                "{texts:?}"
            );
        }
    }
}
