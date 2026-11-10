//! Static modes that change a rule of the game: what players and objects can't do, the hand size, the legend rule and
//! untapping during the other players' untap steps.

use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::valid::selector_any;
use crate::forge::script::Params;

/// The rule of a static line this module reads; `None` for any other mode.
pub fn rule(ctx: &Ctx, p: &Params) -> Option<Tr<Value>> {
    if let Some(found) = super::statics_cant::rule(ctx, p) {
        return Some(found);
    }
    if let Some(found) = super::statics_defender::rule(ctx, p) {
        return Some(found);
    }
    let mode = p.get("Mode").unwrap_or_default();
    let found = match mode {
        "CantGainLife" => players(p, mode, "ValidPlayer").map(|who| json!({"CantGainLife": who})),
        "CantDraw" if p.has("DrawLimit") => super::ep24_rules::ep24_draw_limit(p),
        "CantDraw" => players(p, mode, "ValidPlayer").map(|who| json!({"CantDraw": who})),
        "CantSacrifice" if p.has("ValidCause") => super::ep24_rules::ep24_spared_sacrifice(p),
        "CantChangeLife" => {
            players(p, mode, "ValidPlayer").map(|who| json!({"LifeTotalCantChange": who}))
        }
        "CantPlayLand" => players(p, mode, "Player").map(|who| cant(&who, &json!("PlayLand"))),
        "CantPutCounter" => put_counter(ctx, p),
        "CantSacrifice" => {
            objects(ctx, p, mode, "ValidCard").map(|who| cant(&who, &json!("Sacrifice")))
        }
        "CantTransform" => {
            objects(ctx, p, mode, "ValidCard").map(|who| cant(&who, &json!("Transform")))
        }
        "CantTarget" => cant_target(ctx, p),
        "CantAttach" => cant_attach(ctx, p),
        "AttackRestrict" => attack_restrict(p),
        "Panharmonicon" => super::statics_again::again(ctx, p),
        "CombatDamageToughness" => toughness_damage(ctx, p),
        "AssignNoCombatDamage" => {
            objects(ctx, p, mode, "ValidCard").map(|who| combat(&who, "AssignsNoCombatDamage"))
        }
        "AssignCombatDamageAsUnblocked" => assign_as_unblocked(ctx, p),
        "CastWithFlash" => super::statics_cast::cast_with_flash(ctx, p),
        "CanAttackDefender" => {
            objects(ctx, p, mode, "ValidCard").map(|who| combat(&who, "AttackDespiteDefender"))
        }
        "BlockTapped" => {
            objects(ctx, p, mode, "ValidCard").map(|who| combat(&who, "BlockWhileTapped"))
        }
        "IgnoreHexproof" => ignore_hexproof(ctx, p),
        "IgnoreLandWalk" => ignore_landwalk(ctx, p),
        "CanBlockIfShadow" => block_shadow(ctx, p),
        "TapPowerValue" => tap_power_value(ctx, p),
        "IgnoreLegendRule" => ignore_legend_rule(ctx, p, mode),
        "CountersRemain" => counters_remain(p),
        "UntapOtherPlayer" => untap_other_players(ctx, p),
        "Continuous"
            if p.get("AddKeyword")
                .is_some_and(|k| k.starts_with("PayLifeInsteadOf:")) =>
        {
            super::ep24_rules::ep24_life_for_mana(p)
        }
        "Continuous" if p.has("SetMaxHandSize") => hand_size(ctx, p),
        "Continuous" if p.has("RaiseMaxHandSize") => hand_size_change(ctx, p),
        "Continuous" if p.has("AdjustLandPlays") => land_plays(ctx, p),
        "Continuous" if p.get("AddKeyword") == Some("CantSearchLibrary") => cant_search(p),
        "Continuous" if p.has("AdditionalOptionalVote") => e14c_vote_again(p),
        "Continuous" if p.has("MayLookAt") && !p.has("MayPlay") => super::statics_look::look(p),
        _ => return None,
    };
    Some(found.map(|rule| json!({"Rule": {"rule": rule}})))
}

fn cant(who: &Value, action: &Value) -> Value {
    json!({"Cant": {"who": who, "action": action}})
}

