//! `MayPlay$ True`: permission to cast or play cards from another zone, as the DSL's `MayCast` rule, and the `Play` effect.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, check_keys, only, unmapped};
use super::valid::{defined, selector_any, selector_in, subject};
use crate::forge::script::Params;

/// The keys of a `MayPlay` static line.
const READ: [&str; 16] = [
    "ValidAfterStack",
    "MayPlayIgnoreType",
    "MayPlayIgnoreColor",
    "RaiseCost",
    "Mode",
    "MayPlay",
    "Affected",
    "AffectedZone",
    "MayPlayLimit",
    "MayPlayWithoutManaCost",
    "MayPlayWithFlash",
    "EffectZone",
    "Description",
    "MayPlayPlayer",
    "MayPlayText",
    "Secondary",
];

/// The `MayCast` rule of a static line. `only` names the objects it covers (what an `Effect` object remembers); without
/// it the cards `Affected$` selects are covered.
pub fn may_cast(ctx: &Ctx, p: &Params, only: Option<&Value>) -> Tr<Value> {
    check_keys(p, "Static.MayPlay", &READ, |_, _| false)?;
    if !after_stack_is_redundant(p) {
        return unmapped("key:Static.MayPlay.ValidAfterStack");
    }
    let from = match p.get("AffectedZone") {
        Some(zone @ ("Graveyard" | "Exile" | "Library" | "Hand")) => zone,
        // A card milled to a graveyard or exiled is cast from the zone it was put in (the other zone names a card that
        // an effect moved there instead, or the spell once it is on the stack), and `only` fixes which card.
        Some("Graveyard,Exile")
            if only.is_some() && p.get("Affected") == Some("Card.IsRemembered") =>
        {
            "Graveyard"
        }
        Some("Exile,Stack") if only.is_some() => "Exile",
        Some(other) => return unmapped(format!("zone:MayPlay.AffectedZone={other}")),
        None => return unmapped("key:Static.MayPlay.no-AffectedZone"),
    };
    let own_zone = effect_zone(p);
    if only.is_none() && own_zone?.is_some() {
        let itself = matches!(p.get("Affected"), Some("Card.Self" | "Card.Self+nonLand"));
        if !itself || p.get("EffectZone") != p.get("AffectedZone") {
            return unmapped("key:Static.MayPlay.EffectZone");
        }
    }
    let mut permission = json!({ "from": from });
    let affected = p.get("Affected").unwrap_or("Card");
    match (&only, affected) {
        (Some(objects), valid) => {
            // "That card" with more words (`Card.IsRemembered+nonLand`): the remembered objects the rest still matches.
            let Some(props) = valid.strip_prefix("Card.") else {
                return unmapped("valid:MayPlay-Affected-with-remembered");
            };
            let parts: Vec<&str> = props.split('+').collect();
            if !parts.contains(&"IsRemembered") {
                return unmapped("valid:MayPlay-Affected-with-remembered");
            }
            permission["only"] = (*objects).clone();
            let rest: Vec<&str> = parts
                .into_iter()
                .filter(|part| *part != "IsRemembered")
                .collect();
            // Exile holds anyone's cards: "the card you own" excludes the others among the ones named.
            if !rest.is_empty() {
                let what = selector_any(ctx, &format!("Card.{}", rest.join("+")))?;
                if !what.is_empty() {
                    permission["what"] = json!(what);
                }
            }
        }
        (None, valid) => {
            // A graveyard, hand or library is the player's own: the ownership is in `from`. Exile holds anyone's cards.
            let mut what: Vec<Value> = selector_any(ctx, valid)?
                .into_iter()
                .filter(|pred| from == "Exile" || *pred != json!({"OwnedBy": "You"}))
                .filter(|pred| *pred != json!({"ControlledBy": "You"}))
                .collect();
            // A permanent card is not an instant or a sorcery.
            what.extend(super::valid::permanent_card(valid)?);
            if !what.is_empty() {
                permission["what"] = json!(what);
            }
        }
    }
    if p.get("MayPlayWithoutManaCost") == Some("True") {
        permission["free"] = json!(true);
    }
    if p.get("MayPlayWithFlash") == Some("True") {
        permission["flash"] = json!(true);
    }
    // What the caster pays in addition to the other costs of the cast.
    if let Some(extra) = p.get("RaiseCost") {
        permission["additional"] = super::cost::cost(ctx, extra)?;
    }
    match p.get("MayPlayLimit") {
        None => {}
        Some("1") => permission["once"] = json!(true),
        Some(other) => return unmapped(format!("key:Static.MayPlay.MayPlayLimit={other}")),
    }
    // Mana of any type pays for the cast (CR 609.4b); of any color is the same to the engine.
    match (p.get("MayPlayIgnoreType"), p.get("MayPlayIgnoreColor")) {
        (Some("True"), _) => permission["spend_as"] = json!("AnyType"),
        (None, Some("True")) => permission["spend_as"] = json!("AnyColor"),
        (None, None) => {}
        _ => return unmapped("key:Static.MayPlay.MayPlayIgnore-value"),
    }
    match p.get("MayPlayPlayer") {
        None | Some("You") => {}
        // The owner of the cards named may cast them.
        Some("CardOwner") if only.is_some() => {
            permission["who"] = json!({ "OwnerOf": only });
        }
        Some(other) => return unmapped(format!("key:Static.MayPlay.MayPlayPlayer={other}")),
    }
    // The label of one line of a per-type permission (Muldrotha): each line holds its own `Affected$` and limit.
    let label = p.get("MayPlayText").unwrap_or_default().to_lowercase();
    let text = p.get("Description").unwrap_or_default().to_lowercase();
    if text.contains("play") || label == "land" {
        permission["play"] = json!(true);
    }
    Ok(json!({ "MayCast": permission }))
}

