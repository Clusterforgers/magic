//! The ability header: trigger words the header grammar reads.

use serde_json::Value;

use super::ctx::{Ctx, Tr};
use crate::forge::script::Params;

/// The trigger pattern of a `Mode$` word read here; `None` until a word is read.
pub fn trigger_words(_ctx: &Ctx, _p: &Params, _mode: &str) -> Option<Tr<Value>> {
    None
}
