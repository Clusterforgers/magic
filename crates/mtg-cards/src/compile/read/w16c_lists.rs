//! Lists of w16 that have no order: "another creature or Vehicle" and "another Vehicle or creature" are one text, so are
//! "hexproof and haste" and "haste and hexproof". Only lists of card types, colors, keywords and creature types are
//! sorted; a list of actions keeps its order.

/// The text with each list of unordered words written in one order.
pub(super) fn normal(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut at = 0;
    while at < words.len() {
        if let Some((list, end)) = chain(&words, at) {
            out.push(list);
            at = end;
        } else {
            out.push(words[at].to_owned());
            at += 1;
        }
    }
    let changed = out.join(" ");
    (changed != text).then_some(changed)
}

/// The words that name a card type, a color, a keyword or a kind of object in a list.
const VOCABULARY: &[&str] = &[
    "enters",
    "dies",
    "attacks",
    "blocks",
    "attack",
    "block",
    "artifact",
    "artifacts",
    "creature",
    "creatures",
    "enchantment",
    "enchantments",
    "land",
    "lands",
    "planeswalker",
    "planeswalkers",
    "instant",
    "instants",
    "sorcery",
    "sorceries",
    "battle",
    "battles",
    "permanent",
    "permanents",
    "spell",
    "spells",
    "token",
    "tokens",
    "card",
    "cards",
    "white",
    "blue",
    "black",
    "red",
    "green",
    "colorless",
    "flying",
    "haste",
    "hexproof",
    "trample",
    "vigilance",
    "deathtouch",
    "lifelink",
    "reach",
    "menace",
    "indestructible",
    "defender",
    "shroud",
    "fear",
    "intimidate",
    "prowess",
    "ward",
    "player",
    "players",
    "opponent",
    "opponents",
    "tapped",
    "untapped",
    "attacking",
    "blocking",
    "legendary",
    "basic",
    "snow",
    "historic",
    "multicolored",
    "monocolored",
];

/// Whether the word is one an unordered list is made of (a creature type is a capital letter in the middle of a text).
fn item(word: &str, first: bool) -> bool {
    let core = word.trim_end_matches([',', '.', ';', ':', '"']);
    if core.len() < 3 || core.contains(['{', '}', '/', '~', '+', '-']) {
        return false;
    }
    let lower = core.to_ascii_lowercase();
    VOCABULARY.contains(&lower.as_str())
        || (lower.starts_with("non") && core.chars().all(|c| c.is_ascii_lowercase()))
        || (!first
            && core.chars().next().is_some_and(char::is_uppercase)
            && core.chars().all(char::is_alphabetic))
}

/// The list that starts at `at` as its words in order, and where it ends.
fn chain(words: &[&str], at: usize) -> Option<(String, usize)> {
    let mut items: Vec<&str> = Vec::new();
    let mut connector = "";
    let mut cur = at;
    let mut tail = "";
    loop {
        let word = words.get(cur)?;
        if !item(word, cur == 0) {
            break;
        }
        let core = word.trim_end_matches([',', '.', ';', ':', '"']);
        let comma = word.ends_with(',');
        items.push(core);
        let next = words.get(cur + 1).copied();
        match (comma, next) {
            (_, Some(link @ ("and" | "or"))) if connector.is_empty() || connector == link => {
                connector = link;
                cur += 2;
                let last = words.get(cur)?;
                if !item(last, false) {
                    return None;
                }
                let last_core = last.trim_end_matches([',', '.', ';', ':', '"']);
                items.push(last_core);
                tail = &last[last_core.len()..];
                cur += 1;
                break;
            }
            (true, Some(_)) => cur += 1,
            _ => break,
        }
    }
    if items.len() < 2 || connector.is_empty() {
        return None;
    }
    // "each land and Ally" and "each land or Ally" name the same objects.
    let each = at > 0 && matches!(words[at - 1].to_ascii_lowercase().as_str(), "each" | "all");
    let connector = if each { "or" } else { connector };
    items.sort_by_key(|word| word.to_ascii_lowercase());
    let (last, init) = items.split_last()?;
    let joined = if init.len() == 1 {
        format!("{} {connector} {last}{tail}", init[0])
    } else {
        format!("{}, {connector} {last}{tail}", init.join(", "))
    };
    Some((joined, cur))
}