/// Whether `ValidAfterStack$` (the spell check after the cast) adds nothing to `Affected$`: every card alternative other
/// than a land already names each of its words ("Spell.Insect" beside "Insect.YouOwn").
fn after_stack_is_redundant(p: &Params) -> bool {
    let Some(after) = p.get("ValidAfterStack") else {
        return true;
    };
    // A condition or a limit beside the check is a permission the plain wording does not read.
    let Some(props) = after
        .strip_prefix("Spell.")
        .filter(|_| !p.has("Condition") && !p.has("MayPlayLimit"))
    else {
        return false;
    };
    let words = |text: &str| -> Vec<String> { text.split(['.', '+']).map(str::to_owned).collect() };
    let need = words(props);
    p.get("Affected")
        .unwrap_or_default()
        .split(',')
        .all(|alternative| {
            let have = words(alternative);
            have.first().is_some_and(|first| first == "Land")
                || need.iter().all(|word| have.contains(word))
        })
}

/// The name the spell cast by a `Play` with `ReplaceGraveyard$` is bound to.
const CAST: &str = "cast";

/// The shield that exiles the spell bound as [`CAST`] instead of putting it into a graveyard from the stack.
fn exile_shield(only: &[Value]) -> Value {
    let mut what = vec![json!({"In": "Stack"}), json!({"Is": {"Bound": CAST}})];
    what.extend(only.iter().cloned());
    json!({"Shield": {
        "when": {"ZoneChange": {
            "what": what,
            "from": "Stack", "to": "Graveyard"}},
        "by": {"MoveTo": "Exile"}, "duration": "Forever"}})
}

/// Whether the text says "less than" where a script says `cmcLE`: the script is the looser rule and is not read.
fn strictly_less(oracle: &str) -> bool {
    let text = oracle.to_lowercase();
    text.contains("less than") && !text.contains("less than or equal")
}

/// Whether "otherwise" follows the cast: it may mean a spell too costly to cast, or one the player declined.
fn says_otherwise(oracle: &str) -> bool {
    oracle.to_lowercase().contains("otherwise")
}

/// How many of the cards a `Play` casts.
enum Count {
    One,
    Some(u64),
    All,
}