/// Refuses a key the mode does not read; a static in another zone than the battlefield is another ability.
fn keys(p: &Params, mode: &str, read: &[&str]) -> Tr<()> {
    check_keys(p, &format!("Static.{mode}"), read, |key, value| {
        key == "Mode" || (key == "EffectZone" && value == "Battlefield")
    })
}

/// `CanBlockIfShadow`: the `ValidBlocker$` creatures can block creatures with shadow "as though it had shadow" or "as
/// though they didn't have shadow", as the description says; any other wording is a gap.
fn block_shadow(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "CanBlockIfShadow", &["ValidAttacker", "ValidBlocker"])?;
    if p.get("ValidAttacker") != Some("Creature.withShadow") {
        return unmapped("key:Static.CanBlockIfShadow.ValidAttacker");
    }
    let text = p.get("Description").unwrap_or_default();
    let quirk = if text.contains("as though they didn't have shadow") {
        "BlockIgnoringShadow"
    } else if text.contains("as though it had shadow") || text.contains("as though they had shadow")
    {
        "BlockAsThoughShadow"
    } else {
        return unmapped("key:Static.CanBlockIfShadow.Description");
    };
    let Some(word) = p.get("ValidBlocker") else {
        return unmapped("key:Static.CanBlockIfShadow.ValidBlocker");
    };
    Ok(combat(&object_word(ctx, word)?, quirk))
}

/// The players a word names, as a reference.
fn player_word(word: Option<&str>) -> Tr<Value> {
    match word {
        None | Some("Player") => Ok(json!("EachPlayer")),
        Some("You") => Ok(json!("You")),
        Some("Opponent" | "Player.Opponent") => Ok(json!("Opponents")),
        Some("Player.EnchantedBy") => Ok(json!("AttachedPlayer")),
        Some(other) => super::players_where::word(other)
            .map_or_else(|| unmapped(format!("static-affected:{other}")), Ok),
    }
}

fn players(p: &Params, mode: &str, key: &str) -> Tr<Value> {
    keys(p, mode, &[key])?;
    player_word(p.get(key))
}

/// The objects a valid-string names, as a reference.
fn object_word(ctx: &Ctx, word: &str) -> Tr<Value> {
    Ok(match word {
        "Card.Self" | "Creature.Self" | "Permanent.Self" => json!("This"),
        "Card.EnchantedBy" | "Creature.EnchantedBy" | "Card.EquippedBy" | "Creature.EquippedBy" => {
            json!("Attached")
        }
        other => json!({"Each": selector_any(ctx, other)?}),
    })
}

fn objects(ctx: &Ctx, p: &Params, mode: &str, key: &str) -> Tr<Value> {
    keys(p, mode, &[key])?;
    match p.get(key) {
        Some(word) => object_word(ctx, word),
        None => unmapped(format!("key:Static.{mode}.no-{key}")),
    }
}

/// `CantPutCounter` of any kind on objects, or on players.
fn put_counter(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(
        p,
        "CantPutCounter",
        &["ValidCard", "ValidPlayer", "CounterType"],
    )?;
    let who = match (p.get("ValidCard"), p.get("ValidPlayer")) {
        (Some(word), None) => object_word(ctx, word)?,
        (None, Some(word)) => player_word(Some(word))?,
        _ => return unmapped("key:Static.CantPutCounter.subject"),
    };
    let Some(kind) = p.get("CounterType") else {
        return Ok(cant(&who, &json!("PutCounter")));
    };
    let Some(kind) = super::amount::counter_kind(kind) else {
        return unmapped("key:Static.CantPutCounter.CounterType");
    };
    Ok(cant(&who, &json!({"PutCounterOf": kind})))
}

