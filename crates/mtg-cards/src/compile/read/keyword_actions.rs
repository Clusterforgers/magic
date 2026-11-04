//! Keyword actions that the macro table expands (`crate::macros::ACTIONS`): "Earthbend 2." and "~ endures 2.", and the
//! words they render back to.

use std::sync::OnceLock;

use mtg_core::card::CardType;

use super::tokens::Reader;
use crate::effect::Effect;
use crate::macros;
use crate::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable};

/// The most a keyword action counts when its effects are matched back to words.
const MOST: u32 = 20;

/// "Earthbend 2." (a land you control is the target) or "~ endures 2.": the effects of the table entry.
pub fn keyword_action(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    if r.word("earthbend") {
        let amount = r.number().filter(|n| *n > 0)?;
        if !r.targets.is_empty() {
            return None;
        }
        r.targets.push(TargetSpec {
            what: Targetable::Object(Selector(vec![
                Pred::Type(CardType::Land),
                Pred::ControlledBy(Ref::You),
            ])),
            bounds: Bounds::default(),
            divide: None,
        });
        return macros::action_effects("earthbend", amount, &Ref::Target(0));
    }
    // "~ endures 2.", "target creature endures 2.", "it endures 2."
    r.attempt(|r| {
        let (on, many) = super::effects::subject(r)?;
        (!many && matches!(on, Ref::This | Ref::Target(_) | Ref::Triggering)).then_some(())?;
        r.word("endures").then_some(())?;
        let amount = r.number().filter(|n| *n > 0)?;
        macros::action_effects("endure", amount, &on)
    })
}

/// The expansions of a table entry for each number up to `MOST`, built once: matching effects back to words runs for
/// every ability a view words, and expanding a template parses JSON.
fn expansions(name: &str, on: &Ref) -> &'static [(u32, Vec<Effect>)] {
    type Row = Vec<(u32, Vec<Effect>)>;
    static CACHE: OnceLock<[Row; 4]> = OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        let build = |name: &str, on: &Ref| -> Row {
            (1..=MOST)
                .filter_map(|amount| Some((amount, macros::action_effects(name, amount, on)?)))
                .collect()
        };
        [
            build("earthbend", &Ref::Target(0)),
            build("endure", &Ref::This),
            build("endure", &Ref::Target(0)),
            build("endure", &Ref::Triggering),
        ]
    });
    match (name, on) {
        ("earthbend", _) => &cache[0],
        ("endure", Ref::This) => &cache[1],
        ("endure", Ref::Target(0)) => &cache[2],
        ("endure", Ref::Triggering) => &cache[3],
        _ => &[],
    }
}

/// The words of the table entry the effects begin with, and how many effects they are; `say` words the object an
/// endure works on when it is not the card itself.
pub fn text(
    effects: &[Effect],
    mut say: impl FnMut(&Ref) -> Option<String>,
) -> Option<(String, usize)> {
    let found = |name: &str, on: &Ref| {
        expansions(name, on).iter().find_map(|(amount, steps)| {
            effects.starts_with(steps).then_some((*amount, steps.len()))
        })
    };
    if let Some((amount, len)) = found("earthbend", &Ref::Target(0)) {
        return Some((format!("earthbend {amount}"), len));
    }
    [Ref::This, Ref::Target(0), Ref::Triggering]
        .iter()
        .find_map(|on| {
            let (amount, len) = found("endure", on)?;
            Some((format!("{} endures {amount}", say(on)?), len))
        })
}

#[cfg(test)]
mod tests {
    //! Goal: the keyword actions render back to the words they are read from for every number.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::earthbend_one("earthbend", 1)]
    #[case::earthbend_four("earthbend", 4)]
    #[case::endure_two("endure", 2)]
    #[case::endure_five("endure", 5)]
    fn an_action_renders_to_the_words_it_was_read_from(#[case] name: &str, #[case] amount: u32) {
        let on = if name == "endure" {
            Ref::This
        } else {
            Ref::Target(0)
        };
        let steps = macros::action_effects(name, amount, &on).unwrap();
        let (words, len) = text(&steps, |_| Some("target creature".to_owned())).unwrap();
        assert_eq!(len, steps.len());
        assert!(words.ends_with(&amount.to_string()), "{words}");
    }

    #[test]
    fn effects_of_no_action_have_no_words() {
        assert!(text(&[Effect::Proliferate { who: Ref::You }], |_| None).is_none());
    }

    #[test]
    fn wording_effects_that_are_no_action_is_cheap() {
        // A view words every ability of every object on every event; expanding templates per call made a game crawl.
        let effects = [Effect::Proliferate { who: Ref::You }];
        let started = std::time::Instant::now();
        for _ in 0..20_000 {
            assert!(text(&effects, |_| None).is_none());
        }
        assert!(started.elapsed().as_secs() < 2, "{:?}", started.elapsed());
    }
}