/// `Play`: the player may cast (or play) the cards `Defined$` names or `Valid$` selects in `ValidZone$`, up to `Amount$` of
/// them, for free when `WithoutManaCost$` says so; `CopyCard$` casts a copy of each card in exile.
pub fn play(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "Play",
        &[
            "Defined",
            "Valid",
            "ValidZone",
            "Optional",
            "WithoutManaCost",
            "ValidSA",
            "Amount",
            "Controller",
            "CopyCard",
            "RememberPlayed",
            "ValidTgts",
            "TargetMin",
            "TargetMax",
            "ReplaceGraveyard",
            "AllowRepeats",
            "ReplaceGraveyardValid",
        ],
    )?;
    if p.get("Optional") != Some("True") {
        return unmapped("key:Play.Optional");
    }
    let count = play_count(p)?;
    let who = match p.get("Controller") {
        None | Some("You") => json!("You"),
        Some(word) => defined(ctx, word)?,
    };
    let (spell_only, spell_filter) = valid_sa(ctx, p.get("ValidSA"))?;
    let spell_only = spell_only || targets_only_spells(p);
    let copy = match p.get("CopyCard") {
        None => false,
        // A named card is copied from where it lies (the cards a move to exile remembered).
        Some("True") if p.get("ValidZone") == Some("Exile") || p.has("Defined") => true,
        Some(_) => return unmapped("key:Play.CopyCard"),
    };
    let pool = play_pool(ctx, p, spell_filter)?;
    let free = p.get("WithoutManaCost") == Some("True");
    let exile = exile_instead(ctx, p, spell_only && !copy && matches!(count, Count::One))?;
    let exile_after = exile.is_some();
    let shield_only = exile.unwrap_or_default();
    let played = if exile_after {
        Some(CAST.to_owned())
    } else {
        played_name(ctx, p, spell_only || copy, matches!(count, Count::One))?
    };
    let cast = |card: Value, ctx: &mut Ctx| -> Vec<Value> {
        let mut steps = Vec::new();
        let mut card = card;
        if copy {
            let made = ctx.fresh("copy");
            steps.push(json!({"CopyCard": {"who": who, "what": card, "as": made}}));
            card = json!({ "Bound": made });
        }
        let mut effect = json!({"who": who, "what": card});
        if free {
            effect["free"] = json!(true);
        }
        if !spell_only {
            effect["play"] = json!(true);
        }
        if let Some(name) = &played {
            effect["as"] = json!(name);
            ctx.remembered = Some(json!({ "Bound": name }));
        }
        steps.push(json!({ "CastFromZone": effect }));
        steps
    };
    // "Copy that card three times. You may cast the copies": one copy and one cast after the other.
    if let (true, Count::Some(times)) = (copy && repeats_one_card(p), &count) {
        let mut steps = Vec::new();
        for _ in 0..*times {
            steps.extend(cast(pool.clone(), ctx));
        }
        return Ok(steps);
    }
    if matches!(count, Count::One) && (p.has("Defined") || p.has("ValidTgts")) {
        let mut steps = cast(pool, ctx);
        steps.extend(exile_after.then(|| exile_shield(&shield_only)));
        return Ok(steps);
    }
    let (over, mut steps) = match count {
        Count::All => (pool, Vec::new()),
        Count::One | Count::Some(_) => {
            let max = if let Count::Some(n) = count { n } else { 1 };
            let picked = ctx.fresh("picked");
            let choose = json!({"Choose": {"who": who, "from": pool,
                "count": {"min": 0, "max": max}, "as": picked}});
            (json!({ "Bound": picked }), vec![choose])
        }
    };
    if matches!(count, Count::One) {
        steps.extend(cast(over, ctx));
        steps.extend(exile_after.then(|| exile_shield(&shield_only)));
        return Ok(steps);
    }
    let each = ctx.fresh("card");
    let inner = cast(json!({ "Bound": each }), ctx);
    steps.push(json!({"RepeatEach": {"over": over, "as": each, "effects": inner}}));
    Ok(steps)
}

/// Whether the line with `ReplaceGraveyard$` targets only instants and sorceries, which are cast whatever `ValidSA$` says.
fn targets_only_spells(p: &Params) -> bool {
    p.has("ReplaceGraveyard")
        && p.get("ValidTgts").is_some_and(|tgts| {
            tgts.split(',')
                .all(|alt| alt.starts_with("Instant") || alt.starts_with("Sorcery"))
        })
}

/// Whether the line copies the one card `Defined$` names again and again (`AllowRepeats$`).
fn repeats_one_card(p: &Params) -> bool {
    p.get("AllowRepeats") == Some("True")
        && p.has("Defined")
        && !p.has("Valid")
        && !p.has("ValidZone")
}

/// How many of the cards a `Play` casts: `Amount$`.
fn play_count(p: &Params) -> Tr<Count> {
    Ok(match p.get("Amount") {
        None | Some("1") => Count::One,
        Some("All") => Count::All,
        Some(n) => match n.parse::<u64>() {
            Ok(n) if n >= 2 => Count::Some(n),
            _ => return unmapped("key:Play.Amount"),
        },
    })
}