/// `CantTarget`: the objects can't be the target of spells and abilities from the sources `ValidSource$` names.
fn cant_target(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(
        p,
        "CantTarget",
        &["ValidTarget", "ValidSource", "ValidSA", "Activator"],
    )?;
    let Some(target) = p.get("ValidTarget") else {
        return unmapped("key:Static.CantTarget.no-ValidTarget");
    };
    let who = match target {
        "You" | "Player" | "Opponent" | "Player.Opponent" => player_word(Some(target))?,
        word => object_word(ctx, word)?,
    };
    let mut sources = match p.get("ValidSource") {
        Some(valid) => selector_any(ctx, valid)?,
        None => Vec::new(),
    };
    match p.get("ValidSA") {
        None => {}
        Some("Spell") => sources.push(json!("Spell")),
        Some(_) => return unmapped("key:Static.CantTarget.ValidSA"),
    }
    // "Spells or abilities your opponents control": the source of a spell or ability is controlled by its controller. It
    // is no hexproof, whose opponents are those of the permanent's controller (CR 702.11b).
    match p.get("Activator") {
        None => {}
        Some("Opponent") => sources.push(json!({"ControlledBy": "Opponents"})),
        Some(other) => return unmapped(format!("key:Static.CantTarget.Activator={other}")),
    }
    Ok(cant(&who, &json!({"BeTargeted": sources})))
}

/// `CantAttach`: the `ValidCard$` attachments can't be attached to the `Target$` permanents ("enchanted creature can't be
/// enchanted by other Auras", "~ can be attached only to a creature with toughness 4 or greater").
fn cant_attach(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "CantAttach", &["ValidCard", "Target"])?;
    let (Some(attachment), Some(host)) = (p.get("ValidCard"), p.get("Target")) else {
        return unmapped("key:Static.CantAttach.ValidCard-and-Target");
    };
    let who = object_word(ctx, host)?;
    Ok(cant(
        &who,
        &json!({"BeAttachedBy": selector_any(ctx, attachment)?}),
    ))
}

/// `AttackRestrict`: no more than `MaxAttackers$` creatures can attack you each combat (CR 508.1c); a limit on the whole
/// declaration or at other defenders is another rule.
fn attack_restrict(p: &Params) -> Tr<Value> {
    keys(p, "AttackRestrict", &["MaxAttackers", "ValidDefender"])?;
    if p.get("ValidDefender") != Some("You") {
        return unmapped("key:Static.AttackRestrict.ValidDefender");
    }
    match p.get("MaxAttackers").and_then(|n| n.parse::<u32>().ok()) {
        Some(most) if most >= 1 => Ok(json!({"AttackLimitAt": {"most": most, "at": "You"}})),
        _ => unmapped("key:Static.AttackRestrict.MaxAttackers"),
    }
}

fn untap_other_players(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "UntapOtherPlayer", &["ValidCard"])?;
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Permanent"))?;
    Ok(json!({"UntapDuring": {"what": what, "whose": "Opponents"}}))
}

/// A combat permission for the creatures `who` names.
fn combat(who: &Value, quirk: impl Into<Value>) -> Value {
    json!({"Combat": {"who": who, "quirk": Into::<Value>::into(quirk)}})
}

/// `TapPowerValue`: the creatures count their toughness (`Value$ Toughness`) or `Value$ N` more power when tapped to crew
/// a Vehicle or saddle a Mount. A line that reaches stations or spacecraft is another rule.
fn tap_power_value(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "TapPowerValue", &["ValidSA", "ValidCard", "Value"])?;
    let crews = p.get("ValidSA").is_some_and(|sa| {
        sa.split(',')
            .all(|part| matches!(part, "Activated.Crew+Vehicle" | "Activated.Saddle+Mount"))
    });
    if !crews {
        return unmapped("static:TapPowerValue.ValidSA");
    }
    let value = match p.get("Value") {
        Some("Toughness") => json!("Toughness"),
        Some(n) => match n.parse::<u32>() {
            Ok(n) => json!({"Plus": n}),
            Err(_) => return unmapped("static:TapPowerValue.Value"),
        },
        None => return unmapped("static:TapPowerValue.no-Value"),
    };
    let who = match p.get("ValidCard") {
        Some(word) => object_word(ctx, word)?,
        None => return unmapped("static:TapPowerValue.no-ValidCard"),
    };
    Ok(combat(&who, json!({"TapPower": value})))
}

