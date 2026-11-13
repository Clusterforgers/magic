//! r55-10 wordings of a Fortification's host: "Fortified land has indestructible.", "Whenever fortified land becomes
//! tapped, ...". The host of an attachment is `Ref::Attached` whatever its kind, so each is read and written through
//! its twin about an enchanted land, which the other readers and writers word.

use crate::ability::Ability;

/// The pairs of words swapped between a clause and its twin.
const R5510_SWAPS: [(&str, &str); 2] = [
    ("Fortified land", "Enchanted land"),
    ("fortified land", "enchanted land"),
];

/// The clause with every Fortification word turned into its Aura twin; `None` when it names no fortified land.
fn r5510_twin_text(text: &str) -> Option<String> {
    R5510_SWAPS
        .iter()
        .any(|(fortified, _)| text.contains(fortified))
        .then(|| {
            R5510_SWAPS
                .iter()
                .fold(text.to_owned(), |text, (fortified, enchanted)| {
                    text.replace(fortified, enchanted)
                })
        })
}

/// Reads a clause about a fortified land through its twin about an enchanted land.
pub(super) fn r5510_read(text: &str, read: impl Fn(&str) -> Option<Ability>) -> Option<Ability> {
    read(&r5510_twin_text(text)?)
}

/// Adds the Fortification wordings of every wording about an enchanted land.
pub(super) fn r5510_wordings(texts: &mut Vec<String>) {
    let fortified: Vec<String> = texts
        .iter()
        .filter(|text| {
            R5510_SWAPS
                .iter()
                .any(|(_, enchanted)| text.contains(enchanted))
        })
        .map(|text| {
            R5510_SWAPS
                .iter()
                .fold(text.clone(), |text, (fortified, enchanted)| {
                    text.replace(enchanted, fortified)
                })
        })
        .collect();
    for text in fortified {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a clause about a fortified land reads as its twin about an enchanted land and writes back in its own words,
    //! and a clause about an enchanted creature or a land of no attachment gains no Fortification wording.
    use super::{r5510_twin_text, r5510_wordings};
    use rstest::rstest;

    #[rstest]
    #[case::static_ability(
        "Fortified land has indestructible.",
        Some("Enchanted land has indestructible.")
    )]
    #[case::trigger(
        "Whenever fortified land becomes tapped, target creature gets +1/+1 until end of turn.",
        Some(
            "Whenever enchanted land becomes tapped, target creature gets +1/+1 until end of turn."
        )
    )]
    #[case::no_fortification("Enchanted creature gets +1/+1.", None)]
    #[case::other_land("Target land becomes tapped.", None)]
    fn r5510_twins(#[case] text: &str, #[case] twin: Option<&str>) {
        assert_eq!(r5510_twin_text(text).as_deref(), twin);
    }

    #[rstest]
    #[case::land("Enchanted land has indestructible", true)]
    #[case::creature("Enchanted creature has indestructible", false)]
    fn r5510_writes_the_fortified_wording(#[case] text: &str, #[case] added: bool) {
        let mut texts = vec![text.to_owned()];
        r5510_wordings(&mut texts);
        assert_eq!(texts.len(), if added { 2 } else { 1 });
    }
}