/// `ReplaceGraveyard$` of a `Play`: "if that spell would be put into a graveyard, exile it instead" is a shield over the one
/// spell cast. The result holds the types `ReplaceGraveyardValid$` limits it to; `None` when the line has no such word.
fn exile_instead(ctx: &Ctx, p: &Params, one_spell: bool) -> Tr<Option<Vec<Value>>> {
    let Some(word) = p.get("ReplaceGraveyard") else {
        return Ok(None);
    };
    // "From a graveyard" is any graveyard: a script that keeps to the caster's own cards reads narrower than the text.
    let own_only = [p.get("Valid"), p.get("ValidTgts")]
        .into_iter()
        .flatten()
        .any(|valid| valid.contains("YouOwn") || valid.contains("YouCtrl"));
    if own_only && ctx.oracle.contains("from a graveyard") {
        return unmapped("key:Play.graveyard-owner");
    }
    if word != "Exile" || !one_spell {
        return unmapped("key:Play.ReplaceGraveyard");
    }
    match p.get("ReplaceGraveyardValid") {
        Some(valid) => Ok(Some(selector_any(ctx, valid)?)),
        None => Ok(Some(Vec::new())),
    }
}

/// The cards a `Play` may cast: those `Defined$` names, its target, or those `Valid$` selects in `ValidZone$`, with the
/// predicates of `ValidSA$`.
fn play_pool(ctx: &mut Ctx, p: &Params, spell_filter: Vec<Value>) -> Tr<Value> {
    Ok(
        match (p.get("Defined"), p.get("Valid"), p.get("ValidZone")) {
            // "Cast that card if its mana value is ..." after an exile: the remembered card in exile, kept when it passes.
            (Some("Remembered"), None, None)
                if !spell_filter.is_empty()
                    && !strictly_less(ctx.oracle)
                    && !says_otherwise(ctx.oracle) =>
            {
                let mut selector = vec![
                    json!({"Is": defined(ctx, "Remembered")?}),
                    json!({"In": "Exile"}),
                ];
                selector.extend(spell_filter);
                json!({ "Each": selector })
            }
            (Some(_), None, None) if !spell_filter.is_empty() => {
                return unmapped("key:Play.Defined-with-ValidSA-filter");
            }
            (Some(word), None, None) => defined(ctx, word)?,
            // "you may cast target card from your graveyard": the target is chosen as the ability is put on the stack.
            (None, None, None) if p.has("ValidTgts") && spell_filter.is_empty() => {
                subject(ctx, p, "", None)?
            }
            (None, Some(valid), Some(zone)) => {
                let mut selector = selector_in(ctx, valid, Some(zone))?;
                selector.extend(spell_filter);
                json!({ "Each": selector })
            }
            (None, Some(_), None) => return unmapped("key:Play.no-ValidZone"),
            (None, None, _) => return unmapped("key:Play.no-Defined"),
            _ => return unmapped("key:Play.Defined-with-Valid"),
        },
    )
}

/// `RememberPlayed$` of a `Play`: the name of the spell cast ("if you do"). A land played binds nothing and one name holds
/// one spell, so only a single card cast as a spell may be asked for.
fn played_name(ctx: &mut Ctx, p: &Params, spell: bool, one: bool) -> Tr<Option<String>> {
    match p.get("RememberPlayed") {
        None => Ok(None),
        Some("True") if spell && one => Ok(Some(ctx.fresh("cast"))),
        Some(_) => unmapped("key:Play.RememberPlayed"),
    }
}

/// `ValidSA$` of a `Play`: whether only casting is allowed, and what the spell must be besides (`Spell.cmcLE4`,
/// `Instant.cmcEQX,Sorcery.cmcEQX`), as predicates of the card cast.
fn valid_sa(ctx: &Ctx, text: Option<&str>) -> Tr<(bool, Vec<Value>)> {
    let Some(text) = text else {
        return Ok((false, Vec::new()));
    };
    if text == "Spell" {
        return Ok((true, Vec::new()));
    }
    let alternatives: Vec<String> = text
        .split(',')
        .map(|part| match part.strip_prefix("Spell") {
            Some(rest) if rest.starts_with('.') => format!("Card{rest}"),
            _ => part.to_owned(),
        })
        .collect();
    let known = |a: &String| matches!(a.split('.').next(), Some("Card" | "Instant" | "Sorcery"));
    if !alternatives.iter().all(known) {
        return unmapped("key:Play.ValidSA");
    }
    Ok((true, selector_any(ctx, &alternatives.join(","))?))
}

/// The zone a `MayPlay` static works from when the card itself carries it (`EffectZone$ Graveyard`); `None` on the battlefield.
pub fn effect_zone(p: &Params) -> Tr<Option<&'static str>> {
    match p.get("EffectZone") {
        None | Some("Battlefield") => Ok(None),
        Some("Graveyard") => Ok(Some("Graveyard")),
        Some("Exile") => Ok(Some("Exile")),
        Some(_) => unmapped("key:Static.MayPlay.EffectZone"),
    }
}
