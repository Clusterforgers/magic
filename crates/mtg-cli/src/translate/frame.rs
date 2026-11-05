//! The slot frame: the parts a script line of any verb is made of (who acts, what it affects, target limits, how many, who
//! decides, optional, destination), each read in one place. A verb declares the slots it accepts and lowers what they give.

use serde_json::{Value, json};

use super::amount::{amount, counter_kind};
use super::ctx::{Ctx, Tr, only, unmapped};
use super::valid::{defined, subject};
use crate::forge::script::Params;

/// A family of script keys that means the same on every verb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// The verb's actor key (`DefinedPlayer$`, or `Defined$` on a verb whose subject is a player): the player the line acts for.
    Actor,
    /// `Defined$`: objects the line names.
    Named,
    /// The verb's pool key (`ChangeType$`): what a player picks from.
    Pool,
    /// `ValidTgts$`: the line targets.
    Target,
    /// `TargetMin$`, `TargetMax$`.
    Limits,
    /// The verb's count key (`ChangeNum$`).
    Count,
    /// `Chooser$`, `AlternativeDecider$`: who decides, when it is not the actor.
    Decider,
    /// `Optional$`, `Mandatory$`.
    Optional,
    /// Where the objects go and how they get there.
    Destination,
}

impl Slot {
    /// The keys of the slot that are the same on every verb.
    fn keys(self) -> &'static [&'static str] {
        match self {
            Slot::Named => &["Defined"],
            Slot::Target => &["ValidTgts"],
            Slot::Limits => &["TargetMin", "TargetMax"],
            Slot::Decider => &["Chooser", "AlternativeDecider"],
            Slot::Optional => &["Optional", "Mandatory"],
            Slot::Destination => &[
                "Destination",
                "Origin",
                "LibraryPosition",
                "DestinationAlternative",
                "LibraryPositionAlternative",
                "Shuffle",
                "Tapped",
                "Attacking",
                "GainControl",
                "WithCountersType",
                "WithCountersAmount",
                "Transformed",
                "AttachedTo",
            ],
            Slot::Actor | Slot::Pool | Slot::Count => &[],
        }
    }
}

/// What a verb (or one shape of it) accepts: its slots, the spelling of the two keys verbs name differently and its own keys.
#[derive(Debug, Clone, Copy)]
pub struct Verb {
    /// The label of the gaps of its lines.
    pub label: &'static str,
    /// The slots it reads.
    pub slots: &'static [Slot],
    /// The key of `Slot::Actor`.
    pub actor: &'static str,
    /// The key of `Slot::Pool`.
    pub pool: &'static str,
    /// The key of `Slot::Count`.
    pub count: &'static str,
    /// The keys only this verb has, which it reads itself.
    pub own: &'static [&'static str],
}

impl Verb {
    /// Checks the keys of the line: one outside the declared slots is a gap `key:<label>.<key>`, and one that is accepted
    /// but not read by the end of the line is a gap too (`audit`).
    pub fn admit(&self, p: &Params) -> Tr<()> {
        let mut keys: Vec<&str> = self.own.to_vec();
        for slot in self.slots {
            keys.extend_from_slice(slot.keys());
            match slot {
                Slot::Actor => keys.push(self.actor),
                Slot::Pool => keys.push(self.pool),
                Slot::Count => keys.push(self.count),
                _ => {}
            }
        }
        only(p, self.label, &keys)
    }
}

/// A word that names one player, by the way the line gets to that player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerWord {
    /// A player of the event that triggered the line (`TriggeredPlayer`, `TriggeredCardController`...).
    Event,
    /// A player the line remembered or chose earlier.
    Earlier,
    /// The controller or owner of a target, or the player a parent line targeted.
    OfTarget,
    /// The player an aura enchants.
    Enchanted,
    /// The defending player.
    Defending,
}

impl PlayerWord {
    /// The kind of player a word names, or none when it names anything else.
    pub fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "TriggeredActivator"
            | "TriggeredSourceController"
            | "TriggeredDefendingPlayer"
            | "TriggeredPlayer"
            | "TriggeredTarget"
            | "TriggeredCardController" => Self::Event,
            "RememberedController" | "ChosenPlayer" | "Remembered" | "Player.IsRemembered" => {
                Self::Earlier
            }
            "TargetedController" | "TargetedOwner" | "ParentTarget" => Self::OfTarget,
            "Player.EnchantedBy" => Self::Enchanted,
            "DefendingPlayer" => Self::Defending,
            _ => return None,
        })
    }
}

