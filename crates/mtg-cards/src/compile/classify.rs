//! Stage 3: the shape of a clause, which decides how the compiler reads it.

use super::keywords;

/// The shape of a clause of oracle text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClauseKind {
    /// A keyword with its parameter.
    Keyword,
    /// The text of an instant or sorcery.
    Spell,
    /// "When", "whenever" or "at ...", and a saga chapter.
    Triggered,
    /// A cost, then a colon outside quotes, then an effect.
    Activated,
    /// A planeswalker's loyalty ability: `+N`, `−N`, `0` or `−X`, then a colon.
    Loyalty,
    /// A choice of modes: "Choose one —" and its bullets, or a Spree list.
    Modal,
    /// Anything else on a permanent: a static ability, a replacement effect, a level block.
    Static,
}

/// Costs start with a mana or tap symbol or with one of these words.
const COST_WORDS: [&str; 17] = [
    "Tap",
    "Untap",
    "Sacrifice",
    "Discard",
    "Pay",
    "Exile",
    "Remove",
    "Put",
    "Return",
    "Reveal",
    "Mill",
    "Exert",
    "Collect evidence",
    "Forage",
    "Behold",
    "Blight",
    "Waterbend",
];

/// Lines of a spell that are rules about the spell itself rather than what it does.
const SPELL_RULES: [&str; 4] = [
    "This spell ",
    "~ costs ",
    "~ can't be countered",
    "As an additional cost",
];

const CHAPTERS: [&str; 10] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];

/// The kind of a clause's normalized text; `spell` is true on an instant or sorcery face.
///
/// A clause may run over several lines (modes, a level block); its first line decides.
pub fn classify(text: &str, spell: bool) -> ClauseKind {
    if text.starts_with("LEVEL ") {
        return ClauseKind::Static;
    }
    let head = text.lines().next().unwrap_or_default();
    if !text.contains('\n') && keywords::is_keyword_line(head) {
        return ClauseKind::Keyword;
    }
    if is_chapter(head) {
        return ClauseKind::Triggered;
    }
    let body = head
        .get(ability_word_len(head).unwrap_or(0)..)
        .unwrap_or(head);
    if is_loyalty(body) {
        ClauseKind::Loyalty
    } else if is_triggered(body) {
        ClauseKind::Triggered
    } else if is_activated(body) {
        ClauseKind::Activated
    } else if body.starts_with("Choose ") || body == "Spree" {
        ClauseKind::Modal
    } else if spell && !SPELL_RULES.iter().any(|rule| body.starts_with(rule)) {
        ClauseKind::Spell
    } else {
        ClauseKind::Static
    }
}

/// The byte length of an ability word and its dash ("Landfall — "), when the line starts with one.
fn ability_word_len(line: &str) -> Option<usize> {
    const DASH: &str = " \u{2014} ";
    let (at, _) = line.match_indices(DASH).next()?;
    let word = line.get(..at)?;
    let is_word = word.chars().next().is_some_and(char::is_uppercase)
        && word.chars().count() <= 40
        && word
            .chars()
            .all(|c| c.is_alphabetic() || matches!(c, ' ' | '\'' | '\u{2019}' | '-'))
        && !word.starts_with("Choose");
    let end = at + DASH.len();
    (is_word && end < line.len()).then_some(end)
}

/// A saga chapter: roman numerals, then a dash.
fn is_chapter(line: &str) -> bool {
    line.split_once(" \u{2014} ")
        .is_some_and(|(numbers, _)| numbers.split(", ").all(|number| CHAPTERS.contains(&number)))
}

/// Whether the line starts a triggered ability.
fn is_triggered(body: &str) -> bool {
    body.starts_with("When ")
        || body.starts_with("Whenever ")
        || (body.starts_with("At ") && !body.starts_with("At least"))
}

/// `+N:`, `−N:`, `0:` or `−X:`.
fn is_loyalty(body: &str) -> bool {
    let Some((cost, _)) = body.split_once(':') else {
        return false;
    };
    match cost.strip_prefix(['+', '\u{2212}', '-', '\u{2013}']) {
        Some(amount) => {
            amount == "X" || (!amount.is_empty() && amount.chars().all(|c| c.is_ascii_digit()))
        }
        None => cost == "0",
    }
}

