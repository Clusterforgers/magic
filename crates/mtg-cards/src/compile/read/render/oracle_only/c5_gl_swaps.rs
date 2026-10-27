//! Words the cards of the fifth sample of the names G to L use where the renderers write another: one sacrifice of the
//! source "and" other permanents, a return "attached to" a target, "also get" in a later static of a ladder, "When you
//! attack with" for a trigger that transforms the card, and a token "tapped and attacking" named before its kind.

use super::c5_gl_twins::{C5GlTwin, c5_gl_twin_wordings};
use crate::ability::Ability;
use crate::effect::{Effect, Mode};

/// The swaps of the words alone: the twin is the ability itself, and the JSON parts only say where they apply.
const C5_GL_SWAPS: [(&[&str], C5GlTwin); 4] = [
    (
        &[r#""SacrificeSelf",{"Sacrifice":"#],
        C5GlTwin {
            json: &[],
            words: &[("Sacrifice ~, Sacrifice ", "Sacrifice ~ and ")],
        },
    ),
    (
        &[
            r#""from":"Graveyard""#,
            r#"{"Attach":{"what":"This","to":{"Target":0}}}"#,
        ],
        C5GlTwin {
            json: &[],
            words: &[(
                " from your graveyard to the battlefield. Attach ~ to target ",
                " from your graveyard to the battlefield attached to target ",
            )],
        },
    ),
    (
        &[
            r#"{"Static":{"affected":{"Each":"#,
            r#""condition":{"Compare":{"left":{"Count":"#,
        ],
        C5GlTwin {
            json: &[],
            words: &[("you control get +", "you control also get +")],
        },
    ),
    (
        &[r#"{"GroupAttacks":"#, r#"{"Transform":{"what":"This"}}"#],
        C5GlTwin {
            json: &[],
            words: &[("Whenever you attack with ", "When you attack with ")],
        },
    ),
];

/// The words of a token that enters tapped and attacking, as the renderers write them.
const C5_GL_ATTACKING: &str = " that's tapped and attacking";

/// Adds the wordings of each swap that applies to the ability.
pub(super) fn c5_gl_add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(json) = serde_json::to_string(ability) else {
        return;
    };
    for (needs, twin) in &C5_GL_SWAPS {
        if needs.iter().all(|part| json.contains(part)) {
            texts.extend(c5_gl_twin_wordings(&json, twin));
        }
    }
    if c5_gl_makes_attackers(ability) {
        let found: Vec<String> = texts
            .iter()
            .filter_map(|text| c5_gl_attacking_first(text))
            .collect();
        texts.extend(found);
    }
}

/// Whether a step of the ability, or of one of its modes, creates tokens that are tapped and attacking.
fn c5_gl_makes_attackers(ability: &Ability) -> bool {
    ability.effects().iter().any(|effect| match effect {
        Effect::CreateToken {
            tapped: true,
            attacking: true,
            ..
        } => true,
        Effect::Modal { modes, .. } => modes.iter().any(|Mode { effects, .. }| {
            effects.iter().any(|effect| {
                matches!(
                    effect,
                    Effect::CreateToken {
                        tapped: true,
                        attacking: true,
                        ..
                    }
                )
            })
        }),
        _ => false,
    })
}

/// "Create a tapped and attacking X/X ... token" for "Create an X/X ... token ... that's tapped and attacking"; none
/// when the text has no such token.
fn c5_gl_attacking_first(text: &str) -> Option<String> {
    let (head, tail) = text.split_once(C5_GL_ATTACKING)?;
    let (verb, article) = ["reate an ", "reate a "]
        .iter()
        .filter_map(|article| head.rfind(article).map(|at| (at, article.len())))
        .max_by_key(|&(at, _)| at)?;
    let (before, rest) = head.split_at(verb);
    let token = rest.get(article..)?;
    if token.contains('.') {
        return None;
    }
    Some(format!(
        "{before}reate a tapped and attacking {token}{tail}"
    ))
}
