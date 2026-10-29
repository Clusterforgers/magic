//! Wordings of the popular-card sweep (t15a): the delayed trigger of "your next main phase", "put the card on top" after
//! a search, a payment the opponent declines ("that player may pay {2}. If the player doesn't, ..."), "if ~ is tapped,
//! it deals" and "Choose a color. Add an amount of mana of that color equal to ...". Each reword is checker-only.

/// A wording of the text, or `None` when its words are not there.
type T15aReword = fn(&str) -> Option<String>;

/// Each reword, in the order they are applied one after another.
const T15A_REWORDS: [T15aReword; 5] = [
    t15a_next_main,
    t15a_the_card_on_top,
    t15a_declined_payment,
    t15a_this_is_tapped,
    t15a_chosen_color_amount,
];

/// Adds each wording of the texts with one or more of the rewords applied.
pub(super) fn add(texts: &mut Vec<String>) {
    let mut said: Vec<String> = Vec::new();
    for text in texts.iter() {
        let mut current = text.clone();
        for reword in T15A_REWORDS {
            if let Some(worded) = reword(&current) {
                if !said.contains(&worded) {
                    said.push(worded.clone());
                }
                current = worded;
            }
        }
    }
    texts.extend(said);
}

/// A trigger an effect creates for "your precombat or postcombat main phase" as "your next main phase" (Mana Drain).
fn t15a_next_main(text: &str) -> Option<String> {
    let (head, tail) =
        text.split_once(". At the beginning of your precombat or postcombat main phase, ")?;
    Some(format!(
        "{head}. At the beginning of your next main phase, {tail}"
    ))
}

/// "shuffle and put that card on top" as "shuffle and put the card on top" after the search of a card (Worldly Tutor).
fn t15a_the_card_on_top(text: &str) -> Option<String> {
    let (head, tail) = text.split_once(", then shuffle and put that card on top")?;
    head.contains("earch your library for ")
        .then(|| format!("{head}, then shuffle and put the card on top{tail}"))
}

/// "<head>, you create a Treasure token unless that player pays {2}." as "<head>, that player may pay {2}. If the player
/// doesn't, you create a Treasure token." (Smothering Tithe).
fn t15a_declined_payment(text: &str) -> Option<String> {
    let (head, tail) = text.split_once(", you ")?;
    let (effect, cost) = tail.split_once(" unless that player pays ")?;
    let (cost, stop) = match cost.strip_suffix('.') {
        Some(cost) => (cost, "."),
        None => (cost, ""),
    };
    (!head.contains('.') && !cost.contains('.')).then(|| {
        format!("{head}, that player may pay {cost}. If the player doesn't, you {effect}{stop}")
    })
}

/// "if it's tapped, ~ deals" as "if ~ is tapped, it deals" for the permanent itself (Mana Vault).
fn t15a_this_is_tapped(text: &str) -> Option<String> {
    let (head, tail) = text.split_once(", if it's tapped, ~ deals ")?;
    Some(format!("{head}, if ~ is tapped, it deals {tail}"))
}

/// "Add X mana of any one color, where X is N." as "Choose a color. Add an amount of mana of that color equal to N."
/// (Three Tree City).
fn t15a_chosen_color_amount(text: &str) -> Option<String> {
    let (head, tail) = text.split_once("Add X mana of any one color, where X is ")?;
    (head.ends_with(": ") && !tail.contains(". ")).then(|| {
        format!("{head}Choose a color. Add an amount of mana of that color equal to {tail}")
    })
}