/// Whether the line has a cost, then a colon outside quotes.
fn is_activated(body: &str) -> bool {
    let line = body.lines().next().unwrap_or_default();
    colon_outside_quotes(line).is_some_and(|at| line.get(..at).is_some_and(looks_like_cost))
}

/// The byte offset of the first colon that is not inside quoted ability text.
fn colon_outside_quotes(line: &str) -> Option<usize> {
    let mut quoted = false;
    for (at, c) in line.char_indices() {
        match c {
            '"' | '\u{201c}' | '\u{201d}' => quoted = !quoted,
            ':' if !quoted => return Some(at),
            _ => {}
        }
    }
    None
}

/// Whether the text before a colon is a cost: it starts with a symbol or a cost word.
fn looks_like_cost(before_colon: &str) -> bool {
    let first = before_colon.split(',').next().unwrap_or_default().trim();
    first.starts_with('{')
        || COST_WORDS.iter().any(|word| {
            first
                .strip_prefix(word)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
        })
}

#[cfg(test)]
mod tests {
    //! Goal: every wording of a clause lands in the right kind, with quotes, ability words, modes and keyword lists in the tricky positions.
    use super::*;
    use ClauseKind::{Activated, Keyword, Loyalty, Modal, Spell, Static, Triggered};
    use rstest::rstest;

