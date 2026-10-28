//! Looks at the top of the library of the sixth sample of M to R: "Look at the top X cards of your library, where X is
//! ...", "You may cast a spell with ... from among them without paying its mana cost" and "Put one of those cards into
//! your hand and the other into your graveyard".

use super::{c6_mr_name_after, c6_mr_swapped, c6_mr_twin_all, c6_mr_written};

/// The head of a look whose count is worked out, not fixed.
const C6_MR_LOOK: &str = r#"{"LookAtTop":{"count":{"#;

/// Adds the wordings of the looks.
pub(super) fn add(json: &str, texts: &mut Vec<String>) {
    c6_mr_top_x(json, texts);
    c6_mr_cast_among(json, texts);
    c6_mr_other_graveyard(json, texts);
}

/// "Look at the top X cards of your library, where X is ...", from the twin that looks at three cards and the words of
/// a draw of that many cards.
fn c6_mr_top_x(json: &str, texts: &mut Vec<String>) {
    let Some(at) = json.find(C6_MR_LOOK) else {
        return;
    };
    let start = at + C6_MR_LOOK.len() - 1;
    let Some(amount) = json.get(start..).and_then(c6_mr_balanced) else {
        return;
    };
    let draw = format!(r#"{{"Spell":{{"effects":[{{"Draw":{{"count":{amount}}}}}]}}}}"#);
    let phrases: Vec<String> = c6_mr_written(&draw)
        .iter()
        .filter_map(|text| text.strip_prefix("Draw X cards, where X is "))
        .filter_map(|rest| rest.strip_suffix('.'))
        .map(str::to_owned)
        .collect();
    let whole = format!(r#"{{"LookAtTop":{{"count":{amount}"#);
    let twin = c6_mr_twin_all(json, &[(&whole, r#"{"LookAtTop":{"count":3"#)]);
    for phrase in phrases {
        texts.extend(c6_mr_swapped(
            &twin,
            "the top three cards of your library.",
            &format!("the top X cards of your library, where X is {phrase}."),
        ));
    }
}

/// The JSON object that opens `text`, up to its closing brace.
fn c6_mr_balanced(text: &str) -> Option<&str> {
    let mut depth = 0_u32;
    for (at, c) in text.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return text.get(..=at);
                }
            }
            _ => {}
        }
    }
    None
}

/// "You may cast a spell with ... from among them without paying its mana cost", from the twin that puts the card into
/// your hand.
fn c6_mr_cast_among(json: &str, texts: &mut Vec<String>) {
    let Some(name) = c6_mr_name_after(json, r#"{"CastFromZone":{"what":{"Bound":""#) else {
        return;
    };
    let cast = format!(r#"{{"CastFromZone":{{"what":{{"Bound":"{name}"}},"free":true}}}}"#);
    let kept = format!(r#"{{"Move":{{"what":{{"Bound":"{name}"}},"to":"Hand"}}}}"#);
    let chose = format!(r#""as":"{name}"}}}},{cast}"#);
    if !json.contains(&chose) {
        return;
    }
    let found: Vec<String> = c6_mr_twin_all(json, &[(&cast, &kept)])
        .iter()
        .filter_map(|text| {
            let (head, rest) = text.split_once("You may put ")?;
            let (what, tail) = rest.split_once(" from among them into your hand.")?;
            let spell = what
                .replacen("nonland card", "spell", 1)
                .replacen(" card", " spell", 1);
            Some(format!(
                "{head}You may cast {spell} from among them without paying its mana cost.{tail}"
            ))
        })
        .collect();
    texts.extend(found);
}

/// "Put one of those cards into your hand and the other into your graveyard", from the twin that puts the other on the
/// bottom of your library.
fn c6_mr_other_graveyard(json: &str, texts: &mut Vec<String>) {
    let Some(name) = c6_mr_name_after(json, r#""rest_as":""#) else {
        return;
    };
    let graveyard = format!(r#"{{"Move":{{"what":{{"Bound":"{name}"}},"to":"Graveyard"}}}}"#);
    let bottom = format!(
        r#"{{"Move":{{"what":{{"Bound":"{name}"}},"to":{{"Library":{{"place":"Bottom"}}}}}}}}"#
    );
    texts.extend(c6_mr_swapped(
        &c6_mr_twin_all(json, &[(&graveyard, &bottom)]),
        "the other on the bottom of your library",
        "the other into your graveyard",
    ));
}