/// Whether a word names one player a line without a target of its own may act for.
fn is_player_word(word: &str) -> bool {
    PlayerWord::parse(word).is_some()
}

/// The actor of a verb whose subject is a player (`Defined$ You`, `Player`, `Opponent`, a player word or the one player the
/// line targets). `Self` is the source's controller. A word that names objects is a gap, never a player.
pub fn player_actor(ctx: &mut Ctx, p: &Params, verb: &Verb) -> Tr<Value> {
    let gap = || unmapped(format!("key:{}.{}", verb.label, verb.actor));
    if p.has("ValidTgts") {
        if !matches!(
            p.get("ValidTgts"),
            Some("Player" | "Opponent" | "Player.Opponent")
        ) {
            return gap();
        }
        return subject(ctx, p, "You", None);
    }
    match p.get(verb.actor) {
        None | Some("You" | "Self" | "Card.Self") => Ok(json!("You")),
        Some(word @ ("Player" | "Opponent" | "Player.Opponent" | "Player.Other")) => {
            defined(ctx, word)
        }
        Some("Targeted" | "TargetedPlayer") => ctx.player_target(),
        Some(word) if is_player_word(word) => defined(ctx, word),
        Some(_) => gap(),
    }
}

/// Who a line that picks is about: the player whose cards they are, the player who decides, and whether a loop runs.
#[derive(Debug, Clone)]
pub struct Parties {
    /// The actor (`DefinedPlayer$`, or the player the line targets).
    pub acts: Value,
    /// The decider (`Chooser$`, or by default the actor; you when you target the actor).
    pub decides: Value,
    /// The players the line is made for in turn, and the name the player of each pass is bound to.
    pub each: Option<(Value, String)>,
    /// The actor is the target of the line.
    pub targeted: bool,
}

/// Whether a reference names at most one player, so that one player can answer for it.
fn is_single(who: &Value) -> bool {
    !matches!(who.as_str(), Some("Opponents" | "EachPlayer"))
}

/// A word that names one player who decides.
fn one_player(ctx: &Ctx, word: &str, label: &str) -> Tr<Value> {
    let who = defined(ctx, word)?;
    if is_single(&who) {
        Ok(who)
    } else {
        unmapped(format!("key:{label}={word}"))
    }
}

/// `DefinedPlayer$` and `Chooser$` of a line that picks: whose cards they are (you, the one player the line targets, a
/// player a word names, or each of them in turn) and who picks (the actor unless `Chooser$` says otherwise, you when you
/// target the actor without saying who).
pub fn parties(ctx: &mut Ctx, p: &Params, verb: &Verb) -> Tr<Parties> {
    let label = verb.label;
    let targets = p.has("ValidTgts");
    let one_target = matches!(
        p.get("ValidTgts"),
        Some("Player" | "Opponent" | "Player.Opponent")
    ) && !p.has("TargetMin")
        && !p.has("TargetMax");
    if targets && !one_target {
        return unmapped(format!("key:{label}.ValidTgts"));
    }
    let word = p.get(verb.actor);
    let mut each = None;
    let (acts, targeted) = match (word, targets) {
        (None | Some("You"), false) => (json!("You"), false),
        (None | Some("Targeted" | "TargetedPlayer"), true) => (subject(ctx, p, "You", None)?, true),
        (Some("You"), true) => {
            subject(ctx, p, "You", None)?;
            (json!("You"), false)
        }
        (Some(group @ ("Player" | "Player.Opponent" | "Player.Other")), false) => {
            let name = ctx.fresh("player");
            let acts = json!({ "Bound": name });
            each = Some((defined(ctx, group)?, name));
            (acts, false)
        }
        // The player a parent line targeted.
        (Some("Targeted" | "TargetedPlayer"), false)
            if ctx
                .targets
                .last()
                .is_some_and(|spec| spec.get("bounds").is_none()) =>
        {
            (ctx.player_target()?, false)
        }
        (Some("ParentTarget"), false) if !parent_is_player(ctx) => {
            return unmapped(format!("key:{label}.{}", verb.actor));
        }
        (Some(word), false) if is_player_word(word) => (defined(ctx, word)?, false),
        _ => return unmapped(format!("key:{label}.{}", verb.actor)),
    };
    let decides = match p.get("Chooser") {
        None if targets && word.is_none() => json!("You"),
        None => acts.clone(),
        Some(chooser) if Some(chooser) == word && each.is_none() => acts.clone(),
        Some("You") => json!("You"),
        Some("Targeted") if targets => ctx.last_target()?,
        Some(chooser)
            if is_player_word(chooser) && (!targets || word == Some("You")) && each.is_none() =>
        {
            one_player(ctx, chooser, &format!("{label}.Chooser"))?
        }
        Some(_) => return unmapped(format!("key:{label}.Chooser")),
    };
    Ok(Parties {
        acts,
        decides,
        each,
        targeted,
    })
}