    #[rstest]
    #[case::flying("Flying", false, Keyword)]
    #[case::list("Vigilance, deathtouch, haste", false, Keyword)]
    #[case::equip("Equip {1}", false, Keyword)]
    #[case::equip_with_a_colon_free_cost("Equip {1}", true, Keyword)]
    #[case::unread_keyword("Undying", false, Keyword)]
    #[case::level_up("Level up {1}{U}", false, Keyword)]
    #[case::cost_in_words("Ward\u{2014}Pay 3 life.", false, Keyword)]
    #[case::companion(
        "Companion \u{2014} Each nonland card in your starting deck has a different name.",
        false,
        Keyword
    )]
    #[case::choose_a_background("Choose a Background", false, Keyword)]
    #[case::keyword_as_a_subject("Flying creatures you control get +1/+1.", false, Static)]
    #[case::enchanted_creature("Enchanted creature gets +2/+2.", false, Static)]
    fn keyword_lines(#[case] text: &str, #[case] spell: bool, #[case] expected: ClauseKind) {
        assert_eq!(classify(text, spell), expected);
    }

    #[rstest]
    #[case::when("When ~ enters, draw a card.", Triggered)]
    #[case::whenever("Whenever you cast a spell, you may draw a card.", Triggered)]
    #[case::at("At the beginning of your upkeep, you win the game.", Triggered)]
    #[case::at_the_end("At the beginning of the next end step, sacrifice it.", Triggered)]
    #[case::at_least("At least two creatures must attack each combat if able.", Static)]
    #[case::ability_word(
        "Landfall \u{2014} Whenever a land you control enters, investigate.",
        Triggered
    )]
    #[case::two_word_ability_word("Spell mastery \u{2014} When ~ enters, draw a card.", Triggered)]
    #[case::chapter("I \u{2014} Create a 1/1 token.", Triggered)]
    #[case::chapters("II, III \u{2014} Draw a card.", Triggered)]
    #[case::modal_trigger(
        "Whenever ~ attacks, choose one \u{2014}\n\u{2022} Draw a card.\n\u{2022} Gain 2 life.",
        Triggered
    )]
    #[case::word_when_later("You may cast ~ whenever you could cast an instant.", Static)]
    fn triggered_abilities(#[case] text: &str, #[case] expected: ClauseKind) {
        assert_eq!(classify(text, false), expected);
    }

    #[rstest]
    #[case::mana("{T}: Add {G}.", Activated)]
    #[case::mana_cost("{R}: Draw a card.", Activated)]
    #[case::tap_and_sacrifice("{T}, Sacrifice this land: Search your library.", Activated)]
    #[case::pay_life("{T}, Pay 1 life, Sacrifice ~: Search your library.", Activated)]
    #[case::words_only("Sacrifice a creature: Draw a card.", Activated)]
    #[case::discard("Discard a card: Draw a card.", Activated)]
    #[case::ability_word("Boast \u{2014} {1}: Draw a card.", Activated)]
    #[case::activate_only("{2}: Draw a card. Activate only as a sorcery.", Activated)]
    #[case::quoted_colon_is_static("Enchanted land has \"{T}: Add {G}.\"", Static)]
    #[case::quoted_colon_after_text(
        "Creatures you control have \"{T}: Add one mana of any color.\"",
        Static
    )]
    #[case::curly_quotes("Equipped creature has \u{201c}{T}: Add {C}.\u{201d}", Static)]
    #[case::colon_in_prose("Draw a card: it is great.", Static)]
    #[case::colon_after_a_sentence("Foo bar baz. Tap: draw", Static)]
    #[case::colon_on_a_later_line_of_modes(
        "Choose one \u{2014}\n\u{2022} {T}: Draw a card.\n\u{2022} Gain 2 life.",
        Modal
    )]
    fn activated_abilities(#[case] text: &str, #[case] expected: ClauseKind) {
        assert_eq!(classify(text, false), expected);
    }

    #[rstest]
    #[case::plus("+2: Look at the top card of target player's library.", Loyalty)]
    #[case::zero("0: Draw three cards.", Loyalty)]
    #[case::minus("\u{2212}1: Return target creature to its owner's hand.", Loyalty)]
    #[case::minus_twelve("\u{2212}12: Exile all cards from target player's library.", Loyalty)]
    #[case::minus_x("\u{2212}X: ~ deals X damage to target creature.", Loyalty)]
    #[case::plus_x("+X: Draw X cards.", Loyalty)]
    #[case::ascii_minus("-3: Destroy target creature.", Loyalty)]
    #[case::en_dash("\u{2013}2: Draw two cards.", Loyalty)]
    #[case::with_modes(
        "\u{2212}2: Choose one \u{2014}\n\u{2022} Draw a card.\n\u{2022} Gain 2 life.",
        Loyalty
    )]
    #[case::plus_without_a_colon("+1/+1 counters are great.", Static)]
    #[case::number_is_not_loyalty("2: Draw a card.", Static)]
    #[case::minus_words("\u{2212}X is a number.", Static)]
    fn loyalty_abilities(#[case] text: &str, #[case] expected: ClauseKind) {
        assert_eq!(classify(text, false), expected);
    }

    #[rstest]
    #[case::choose_one("Choose one \u{2014}\n\u{2022} A.\n\u{2022} B.", Modal)]
    #[case::choose_both("Choose one or both \u{2014}\n\u{2022} A.\n\u{2022} B.", Modal)]
    #[case::choose_up_to(
        "Choose up to two \u{2014}\n\u{2022} A.\n\u{2022} B.\n\u{2022} C.",
        Modal
    )]
    #[case::spree(
        "Spree\n+ {1} \u{2014} Draw a card.\n+ {2} \u{2014} Gain 2 life.",
        Modal
    )]
    fn modal_spells(#[case] text: &str, #[case] expected: ClauseKind) {
        assert_eq!(classify(text, true), expected);
        assert_eq!(classify(text, false), expected);
    }

    #[rstest]
    #[case::damage("~ deals 3 damage to any target.", true, Spell)]
    #[case::two_sentences(
        "Damage can't be prevented this turn. ~ deals 2 damage to any target.",
        true,
        Spell
    )]
    #[case::scry("Scry 1.", true, Spell)]
    #[case::cost_increase(
        "This spell costs {1} more to cast for each target beyond the first.",
        true,
        Static
    )]
    #[case::cost_with_name("~ costs {1} less to cast.", true, Static)]
    #[case::uncounterable("~ can't be countered.", true, Static)]
    #[case::additional_cost(
        "As an additional cost to cast this spell, discard a card.",
        true,
        Static
    )]
    #[case::same_words_on_a_permanent("~ deals 3 damage to any target.", false, Static)]
    #[case::replacement(
        "If an effect would create one or more tokens under your control, it creates twice that many.",
        false,
        Static
    )]
    #[case::level_block("LEVEL 2-3\n2/4\n{U}{U}, {T}: Copy target spell.", false, Static)]
    #[case::static_ability("Equipped creature gets +2/+0.", false, Static)]
    #[case::spell_text_with_a_trigger("When you cast this spell, draw a card.", true, Triggered)]
    fn spells_and_statics(#[case] text: &str, #[case] spell: bool, #[case] expected: ClauseKind) {
        assert_eq!(classify(text, spell), expected);
    }
}
