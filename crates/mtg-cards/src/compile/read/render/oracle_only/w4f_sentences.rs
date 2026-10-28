//! Sentences the oracle text joins, orders or shortens where the renderers write them apart: "proliferate twice",
//! "you lose 1 life and amass Zombies 1", "Choose one. Activate only as a sorcery." before the modes, "Exile ~ with three
//! time counters on it.", "for the first time each turn", "~ becomes ... until end of turn and can't be blocked this
//! turn", "Choose a color. ... protection from the chosen color", "Remove X time counters from ~ and exile it", two
//! damage steps on the mana spent in one sentence and "put your choice of ... or ..." for two modes of counters.

/// A wording of the text, or `None` when its words are not there.
type W4fRewording = fn(&str) -> Option<String>;

/// Each rewording, in the order they are applied one after another.
const W4F_REWORDINGS: [W4fRewording; 10] = [
    w4f_choice_of,
    w4f_twice,
    w4f_and_amass,
    w4f_limit_first,
    w4f_exiled_with,
    w4f_first_time,
    w4f_and_unblockable,
    w4f_chosen_color,
    w4f_exile_it,
    w4f_spent_both,
];

/// Adds each wording of the texts with one or more of the rewordings applied.
pub(super) fn add(texts: &mut Vec<String>) {
    let mut said: Vec<String> = Vec::new();
    for text in texts.iter() {
        let mut current = text.clone();
        for rewording in W4F_REWORDINGS {
            if let Some(worded) = rewording(&current) {
                if !said.contains(&worded) {
                    said.push(worded.clone());
                }
                current = worded;
            }
        }
    }
    texts.extend(said);
}

/// Two modes that put counters on the same target as one sentence: "choose one —\n• Put a +1/+1 counter on T.\n• Put
/// two charge counters on T." as "put your choice of a +1/+1 counter or two charge counters on T.".
fn w4f_choice_of(text: &str) -> Option<String> {
    let (head, modes) = text.split_once("choose one —\n")?;
    let (first, second) = modes.split_once('\n')?;
    let (one, target) = first.strip_prefix("• Put ")?.split_once(" on ")?;
    let (other, again) = second.strip_prefix("• Put ")?.split_once(" on ")?;
    (target == again && !again.contains('\n'))
        .then(|| format!("{head}put your choice of {one} or {other} on {target}"))
}

/// "proliferate, then proliferate again" as "proliferate twice".
fn w4f_twice(text: &str) -> Option<String> {
    let from = "roliferate, then proliferate again";
    text.contains(from)
        .then(|| text.replacen(from, "roliferate twice", 1))
}

/// "you lose 1 life. Then amass Zombies 1." as "you lose 1 life and amass Zombies 1.": the same player does both.
fn w4f_and_amass(text: &str) -> Option<String> {
    let (at, len) = [". Then amass ", ". Amass "]
        .iter()
        .find_map(|from| text.find(from).map(|at| (at, from.len())))?;
    let opens = text[..at].rfind(". ").map_or(0, |end| end + 2);
    text[opens..at]
        .contains(", you ")
        .then(|| format!("{} and amass {}", &text[..at], &text[at + len..]))
}

/// The modes of an activated ability with the sentence that limits it after the last mode, as the oracle text puts
/// it: "{4}, {T}: Choose one. Activate only as a sorcery.\n• ...".
fn w4f_limit_first(text: &str) -> Option<String> {
    let (head, _) = text.split_once(": Choose ")?;
    if head.contains('\n') {
        return None;
    }
    let (at, len) = [" Then activate only ", " Activate only "]
        .iter()
        .find_map(|from| text.rfind(from).map(|at| (at, from.len())))?;
    let limit = format!("Activate only {}", &text[at + len..]);
    let modes = &text[..at];
    (!limit.contains('\n') && modes.contains(" —\n"))
        .then(|| modes.replacen(" —\n", &format!(". {limit}\n"), 1))
}