/// Whether the target the parent line declared is one player.
pub fn parent_is_player(ctx: &Ctx) -> bool {
    ctx.parent_target
        .and_then(|at| ctx.targets.get(usize::from(at)))
        .is_some_and(|spec| {
            matches!(spec["what"].as_str(), Some("Player" | "Opponent"))
                && spec.get("bounds").is_none()
        })
}

/// Who decides on a line where no player picks: `Chooser$`, with `DefinedPlayer$` saying the same or nothing more; you
/// without one.
pub fn decider(ctx: &Ctx, p: &Params, verb: &Verb) -> Tr<Value> {
    let label = verb.label;
    match (p.get("Chooser"), p.get(verb.actor)) {
        (None, None | Some("You")) => Ok(json!("You")),
        (Some(chooser), None) => one_player(ctx, chooser, &format!("{label}.Chooser")),
        (Some(chooser), Some(word)) if chooser == word => {
            one_player(ctx, chooser, &format!("{label}.Chooser"))
        }
        _ => unmapped(format!("key:{label}.{}", verb.actor)),
    }
}

/// `Defined$` and `ValidTgts$`: what the line affects. A target is declared; `Defined$` beside a target of a player names the
/// objects while the player serves another slot ("unless that player pays"), and `Defined$ Targeted` says the target again.
pub fn affects(ctx: &mut Ctx, p: &Params, default: &str, zone: Option<&str>) -> Tr<Value> {
    if let (Some(valid), Some(word)) = (p.get("ValidTgts"), p.get("Defined"))
        && word != "Targeted"
    {
        if !matches!(valid, "Player" | "Opponent") {
            return unmapped("key:Frame.Defined-beside-a-target");
        }
        let spec = super::valid::target_spec(ctx, p, None)?;
        ctx.target(spec)?;
        return defined(ctx, word);
    }
    subject(ctx, p, default, zone)
}

/// How many of a pool a player picks.
#[derive(Debug, Clone)]
pub struct Count {
    /// The fewest.
    pub min: Value,
    /// The most.
    pub max: Value,
    /// The line left the count at one.
    pub is_one: bool,
}

/// The count of a pool: `<count key>$`, one without it; the fewest is the most when the line says `Mandatory$ True` and is
/// not optional, none otherwise.
pub fn count(ctx: &Ctx, p: &Params, verb: &Verb) -> Tr<Count> {
    let word = p.get(verb.count);
    let max = amount(ctx, word.unwrap_or("1"))?;
    let min = if p.get("Mandatory") == Some("True") && !is_optional(p) {
        max.clone()
    } else {
        json!(0)
    };
    Ok(Count {
        min,
        max,
        is_one: matches!(word, None | Some("1")),
    })
}

/// Whether the line says `Optional$` with any word but `False`.
pub fn is_optional(p: &Params) -> bool {
    p.get("Optional").is_some_and(|word| word != "False")
}

