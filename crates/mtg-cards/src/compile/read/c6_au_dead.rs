//! The card of another creature that died, found in its new zone (CR 400.7): "exile it", "return it to the battlefield
//! under your control", "return that card to the battlefield under your control".

use super::effects::{other_dead_card, templates};
use super::tokens::Reader;
use crate::effect::{Destination, Effect};
use crate::refs::Ref;

/// The sentences after "exile it" that the card joins with "and", as the writer opens them and as the card does.
const C6_AU_JOINTS: [(&str, &str); 2] = [
    ("exile it. Create ", "exile it and you create "),
    ("exile it. Put ", "exile it and put "),
];

/// The card that returns called "that card" for "it" ("return that card to the battlefield under your control"), and
/// the sentence after "exile it" joined to it with "and" ("exile it and you create ...").
pub fn c6_au_wordings(texts: &[String]) -> Vec<String> {
    const IT: &str = "return it to the battlefield under your control";
    let mut more: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(IT))
        .map(|text| {
            text.replacen(
                IT,
                "return that card to the battlefield under your control",
                1,
            )
        })
        .collect();
    for (written, joined) in C6_AU_JOINTS {
        more.extend(
            texts
                .iter()
                .filter(|text| text.contains(written))
                .map(|text| text.replacen(written, joined, 1)),
        );
    }
    more
}

/// "exile it and you create ...", "exile it and put ...": the card of the creature that died is exiled, then the
/// sentence after "and" happens.
pub fn c6_au_exile_and(r: &mut Reader<'_>, effects: &mut Vec<Effect>) -> Option<()> {
    r.need("exile")?;
    let card = c6_au_exile_it(r)?;
    r.need("and")?;
    let rest = templates().find_map(|template| {
        r.attempt(|r| {
            let found = template(r)?;
            r.sentence_end().then_some(found)
        })
    })?;
    effects.push(Effect::Exile {
        what: card,
        bind: None,
        duration: None,
    });
    effects.extend(rest);
    Some(())
}

/// "exile it" when the trigger is another creature dying: that card in the graveyard is exiled.
pub fn c6_au_exile_it(r: &mut Reader<'_>) -> Option<Ref> {
    r.others_died.then_some(())?;
    r.word("it").then(other_dead_card)
}

/// "return it to the battlefield [tapped] under your control", also "return that card ..." and "... under its owner's
/// control": the card of the creature that died enters under the control of the ability's controller or of its owner.
pub fn c6_au_return_under_you(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.others_died.then_some(())?;
    r.attempt(|r| {
        (r.word("it") || r.words("that card")).then_some(())?;
        r.need("to the battlefield")?;
        let tapped = r.word("tapped");
        let under = if r.words("under your control") {
            Some(Ref::You)
        } else {
            r.need("under its owner's control")?;
            None
        };
        Some(vec![Effect::Move {
            what: other_dead_card(),
            to: Destination::Battlefield {
                tapped,
                under,
                transformed: false,
                counters: Vec::new(),
            },
        }])
    })
}