/// "Exile ~. Put three time counters on ~." as "Exile ~ with three time counters on it.".
fn w4f_exiled_with(text: &str) -> Option<String> {
    let (at, len) = [
        "Exile ~. Put ",
        "Exile ~. Then put ",
        "Exile ~ and you put ",
        "Exile ~ and put ",
    ]
    .iter()
    .find_map(|from| text.find(from).map(|at| (at, from.len())))?;
    let rest = &text[at + len..];
    let end = rest.find(" on ~")?;
    let counters = &rest[..end];
    counters.ends_with("counters").then(|| {
        format!(
            "{}Exile ~ with {counters} on it{}",
            &text[..at],
            &rest[end + 5..]
        )
    })
}

/// "Whenever ..., .... This ability triggers only once each turn." as "Whenever ... for the first time each turn, ...".
fn w4f_first_time(text: &str) -> Option<String> {
    let body = [
        " This ability triggers only once each turn.",
        " Then this ability triggers only once each turn.",
    ]
    .iter()
    .find_map(|limit| text.strip_suffix(limit))?;
    let event = body.strip_prefix("Whenever ")?;
    let (event, effects) = event.split_once(", ")?;
    Some(format!(
        "Whenever {event} for the first time each turn, {effects}"
    ))
}

/// "~ becomes ... until end of turn. ~ can't be blocked this turn." as one sentence with "and".
fn w4f_and_unblockable(text: &str) -> Option<String> {
    let from = [
        ". ~ can't be blocked this turn",
        ". It can't be blocked this turn",
    ]
    .iter()
    .find(|from| text.contains(*from))?;
    let at = text.find(from)?;
    let opens = text[..at]
        .rfind(". ")
        .max(text[..at].rfind(": "))
        .map_or(0, |end| end + 2);
    text[opens..].starts_with("~ ").then(|| {
        format!(
            "{} and can't be blocked this turn{}",
            &text[..at],
            &text[at + from.len()..]
        )
    })
}

/// "... protection from the color of your choice ..." as "Choose a color. ... protection from the chosen color ...".
fn w4f_chosen_color(text: &str) -> Option<String> {
    let at = text.find("protection from the color of your choice")?;
    let opens = text[..at]
        .rfind(". ")
        .max(text[..at].rfind(": "))
        .map_or(0, |end| end + 2);
    let rest = text[opens..].replacen(
        "protection from the color of your choice",
        "protection from the chosen color",
        1,
    );
    Some(format!("{}Choose a color. {rest}", &text[..opens]))
}

/// A cost "Remove X time counters from ~, Exile ~:" as "... from ~ and exile it:", and "Return all ... cards" in it as
/// "Return each ... card".
fn w4f_exile_it(text: &str) -> Option<String> {
    let from = " from ~, Exile ~: ";
    let at = text.find(from)?;
    let rest = &text[at + from.len()..];
    let rest = match rest
        .strip_prefix("Return all ")
        .and_then(|after| after.split_once(" cards "))
    {
        Some((kind, after)) => format!("Return each {kind} card {after}"),
        None => rest.to_owned(),
    };
    Some(format!("{} from ~ and exile it: {rest}", &text[..at]))
}

/// "~ deals 3 damage to A if {R} was spent to cast it. If {G} was spent to cast it, ~ deals 3 damage to B." as one
/// sentence: "~ deals 3 damage to A if {R} was spent to cast this spell and 3 damage to B if {G} was spent to cast this
/// spell."
fn w4f_spent_both(text: &str) -> Option<String> {
    let spent = " was spent to cast it";
    let (first, second) = text.split_once(". If ")?;
    let (condition, damage) = second.split_once(", ~ deals ")?;
    let damage = damage.strip_suffix('.').unwrap_or(damage);
    (first.starts_with("~ deals ")
        && first.ends_with(spent)
        && first.contains(" if {")
        && condition.ends_with(spent)
        && !damage.contains(". "))
    .then(|| {
        let this = " was spent to cast this spell";
        format!(
            "{} and {damage} if {}.",
            first.replace(spent, this),
            condition.replace(spent, this)
        )
    })
}
