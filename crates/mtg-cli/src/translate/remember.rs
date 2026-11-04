//! The `Remember*` keys of a step that moves objects: the objects it moved are bound to a name that `Defined$ Remembered`
//! reads afterwards.

use serde_json::{Value, json};

use super::ctx::Ctx;
use crate::forge::script::Params;

/// Whether the step asks to remember what it moved, by any of the `keys` set to `True`.
pub fn wanted(p: &Params, keys: &[&str]) -> bool {
    keys.iter().any(|key| p.get(key) == Some("True"))
}

/// Names the objects the step `verb` moved (`as`) and makes them what `Remembered` stands for in the lines after it.
pub fn bind_moved(ctx: &mut Ctx, effect: &mut Value, verb: &str, stem: &str) {
    let name = ctx.fresh(stem);
    ctx.remembered = Some(json!({ "Bound": name }));
    effect[verb]["as"] = json!(name);
}