/// The effects as a "may" of the player when the line is optional. A "may" the printed description does not carry is
/// Forge's narrowing, not the card's: a gap.
pub fn optional(p: &Params, label: &str, effects: Vec<Value>, who: &Value) -> Tr<Vec<Value>> {
    match p.get("Optional") {
        None | Some("False") => return Ok(effects),
        Some("True" | "You") => {}
        Some(_) => return unmapped(format!("key:{label}.Optional")),
    }
    if let Some(text) = p.get("SpellDescription")
        && !text
            .to_lowercase()
            .split(|c: char| !c.is_alphabetic())
            .any(|w| w == "may")
    {
        return unmapped(format!("key:{label}.Optional"));
    }
    Ok(ask(effects, who))
}

/// The one way a "may" is written: the player `who` decides whether to do all of `effects`. A player asked twice by two parts
/// of one line (the trigger and the step it runs) is asked once: effects that already begin with that player's "may" take
/// the rest into it.
pub fn ask(mut effects: Vec<Value>, who: &Value) -> Vec<Value> {
    let same_asker = |may: &Value| may.get("who").unwrap_or(&json!("You")) == who;
    let begins_with_may = effects
        .first()
        .and_then(|step| step.get("May"))
        .is_some_and(same_asker);
    if begins_with_may {
        let rest: Vec<Value> = effects.drain(1..).collect();
        if let Some(inner) = effects[0]["May"]["effects"].as_array_mut() {
            inner.extend(rest);
        }
        return effects;
    }
    let mut may = json!({"May": {"effects": effects}});
    if *who != json!("You") {
        may["May"]["who"] = who.clone();
    }
    vec![may]
}

/// Where moved objects go, as a destination or the exile verb.
pub enum Goes {
    /// Exiled.
    Exile,
    /// Exiled face down, linked to the source (hideaway).
    Hidden,
    /// To this destination.
    To(Value),
}

/// Where the moved objects go; a library placement follows `LibraryPosition$` (the top without one, as Forge) and a
/// `Shuffle$ True` puts them in shuffled.
pub fn goes(ctx: &Ctx, p: &Params, destination: &str) -> Tr<Goes> {
    goes_in(ctx, p, destination, p.get("LibraryPosition"), false)
}

/// The one way moved objects go, or two with the player who decides between them and the word for each.
pub struct Places {
    /// The way they go without a choice, and when a choice falls to nobody.
    pub first: Goes,
    /// `DestinationAlternative$`: the other way.
    pub second: Option<Alternative>,
}

/// The other way moved objects may go.
pub struct Alternative {
    /// The other way.
    pub goes: Goes,
    /// Who decides.
    pub who: Value,
    /// The word of the first way, then of this one.
    pub words: [String; 2],
}

/// The `Destination$` of a line and its alternative: `DestinationAlternative$` with `LibraryPositionAlternative$` is a second
/// way the objects may go, decided by `AlternativeDecider$` (you without one). `searched` makes a library placement the
/// search's own shuffle and position, never a default.
pub fn places(ctx: &Ctx, p: &Params, destination: &str, searched: bool) -> Tr<Places> {
    let first = goes_in(ctx, p, destination, p.get("LibraryPosition"), searched)?;
    let Some(zone) = p.get("DestinationAlternative") else {
        return Ok(Places {
            first,
            second: None,
        });
    };
    if p.get("Shuffle") == Some("True") {
        return unmapped("key:Frame.DestinationAlternative-with-Shuffle");
    }
    let position = p.get("LibraryPositionAlternative").or(Some("0"));
    let goes = goes_in(ctx, p, zone, position, searched)?;
    let who = match p.get("AlternativeDecider") {
        None => json!("You"),
        Some(word) => one_player(ctx, word, "Frame.AlternativeDecider")?,
    };
    let words = [way_word(&first)?, way_word(&goes)?];
    if words[0] == words[1] {
        return unmapped("key:Frame.DestinationAlternative-same");
    }
    Ok(Places {
        first,
        second: Some(Alternative { goes, who, words }),
    })
}

