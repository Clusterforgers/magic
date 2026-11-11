//! A creature of one of several creature types is also printed with the types before the noun: "target creature that's a
//! Wolf or a Werewolf" reads as "target Wolf or Werewolf creature"; a subtype an object lacks is also printed after the
//! noun: "target non-God enchantment card" reads as "target enchantment card that isn't a God".

/// The wordings with each "creature that's a Wolf or a Werewolf" said as "Wolf or Werewolf creature", and each "non-God
/// enchantment card" as "enchantment card that isn't a God".
pub(super) fn rmt0_kinds_before(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .flat_map(|text| [before(text), lacking_after(text)])
        .flatten()
        .collect()
}

/// The nouns a phrase ends with, after which "that isn't a God" is said.
const NOUNS: [&str; 4] = ["card", "creature", "permanent", "spell"];

/// The text with its first "non-God" before a noun said as "that isn't a God" after the noun.
fn lacking_after(text: &str) -> Option<String> {
    let at = text.find(" non-")?;
    let rest = &text[at + " non-".len()..];
    let end = rest.find(' ')?;
    let name = &rest[..end];
    name.chars().next().filter(char::is_ascii_uppercase)?;
    let after = &rest[end + 1..];
    let words: Vec<&str> = after.split(' ').collect();
    let noun_at = words
        .iter()
        .take(3)
        .position(|word| NOUNS.contains(&word.trim_end_matches(['.', ','])))?;
    // A mark after the noun stays after the words that follow it.
    let noun = words[noun_at];
    let bare = noun.trim_end_matches(['.', ',']);
    let mark = &noun[bare.len()..];
    let mut head: Vec<&str> = words[..noun_at].to_vec();
    head.push(bare);
    let head = head.join(" ");
    let tail = words[noun_at + 1..].join(" ");
    let article = if name.starts_with(['A', 'E', 'I', 'O', 'U']) {
        "an"
    } else {
        "a"
    };
    let tail = if tail.is_empty() {
        String::new()
    } else {
        format!(" {tail}")
    };
    Some(format!(
        "{} {head} that isn't {article} {name}{mark}{tail}",
        &text[..at]
    ))
}

/// The text with its first list of creature types after "creature that's" moved before the noun.
fn before(text: &str) -> Option<String> {
    const HEAD: &str = "creature that's ";
    let at = text.find(HEAD)?;
    let rest = &text[at + HEAD.len()..];
    let mut names = Vec::new();
    let mut tail = rest;
    loop {
        let after = tail
            .strip_prefix("a ")
            .or_else(|| tail.strip_prefix("an "))?;
        let end = after
            .find(|c: char| !c.is_alphanumeric() && c != '-')
            .unwrap_or(after.len());
        let name = &after[..end];
        name.chars().next().filter(char::is_ascii_uppercase)?;
        names.push(name);
        tail = &after[end..];
        if let Some(more) = tail
            .strip_prefix(", or ")
            .or_else(|| tail.strip_prefix(" or "))
        {
            tail = more;
            let last = tail
                .strip_prefix("a ")
                .or_else(|| tail.strip_prefix("an "))?;
            let end = last
                .find(|c: char| !c.is_alphanumeric() && c != '-')
                .unwrap_or(last.len());
            let name = &last[..end];
            name.chars().next().filter(char::is_ascii_uppercase)?;
            names.push(name);
            tail = &last[end..];
            break;
        }
        tail = tail.strip_prefix(", ")?;
    }
    (names.len() >= 2).then_some(())?;
    let list = match names.as_slice() {
        [one, two] => format!("{one} or {two}"),
        [many @ .., last] => format!("{}, or {last}", many.join(", ")),
        [] => return None,
    };
    Some(format!("{}{list} creature{tail}", &text[..at]))
}

#[cfg(test)]
mod tests {
    //! Goal: a list of creature types after "creature that's" is also written before the noun, and nothing else is.
    use super::{before, lacking_after};
    use rstest::rstest;

    #[rstest]
    #[case::two(
        "Return target creature that's a Wolf or a Werewolf to its owner's hand.",
        Some("Return target Wolf or Werewolf creature to its owner's hand.")
    )]
    #[case::three(
        "Destroy target creature that's an Elf, a Goblin, or a Wolf.",
        Some("Destroy target Elf, Goblin, or Wolf creature.")
    )]
    #[case::one("Destroy target creature that's a Wolf.", None)]
    #[case::not_a_type("Destroy target creature that's attacking or blocking.", None)]
    #[case::none("Destroy target creature.", None)]
    fn the_types_move_before_the_noun(#[case] text: &str, #[case] expected: Option<&str>) {
        assert_eq!(before(text).as_deref(), expected);
    }

    #[rstest]
    #[case::a_card(
        "When ~ enters, return target non-God enchantment card from your graveyard to your hand.",
        Some(
            "When ~ enters, return target enchantment card that isn't a God from your graveyard to your hand."
        )
    )]
    #[case::an_elf(
        "Destroy target non-Elf creature.",
        Some("Destroy target creature that isn't an Elf.")
    )]
    #[case::lowercase("Destroy target nonblack creature.", None)]
    fn a_lacking_subtype_moves_after_the_noun(#[case] text: &str, #[case] expected: Option<&str>) {
        assert_eq!(lacking_after(text).as_deref(), expected);
    }
}