/// `IgnoreLandWalk`: creatures with a landwalk ability (`ValidKeyword$ Landwalk:Forest`, any when absent) can be blocked
/// as though they did not have it, or the `ValidBlocker$` creatures can block them as though they did not.
fn ignore_landwalk(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(
        p,
        "IgnoreLandWalk",
        &["ValidKeyword", "ValidBlocker", "Secondary"],
    )?;
    match (p.get("ValidKeyword"), p.get("ValidBlocker")) {
        (Some(word), None) => {
            let Some(land) = word.strip_prefix("Landwalk:") else {
                return unmapped("key:Static.IgnoreLandWalk.ValidKeyword");
            };
            let land = json!([{"Subtype": land}]);
            let who = json!({"Each": [{"HasKeyword": {"Landwalk": land}}]});
            Ok(combat(&who, json!({"IgnoreLandwalk": land})))
        }
        (None, None) => {
            let who = json!({"Each": [{"Type": "Creature"}]});
            Ok(combat(&who, "IgnoreAllLandwalk"))
        }
        (None, Some(word)) => Ok(combat(&object_word(ctx, word)?, "BlockIgnoringLandwalk")),
        _ => unmapped("key:Static.IgnoreLandWalk.shape"),
    }
}

/// `IgnoreHexproof`: spells and abilities the `Activator$` players control may target the `ValidEntity$` permanents and
/// players as though they did not have hexproof. A line that also frees its targets from ward is a gap.
fn ignore_hexproof(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "IgnoreHexproof", &["Activator", "ValidEntity"])?;
    if p.get("Description")
        .is_some_and(|text| text.contains("Ward"))
    {
        return unmapped("static:IgnoreHexproof-ward");
    }
    let by = match p.get("Activator") {
        None => json!("EachPlayer"),
        Some("You") => json!("You"),
        Some(other) => return unmapped(format!("key:Static.IgnoreHexproof.Activator={other}")),
    };
    let Some(entity) = p.get("ValidEntity") else {
        return unmapped("key:Static.IgnoreHexproof.no-ValidEntity");
    };
    let (mut permanents, mut players) = (Vec::new(), false);
    for word in entity.split(',') {
        match word {
            "Opponent" => players = true,
            other => permanents.extend(selector_any(ctx, other)?),
        }
    }
    let mut rule = json!({"by": by});
    if !permanents.is_empty() {
        rule["what"] = json!({"Each": permanents});
    }
    if players {
        rule["players"] = json!("Opponents");
    }
    Ok(json!({"IgnoreHexproof": rule}))
}

/// `SetMaxHandSize$` of the players `Affected$` names.
fn hand_size(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "Continuous", &["Affected", "SetMaxHandSize"])?;
    let who = player_word(p.get("Affected"))?;
    match p.get("SetMaxHandSize") {
        Some("Unlimited") => Ok(json!({"NoMaxHandSize": who})),
        Some(size) => Ok(json!({"MaxHandSize": {"who": who, "size": amount(ctx, size)?}})),
        None => unmapped("key:Static.Continuous.SetMaxHandSize"),
    }
}

/// `RaiseMaxHandSize$` of the players `Affected$` names: their maximum hand size grows by that many cards (shrinks when negative).
fn hand_size_change(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "Continuous", &["Affected", "RaiseMaxHandSize"])?;
    let who = player_word(p.get("Affected"))?;
    let by = amount(ctx, p.get("RaiseMaxHandSize").unwrap_or_default())?;
    Ok(json!({"HandSizeChange": {"who": who, "by": by}}))
}

/// `AdjustLandPlays$` of the players `Affected$` names: that many more land plays on each of their turns.
fn land_plays(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(
        p,
        "Continuous",
        &["Affected", "AdjustLandPlays", "Secondary"],
    )?;
    let who = player_word(p.get("Affected"))?;
    match p.get("AdjustLandPlays") {
        Some("Unlimited") | None => unmapped("key:Static.Continuous.AdjustLandPlays=Unlimited"),
        Some(count) => {
            Ok(json!({"AdditionalLandPlays": {"who": who, "count": amount(ctx, count)?}}))
        }
    }
}

/// `CombatDamageToughness`: every creature of the players assigns combat damage equal to its toughness. A line that
/// limits it to some creatures (those with toughness above power, with defender) would change more than it prints.
fn toughness_damage(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "CombatDamageToughness", &["ValidCard"])?;
    let who = match p.get("ValidCard") {
        Some("Creature.YouCtrl") => "You",
        Some("Creature") => "EachPlayer",
        // Some creatures only: a rule of those creatures, not of the players.
        Some(other) => return Ok(combat(&object_word(ctx, other)?, "AssignsByToughness")),
        None => return unmapped("key:Static.CombatDamageToughness.no-ValidCard"),
    };
    Ok(json!({"ToughnessAssignsDamage": who}))
}

