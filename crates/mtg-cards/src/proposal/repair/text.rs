//! The words of the printed face that one ability of the proposal stands for, when the order of the abilities and the
//! lines of the text leave one reading.

use crate::support::PrintedFace;

/// What a line of the printed text is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Line {
    /// "When ...", "Whenever ...", "At ...".
    Trigger,
    /// "+1: ...", "-X: ...".
    Loyalty,
    /// "{T}: ...".
    Activated,
    /// The text of a spell, a keyword, a static ability.
    Other,
}

/// The text without its reminders in parentheses.
fn without_reminders(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0usize;
    for c in text.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// The line without the ability word before its dash ("Landfall \u{2014} Whenever ..."): a few plain words.
fn without_ability_word(line: &str) -> &str {
    match line.split_once(" \u{2014} ") {
        Some((word, rest))
            if word.split_whitespace().count() <= 3
                && !word.contains(['{', ':', ',', '.'])
                && !word.chars().any(|c| c.is_ascii_digit()) =>
        {
            rest
        }
        _ => line,
    }
}

/// The kind of a line, from its first words.
fn classify(line: &str) -> Line {
    let first = line
        .split(|c: char| !c.is_alphanumeric())
        .next()
        .unwrap_or("")
        .to_lowercase();
    if matches!(first.as_str(), "when" | "whenever" | "at") {
        return Line::Trigger;
    }
    let Some((head, _)) = line.split_once(':') else {
        return Line::Other;
    };
    if head.contains('"') {
        return Line::Other;
    }
    let head = head.trim();
    let loyalty = !head.is_empty()
        && head
            .trim_start_matches(['+', '\u{2212}', '-'])
            .chars()
            .all(|c| c.is_ascii_digit() || c == 'X');
    if loyalty {
        Line::Loyalty
    } else {
        Line::Activated
    }
}

/// The part of a line the effects are worded in: after the cost, after the event.
fn effects_of(line: &str, kind: Line) -> &str {
    match kind {
        Line::Trigger => line.split_once(", ").map_or(line, |(_, rest)| rest),
        Line::Loyalty | Line::Activated => line.split_once(':').map_or(line, |(_, rest)| rest),
        Line::Other => line,
    }
}

/// The words of the effects of the `nth` of `count` abilities of this kind in the face (`Spell`, `Triggered`,
/// `Activated`, `Loyalty`), when the lines of that kind are as many as the abilities, or the face is an instant or a
/// sorcery with one `Spell`.
pub(super) fn ability_text(
    face: &PrintedFace,
    kind: &str,
    nth: usize,
    count: usize,
) -> Option<String> {
    let lines: Vec<(Line, String)> = face
        .oracle_text
        .lines()
        .map(without_reminders)
        .map(|line| without_ability_word(line.trim()).to_owned())
        .filter(|line| !line.is_empty())
        .map(|line| (classify(&line), line))
        .collect();
    let wanted = match kind {
        "Spell" => {
            let spell = face.type_line.contains("Instant") || face.type_line.contains("Sorcery");
            let bullets = lines.iter().any(|(_, line)| line.starts_with('\u{2022}'));
            if !spell || count != 1 || bullets {
                return None;
            }
            let own: Vec<&str> = lines
                .iter()
                .filter(|(line, _)| *line == Line::Other)
                .map(|(_, text)| text.as_str())
                .collect();
            return Some(own.join(" "));
        }
        "Triggered" => Line::Trigger,
        "Loyalty" => Line::Loyalty,
        "Activated" => Line::Activated,
        _ => return None,
    };
    let own: Vec<&(Line, String)> = lines.iter().filter(|(line, _)| *line == wanted).collect();
    if own.len() != count {
        return None;
    }
    let (line, text) = own.get(nth)?;
    Some(effects_of(text, *line).to_owned())
}

/// Whether the text has the word (a whole word, case-sensitive).
pub(super) fn has_word(text: &str, word: &str) -> bool {
    text.split(|c: char| !c.is_alphanumeric() && c != '\'')
        .any(|part| part == word)
}