/// The word a player reads for a way objects may go.
fn way_word(way: &Goes) -> Tr<String> {
    let Goes::To(to) = way else {
        return Ok("Exile".to_owned());
    };
    if let Some(word) = to.as_str() {
        return Ok(word.to_owned());
    }
    if to.get("Battlefield").is_some() {
        return Ok("Battlefield".to_owned());
    }
    let place = &to["Library"]["place"];
    match (place.as_str(), place["Below"].as_u64()) {
        (Some("Top"), _) => Ok("Top".to_owned()),
        (Some("Bottom"), _) => Ok("Bottom".to_owned()),
        (_, Some(1)) => Ok("Second from the top".to_owned()),
        (_, Some(2)) => Ok("Third from the top".to_owned()),
        _ => unmapped("key:Frame.DestinationAlternative-word"),
    }
}

/// The move of `what` the places say: one step, or the decider's choice between the two ways and the move it picked. Nobody
/// to decide takes the first way.
pub fn place(ctx: &mut Ctx, what: &Value, places: Places) -> Vec<Value> {
    let Places { first, second } = places;
    let Some(second) = second else {
        return vec![moves(what, first)];
    };
    let [word, other] = second.words;
    let first_way = (word, vec![moves(what, first)]);
    let other_way = (other, vec![moves(what, second.goes)]);
    either(ctx, &second.who, &first_way, &other_way)
}

/// The one general step of a player choosing between two courses: the choice of a word, then the course it picked. Nobody to
/// choose takes the first course.
pub fn either(
    ctx: &mut Ctx,
    who: &Value,
    first: &(String, Vec<Value>),
    second: &(String, Vec<Value>),
) -> Vec<Value> {
    let name = ctx.fresh("way");
    let mut choose = json!({"kind": {"Options": [first.0, second.0]}, "as": name});
    if *who != json!("You") {
        choose["who"] = who.clone();
    }
    vec![
        json!({ "ChooseValue": choose }),
        json!({"If": {
            "condition": {"ChosenWord": {"name": name, "word": second.0}},
            "then": second.1,
            "otherwise": first.1,
        }}),
    ]
}

/// The way objects go to a zone. `searched`: what a search found, whose library placement is the search's own.
fn goes_in(
    ctx: &Ctx,
    p: &Params,
    destination: &str,
    position: Option<&str>,
    searched: bool,
) -> Tr<Goes> {
    let tapped = p.get("Tapped") == Some("True");
    if destination != "Battlefield" {
        for key in [
            "GainControl",
            "Attacking",
            "WithCountersType",
            "Transformed",
            "AttachedTo",
        ] {
            if key == "WithCountersType" && destination == "Exile" {
                continue;
            }
            if p.has(key) {
                return unmapped(format!("key:ChangeZone.{key}"));
            }
        }
    }
    Ok(match destination {
        "Exile" => Goes::Exile,
        "Hand" => Goes::To(json!("Hand")),
        "Graveyard" => Goes::To(json!("Graveyard")),
        "Battlefield" if p.has("AttachedTo") => {
            // The attached placement carries only the host: any other way of entering is a gap, not a drop.
            for key in [
                "Tapped",
                "Attacking",
                "GainControl",
                "WithCountersType",
                "Transformed",
            ] {
                if p.has(key) {
                    return unmapped(format!("key:ChangeZone.AttachedTo-with-{key}"));
                }
            }
            let host = defined(ctx, p.get("AttachedTo").unwrap_or_default())?;
            Goes::To(json!({"BattlefieldAttached": {"to": host}}))
        }
        "Battlefield" => {
            let mut enters = json!({"tapped": tapped});
            match p.get("Transformed") {
                None | Some("False") => {}
                Some("True") => enters["transformed"] = json!(true),
                Some(_) => return unmapped("key:ChangeZone.Transformed"),
            }
            match p.get("GainControl") {
                None | Some("False") => {}
                // The script says "under your control" where the printed text says the owner's: the script is out of date.
                Some("True")
                    if [
                        "that card to the battlefield under its owner's",
                        "that card to the battlefield under its owner’s",
                    ]
                    .iter()
                    .any(|text| ctx.oracle.contains(text)) =>
                {
                    return unmapped("key:ChangeZone.GainControl-printed-as-owner");
                }
                Some("True") => enters["under"] = json!("You"),
                Some(word) => enters["under"] = one_player(ctx, word, "Frame.GainControl")?,
            }
            if p.has("WithCountersType") {
                enters["counters"] = with_counters(p)?;
            }
            Goes::To(json!({ "Battlefield": enters }))
        }
        "Library" => {
            let place = match (position, p.get("Shuffle"), searched) {
                (_, Some("True"), false) | (None, Some("True"), _) => json!("Shuffled"),
                (Some("0"), ..) | (None, Some("False") | None, false) => json!("Top"),
                (Some("-1"), ..) => json!("Bottom"),
                (Some(n), ..) => match n.parse::<u32>() {
                    Ok(below) if below >= 1 => json!({ "Below": below }),
                    _ => return unmapped("key:ChangeZone.LibraryPosition"),
                },
                _ => return unmapped("key:ChangeZone.LibraryPosition"),
            };
            Goes::To(json!({"Library": {"place": place}}))
        }
        other => return unmapped(format!("zone:Destination={other}")),
    })
}

