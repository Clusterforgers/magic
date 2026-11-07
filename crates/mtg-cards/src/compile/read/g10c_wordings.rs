//! Oracle wordings of a static the writers give as "As long as ..., ~ can't be blocked.": the card may print the
//! condition last, "~ can't be blocked if you've committed a crime this turn."

/// "~ can't be blocked if <condition>." for the writers' "As long as <condition>, ~ can't be blocked."; nothing for
/// another text or a condition of several clauses.
pub(super) fn unblockable_if(text: &str) -> Option<String> {
    let held = text
        .strip_prefix("As long as ")?
        .strip_suffix(", ~ can't be blocked.")?;
    (!held.contains([',', '.'])).then(|| format!("~ can't be blocked if {held}."))
}