/// `AssignCombatDamageAsUnblocked`: the controller may have the creatures assign their combat damage as though they were
/// not blocked. Only the optional form is the rule the engine asks about.
fn assign_as_unblocked(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(
        p,
        "AssignCombatDamageAsUnblocked",
        &["ValidCard", "Optional"],
    )?;
    if p.get("Optional") != Some("True") {
        return unmapped("key:Static.AssignCombatDamageAsUnblocked.mandatory");
    }
    let Some(word) = p.get("ValidCard") else {
        return unmapped("key:Static.AssignCombatDamageAsUnblocked.no-ValidCard");
    };
    Ok(combat(&object_word(ctx, word)?, "AssignsAsUnblocked"))
}

/// `CountersRemain`: the card keeps its counters as it moves to any zone but a hand or a library.
fn counters_remain(p: &Params) -> Tr<Value> {
    check_keys(
        p,
        "Static.CountersRemain",
        &["ValidCard", "EffectZone"],
        |key, _| key == "Mode",
    )?;
    if p.get("ValidCard") != Some("Card.Self") || p.get("EffectZone") != Some("All") {
        return unmapped("key:Static.CountersRemain.shape");
    }
    Ok(json!("CountersRemain"))
}

/// `Affected$ Player` with `AddKeyword$ CantSearchLibrary`: the players named can't search libraries.
fn cant_search(p: &Params) -> Tr<Value> {
    keys(p, "Continuous", &["Affected", "AddKeyword"])?;
    match p.get("Affected") {
        Some(word @ ("Player" | "Opponent")) => Ok(json!({"CantSearch": player_word(Some(word))?})),
        _ => unmapped("static-affected:CantSearchLibrary"),
    }
}

/// `Affected$ You` with `AdditionalOptionalVote$ 1`: while voting, you may vote an additional time (CR 701.38d). A
/// vote the player must cast (`AdditionalVote$`) or more than one stays a gap.
fn e14c_vote_again(p: &Params) -> Tr<Value> {
    keys(p, "Continuous", &["Affected", "AdditionalOptionalVote"])?;
    match (p.get("Affected"), p.get("AdditionalOptionalVote")) {
        (Some("You"), Some("1")) => Ok(json!({"VoteAgain": "You"})),
        _ => unmapped("key:Static.Continuous.AdditionalOptionalVote"),
    }
}

/// `BeginTurn` replacement of extra turns: "if a player would begin an extra turn, that player skips that turn instead".
pub fn skip_extra_turns(p: &Params) -> Tr<Value> {
    check_keys(
        p,
        "Replacement.BeginTurn",
        &["Event", "ActiveZones", "ValidPlayer", "ExtraTurn", "Skip"],
        |_, _| false,
    )?;
    if p.get("ExtraTurn") != Some("True")
        || p.get("Skip") != Some("True")
        || p.get("ActiveZones") != Some("Battlefield")
    {
        return unmapped("replacement:BeginTurn");
    }
    let who = match p.get("ValidPlayer") {
        None => "EachPlayer",
        Some("Opponent") => "Opponents",
        Some(_) => return unmapped("replacement:BeginTurn-player"),
    };
    Ok(json!({"Rule": {"rule": {"SkipExtraTurns": who}}}))
}

/// `IgnoreLegendRule`: the legend rule stops for every player, or for the controller when `ValidCard$` is "permanents you
/// control", or for a narrower set of permanents ("Slivers you control", "tokens you control") judged for the controller.
fn ignore_legend_rule(ctx: &Ctx, p: &Params, mode: &str) -> Tr<Value> {
    keys(p, mode, &["ValidCard"])?;
    let who = match p.get("ValidCard") {
        None => "EachPlayer",
        Some("Permanent.YouCtrl" | "Card.YouCtrl") => "You",
        Some(valid) => return Ok(json!({"NoLegendRuleFor": selector_any(ctx, valid)?})),
    };
    Ok(json!({"NoLegendRule": who}))
}
