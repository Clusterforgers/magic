//! A token with an ability in quotes as the oracle text words it: the name before the ability ("token named ~'s
//! Pridemate with "..."), the token called "this token" in its ability, and no size for a token whose ability sets its
//! power and toughness ("create a colorless Sculpture artifact creature token with "This token's power and toughness
//! are each equal to ..."").

/// The opening of an ability in quotes that a token has.
const W4F_QUOTED: &str = "token with \"";

/// Adds each wording of an ability that creates a token with an ability in quotes, worded as the oracle text does.
pub(super) fn add(texts: &mut Vec<String>) {
    let mut said: Vec<String> = Vec::new();
    for text in texts.iter().filter(|text| text.contains(W4F_QUOTED)) {
        let named = w4f_named_first(text);
        let called = w4f_this_token(&named);
        let bare = w4f_unsized(&called);
        for worded in [named, called, bare] {
            if worded != *text && !said.contains(&worded) {
                said.push(worded);
            }
        }
    }
    texts.extend(said);
}

/// `token with "Q" named N` as `token named N with "Q"`; the text itself when it has no name after the quotes.
fn w4f_named_first(text: &str) -> String {
    let Some(open) = text.find(W4F_QUOTED) else {
        return text.to_owned();
    };
    let quote = open + W4F_QUOTED.len() - 1;
    let Some(close) = text[quote + 1..].find('"').map(|at| quote + 1 + at + 1) else {
        return text.to_owned();
    };
    let Some(after) = text[close..].strip_prefix(" named ") else {
        return text.to_owned();
    };
    let end = after.find('.').unwrap_or(after.len());
    format!(
        "{}token named {} with {}{}",
        &text[..open],
        &after[..end],
        &text[quote..close],
        &after[end..]
    )
}

/// The text with `~` in each ability in quotes a token has called "this token" ("This token" opening a sentence).
fn w4f_this_token(text: &str) -> String {
    let pieces: Vec<&str> = text.split('"').collect();
    let mut out: Vec<String> = Vec::with_capacity(pieces.len());
    for (at, piece) in pieces.iter().enumerate() {
        let granted = at % 2 == 1
            && at + 1 < pieces.len()
            && pieces[at - 1].ends_with(" with ")
            && pieces[at - 1].contains("token");
        if !granted {
            out.push((*piece).to_owned());
            continue;
        }
        let opened = match piece.strip_prefix('~') {
            Some(rest) => format!("This token{rest}"),
            None => (*piece).to_owned(),
        };
        out.push(
            opened
                .replace(". ~", ". This token")
                .replace('~', "this token"),
        );
    }
    // A sentence that ends in the quotes takes no period after them: `... opponent." Wizards you control ...`.
    out.join("\"").replace(".\". ", ".\" ")
}

/// The text without the size "0/0" of a token whose ability in quotes sets its power and toughness.
fn w4f_unsized(text: &str) -> String {
    if text.contains("power and toughness are each equal to") {
        text.replacen(" a 0/0 ", " a ", 1)
    } else {
        text.to_owned()
    }
}