/// The counters a card enters with (`WithCountersType$ P1P1 | WithCountersAmount$ 2`): one kind with a fixed number, one
/// counter without an amount; a keyword counter, a list or a number that is not fixed stays a gap.
fn with_counters(p: &Params) -> Tr<Value> {
    let Some(kind) = p.get("WithCountersType").and_then(counter_kind) else {
        return unmapped("key:ChangeZone.WithCountersType");
    };
    let number = match p.get("WithCountersAmount") {
        None => Some(1),
        Some(text) => text.parse::<u32>().ok().filter(|n| *n >= 1),
    };
    match number {
        Some(n) => Ok(json!([[kind, n]])),
        None => unmapped("key:ChangeZone.WithCountersAmount"),
    }
}

/// The name of what the first move among the steps moved, naming it when it has none; a step list with no move of `verb` is a
/// gap: what follows reads the moved objects as they are now (CR 400.7), never the reference the move began with.
pub fn moved_name(
    ctx: &mut Ctx,
    effects: &mut [Value],
    verb: &str,
    prefix: &str,
    gap: &str,
) -> Tr<String> {
    let Some(step) = effects.iter_mut().find(|step| step.get(verb).is_some()) else {
        return unmapped(gap.to_owned());
    };
    if let Some(name) = step[verb].get("as").and_then(Value::as_str) {
        return Ok(name.to_owned());
    }
    let name = ctx.fresh(prefix);
    step[verb]["as"] = json!(name);
    Ok(name)
}

/// `Attacking$ True`: the one object that entered joins the attack, its controller choosing what it attacks (CR 508.4).
pub fn enters_attacking(ctx: &mut Ctx, p: &Params, effects: &mut Vec<Value>) -> Tr<()> {
    match p.get("Attacking") {
        None => return Ok(()),
        Some("True") => {}
        Some(_) => return unmapped("key:Frame.Attacking"),
    }
    if p.has("TargetMax") || !matches!(p.get("ChangeNum"), None | Some("1")) {
        return unmapped("key:Frame.Attacking-for-several");
    }
    let name = moved_name(ctx, effects, "Move", "moved", "key:Frame.Attacking-move")?;
    effects.push(json!({"JoinAttack": {"what": {"Bound": name}}}));
    Ok(())
}

/// `WithCountersType$` on a move to exile ("exile it with three time counters on it"): the counters go on the object it became.
pub fn exiled_with_counters(ctx: &mut Ctx, p: &Params, effects: &mut Vec<Value>) -> Tr<()> {
    if !p.has("WithCountersType") {
        return Ok(());
    }
    let counters = with_counters(p)?;
    let name = moved_name(
        ctx,
        effects,
        "Exile",
        "exiled",
        "key:Frame.WithCounters-exile",
    )?;
    effects.push(json!({"PutCounters": {
        "on": {"Bound": name},
        "counter": counters[0][0],
        "count": counters[0][1],
    }}));
    Ok(())
}

/// The step that moves `what` the given way.
pub fn moves(what: &Value, goes: Goes) -> Value {
    match goes {
        Goes::Exile => json!({"Exile": {"what": what}}),
        Goes::Hidden => json!({"ExileHidden": {"what": what, "held": true}}),
        Goes::To(to) => json!({"Move": {"what": what, "to": to}}),
    }
}
