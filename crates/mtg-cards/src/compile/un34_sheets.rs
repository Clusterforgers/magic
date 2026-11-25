//! Sticker sheets (CR 123): a line "{TK}{TK} — Flying" is an ability sticker and "{TK}{TK} — 4/2" a power/toughness
//! sticker for that many tickets; the three name stickers are the words of the sheet's name and its three art stickers
//! are free, as every Unfinity sheet prints them.

use crate::ability::{Ability, Sticker};
use crate::keyword::Keyword;
use crate::modify::Modification;
use crate::refs::{Ref, ZoneKind};
use crate::support::PrintedFace;

/// The type line of a sticker sheet.
const SHEET_TYPE: &str = "Stickers";

/// The art stickers every sheet has.
const ART_STICKERS: usize = 3;

/// The counts a sheet writes in words before a ticket symbol.
const NUMBER_WORDS: [&str; 9] = [
    "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
];

/// Sheets whose name stickers are not one word each.
const NAME_GROUPS: [(&str, [&str; 3]); 1] = [(
    "Ancestral Hot Dog Minotaur",
    ["Ancestral", "Hot Dog", "Minotaur"],
)];

/// Whether the face is a sticker sheet.
pub(super) fn is_sheet(face: &PrintedFace) -> bool {
    face.type_line == SHEET_TYPE
}

/// The tickets and the rest of a sticker line: "{TK}{TK} — Flying" is 2 and "Flying"; none for another line.
pub(crate) fn sticker_line(text: &str) -> Option<(u32, &str)> {
    let mut rest = text;
    let mut tickets = 0;
    while let Some(after) = rest.strip_prefix("{TK}") {
        tickets += 1;
        rest = after;
    }
    let rest = rest.strip_prefix(" \u{2014} ")?;
    (tickets > 0 && !rest.trim().is_empty()).then_some((tickets, rest))
}

/// A power/toughness sticker's numbers: "4/2".
fn power_toughness(text: &str) -> Option<(i32, i32)> {
    let (power, toughness) = text.trim().split_once('/')?;
    let number = |part: &str| {
        (!part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
            .then(|| part.parse().ok())
            .flatten()
    };
    Some((number(power)?, number(toughness)?))
}

/// The sticker of a line, its ability read by `read`; none when the ability does not read into one ability or a run of
/// keywords. A run of keywords ("Vigilance, reach") is one sticker: a static ability that gives the permanent each one.
pub(super) fn sticker(
    tickets: u32,
    rest: &str,
    read: impl FnOnce(&str) -> (Option<Ability>, Vec<Ability>),
) -> Option<Ability> {
    if let Some((power, toughness)) = power_toughness(rest) {
        return Some(Ability::Sticker(Sticker::PowerToughness {
            power,
            toughness,
            tickets,
        }));
    }
    let ability = match read(rest) {
        (Some(ability), more) if more.is_empty() => ability,
        (Some(first), more) => {
            let mods = std::iter::once(first)
                .chain(more)
                .map(|ability| match ability {
                    Ability::Keyword(keyword) => Some(Modification::AddKeyword(keyword)),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            Ability::Static {
                affected: Ref::This,
                mods,
                condition: None,
                defining: false,
                from: ZoneKind::Battlefield,
            }
        }
        (None, _) => return None,
    };
    Some(Ability::Sticker(Sticker::Ability {
        ability: Box::new(ability),
        tickets,
    }))
}

/// The keywords an ability sticker gives: its keyword, or each keyword of its run.
pub fn sticker_keywords(ability: &Ability) -> Vec<&Keyword> {
    match ability {
        Ability::Keyword(keyword) => vec![keyword],
        Ability::Static {
            affected: Ref::This,
            mods,
            condition: None,
            ..
        } => mods
            .iter()
            .map(|modification| match modification {
                Modification::AddKeyword(keyword) => Some(keyword),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// The clause of a face, a sticker line of a sheet in today's wording.
pub(super) fn current_clause(sheet: bool, text: String) -> String {
    if sheet {
        current_line(&text).unwrap_or(text)
    } else {
        text
    }
}

/// The tickets and the rest of a sticker line of a sheet; none for another face or line.
pub(super) fn sheet_line(sheet: bool, text: &str) -> Option<(u32, &str)> {
    sheet.then(|| sticker_line(text)).flatten()
}

/// A sticker line in today's wording; none for another line.
pub(super) fn current_line(text: &str) -> Option<String> {
    let (tickets, rest) = sticker_line(text)?;
    let symbols = "{TK}".repeat(usize::try_from(tickets).ok()?);
    Some(format!("{symbols} \u{2014} {}", current_wording(rest)))
}

/// The rest of a sticker line in today's wording, which the readers know; the sheets keep older wordings of the same
/// rules ("enters under your control", "Add {2}", "seven {TK}", "Whenever this permanent dies").
fn current_wording(rest: &str) -> String {
    let mut text = rest.to_owned();
    for (old, new) in [
        (
            "a creature enters under your control",
            "a creature you control enters",
        ),
        (
            "a land enters under your control",
            "a land you control enters",
        ),
        ("Whenever this permanent dies", "When this creature dies"),
        ("When this permanent dies", "When this creature dies"),
        ("Whenever this creature dies", "When this creature dies"),
        (
            "Whenever this permanent leaves the battlefield",
            "When this permanent leaves the battlefield",
        ),
        (
            "Whenever this creature leaves the battlefield",
            "When this creature leaves the battlefield",
        ),
    ] {
        text = text.replace(old, new);
    }
    for (count, word) in (1usize..).zip(NUMBER_WORDS) {
        let tickets = "{TK}".repeat(count);
        text = text.replace(&format!(" {word} {{TK}}"), &format!(" {tickets}"));
        let colorless = "{C}".repeat(count);
        text = text.replace(&format!("Add {{{count}}}"), &format!("Add {colorless}"));
    }
    text
}

/// The free name and art stickers of a sheet: a name sticker for each word of its name, then three art stickers.
pub(super) fn free_stickers(name: &str) -> Vec<Ability> {
    let words: Vec<&str> = NAME_GROUPS
        .iter()
        .find(|(sheet, _)| *sheet == name)
        .map_or_else(|| name.split(' ').collect(), |(_, groups)| groups.to_vec());
    words
        .into_iter()
        .map(|words| Sticker::Name {
            words: words.to_owned(),
            tickets: 0,
        })
        .chain(std::iter::repeat_n(Sticker::Art {}, ART_STICKERS))
        .map(Ability::Sticker)
        .collect()
}

#[cfg(test)]
mod tests {
    //! Goal: a sticker line splits into its tickets and its sticker, a power/toughness sticker reads its numbers, the
    //! sheets' older wordings become today's, a run of keywords is one sticker, and a sheet's free stickers are its
    //! name's words and three art stickers.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(
        "{TK}{TK} \u{2014} Whenever a creature enters under your control, draw a card.",
        "{TK}{TK} \u{2014} Whenever a creature you control enters, draw a card."
    )]
    #[case(
        "{TK} \u{2014} Landfall \u{2014} Whenever a land enters under your control, scry 1.",
        "{TK} \u{2014} Landfall \u{2014} Whenever a land you control enters, scry 1."
    )]
    #[case(
        "{TK}{TK}{TK} \u{2014} When this permanent dies, you get seven {TK}.",
        "{TK}{TK}{TK} \u{2014} When this creature dies, you get {TK}{TK}{TK}{TK}{TK}{TK}{TK}."
    )]
    #[case(
        "{TK}{TK} \u{2014} Whenever this permanent leaves the battlefield, draw a card.",
        "{TK}{TK} \u{2014} When this permanent leaves the battlefield, draw a card."
    )]
    #[case(
        "{TK}{TK} \u{2014} {T}: Add {2}.",
        "{TK}{TK} \u{2014} {T}: Add {C}{C}."
    )]
    #[case("{TK}{TK} \u{2014} Flying", "{TK}{TK} \u{2014} Flying")]
    fn a_sheet_line_reads_in_todays_wording(#[case] line: &str, #[case] current: &str) {
        assert_eq!(current_line(line).as_deref(), Some(current));
    }

    #[rstest]
    #[case::one(vec![Keyword::Flying], false)]
    #[case::a_run(vec![Keyword::Vigilance, Keyword::Reach], true)]
    #[case::twice(vec![Keyword::Prowess, Keyword::Prowess], true)]
    fn a_run_of_keywords_is_one_sticker(#[case] keywords: Vec<Keyword>, #[case] run: bool) {
        let read: Vec<Ability> = keywords.iter().cloned().map(Ability::Keyword).collect();
        let Some(Ability::Sticker(Sticker::Ability { ability, tickets })) =
            sticker(3, "Keywords", |_| {
                let mut read = read.into_iter();
                (read.next(), read.collect())
            })
        else {
            panic!("an ability sticker");
        };
        assert_eq!(tickets, 3);
        assert_eq!(matches!(*ability, Ability::Static { .. }), run);
        assert_eq!(
            sticker_keywords(&ability),
            keywords.iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_line_that_reads_into_more_than_keywords_is_no_sticker() {
        let read = |_: &str| {
            (
                Some(Ability::Keyword(Keyword::Flying)),
                vec![Ability::Manual {
                    reason: String::new(),
                    span: crate::card::Span { start: 0, end: 1 },
                }],
            )
        };
        assert_eq!(sticker(2, "Flying and more", read), None);
    }

    #[rstest]
    #[case("{TK}{TK} \u{2014} Flying", Some((2, "Flying")))]
    #[case("{TK}{TK}{TK}{TK}{TK} \u{2014} 8/6", Some((5, "8/6")))]
    #[case("{TK} \u{2014} Haste", Some((1, "Haste")))]
    #[case("Flying", None)]
    #[case("{TK}{TK}: Draw a card.", None)]
    #[case("Hellbent \u{2014} Draw a card.", None)]
    fn a_sticker_line_has_tickets_and_a_rest(
        #[case] line: &str,
        #[case] split: Option<(u32, &str)>,
    ) {
        assert_eq!(sticker_line(line), split);
    }

    #[rstest]
    #[case("4/2", Some((4, 2)))]
    #[case("13/13", Some((13, 13)))]
    #[case("+2/+0", None)]
    #[case("X/2", None)]
    #[case("4", None)]
    fn power_and_toughness_are_two_numbers(#[case] text: &str, #[case] found: Option<(i32, i32)>) {
        assert_eq!(power_toughness(text), found);
    }

    #[rstest]
    #[case("Cool Fluffy Loxodon", &["Cool", "Fluffy", "Loxodon"])]
    #[case("Ancestral Hot Dog Minotaur", &["Ancestral", "Hot Dog", "Minotaur"])]
    #[case("Narrow-Minded Baloney Fireworks", &["Narrow-Minded", "Baloney", "Fireworks"])]
    #[case("Urza's Dark Cannonball", &["Urza's", "Dark", "Cannonball"])]
    fn a_sheet_names_its_words_and_has_three_art_stickers(
        #[case] name: &str,
        #[case] words: &[&str],
    ) {
        let free = free_stickers(name);
        let named: Vec<&str> = free
            .iter()
            .filter_map(|ability| match ability {
                Ability::Sticker(Sticker::Name { words, tickets: 0 }) => Some(words.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(named, words);
        let art = free
            .iter()
            .filter(|ability| matches!(ability, Ability::Sticker(Sticker::Art {})))
            .count();
        assert_eq!(
            (art, free.len()),
            (ART_STICKERS, words.len() + ART_STICKERS)
        );
    }
}
