//! The bulk read endpoints: the pending drafts whose clauses read back as other text only in wording
//! (`mtg_cards::wording`), listed for a signed-in account, and its approval of the ones it checked as one `correct`
//! verdict per card on the hash the page showed (`docs/server.md`, Bulk reads).

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

use axum::Json;
use axum::extract::{Request, State};
use axum::http::HeaderMap;
use mtg_cards::ReadState;
use mtg_cards::wording::{ClauseMatch, Rule, bulk_candidate};
use mtg_db::{Card, DraftKey, Verdict};
use mtg_import::ids::OracleId;
use mtg_protocol::rest::{
    BulkApprovalBody, BulkApprovalResult, BulkCard, BulkClause, BulkReads, BulkRule,
    MAX_BULK_APPROVALS,
};

use crate::api::{ApiError, read_json_limited};
use crate::card_api::failed;
use crate::cards::{Rules, printed_card};
use crate::reads::{Session, card_draft, session};
use crate::server::AppState;

/// Largest body of an approval: [`MAX_BULK_APPROVALS`] cards of an oracle id and a hash each.
const BODY_LIMIT: usize = MAX_BULK_APPROVALS * 128;

/// The match of each draft by `read_hash`, which fixes both the rules and the printed text, so it never goes stale.
type Matches = Mutex<HashMap<String, Option<Arc<Vec<ClauseMatch>>>>>;

fn matches() -> &'static Matches {
    static MATCHES: OnceLock<Matches> = OnceLock::new();
    MATCHES.get_or_init(Matches::default)
}

/// The match of the draft with `read_hash`, from the cache or worked out once.
fn matched(
    read_hash: &str,
    work: impl FnOnce() -> Option<Vec<ClauseMatch>>,
) -> Option<Arc<Vec<ClauseMatch>>> {
    let known = matches()
        .lock()
        .ok()
        .and_then(|map| map.get(read_hash).cloned());
    if let Some(found) = known {
        return found;
    }
    let found = work().map(Arc::new);
    if let Ok(mut map) = matches().lock() {
        map.insert(read_hash.to_owned(), found.clone());
    }
    found
}

/// The wire form of one candidate.
fn wire_card(id: &str, name: &str, read_hash: &str, clauses: &[ClauseMatch]) -> BulkCard {
    let rule = clauses
        .first()
        .and_then(|clause| clause.rules.first())
        .map_or("", |rule| rule.name());
    BulkCard {
        oracle_id: id.to_owned(),
        name: name.to_owned(),
        read_hash: read_hash.to_owned(),
        rule: rule.to_owned(),
        clauses: clauses
            .iter()
            .map(|clause| BulkClause {
                printed: clause.printed.clone(),
                engine: clause.written.clone(),
                rules: clause
                    .rules
                    .iter()
                    .map(|rule| rule.name().to_owned())
                    .collect(),
            })
            .collect(),
    }
}

impl Session<'_> {
    /// The candidates: drafts marked `differs` that decide their card, pending, not judged by this account on the
    /// current hash, and whose every differing clause matches; by name.
    async fn candidates(&self) -> Result<Vec<BulkCard>, ApiError> {
        let rules = self.cards.rules().await?;
        let ids: Vec<OracleId> = rules
            .library()
            .drafts()
            .map(|(_, draft)| draft)
            .filter(|draft| draft.differs && draft.proof.is_none())
            .map(|draft| OracleId(draft.card.oracle_id.clone()))
            .collect();
        let db = self.cards.database();
        let waiting: HashSet<String> = db
            .reads()
            .queue(self.user, &ids)
            .await
            .map_err(|error| failed(&error))?
            .into_iter()
            .filter(|row| row.state == mtg_db::ReadState::Pending)
            .map(|row| row.read_hash)
            .collect();
        let stored = db
            .cards()
            .get_many(&ids)
            .await
            .map_err(|error| failed(&error))?;
        // Reading back every draft takes seconds to minutes; it runs on the blocking pool so requests and
        // `/healthz` keep answering.
        tokio::task::spawn_blocking(move || judge(&rules, &stored, &waiting))
            .await
            .map_err(|error| failed(&error))
    }
}

/// The candidates among `stored`: pending on a hash in `waiting`, every differing clause matched; by name.
fn judge(rules: &Rules, stored: &[Card], waiting: &HashSet<String>) -> Vec<BulkCard> {
    let mut cards = Vec::new();
    for card in stored {
        let Some(current) = card_draft(rules, card) else {
            continue;
        };
        if current.state != ReadState::Pending || !waiting.contains(&current.read_hash) {
            continue;
        }
        let Some(draft) = rules.library().draft(card.oracle_id.as_str()) else {
            continue;
        };
        let found = matched(&current.read_hash, || {
            bulk_candidate(&draft.differs(&printed_card(card)))
        });
        if let Some(clauses) = found {
            cards.push(wire_card(
                card.oracle_id.as_str(),
                &card.name,
                &current.read_hash,
                &clauses,
            ));
        }
    }
    cards.sort_by(|a, b| a.name.cmp(&b.name).then(a.oracle_id.cmp(&b.oracle_id)));
    cards
}

/// The rules the listed cards need, in their fixed order.
fn rules_of(cards: &[BulkCard]) -> Vec<BulkRule> {
    let used: HashSet<&str> = cards
        .iter()
        .flat_map(|card| &card.clauses)
        .flat_map(|clause| clause.rules.iter().map(String::as_str))
        .collect();
    Rule::ALL
        .iter()
        .filter(|rule| used.contains(rule.name()))
        .map(|rule| BulkRule {
            name: rule.name().to_owned(),
            title: rule.title().to_owned(),
            summary: rule.summary().to_owned(),
        })
        .collect()
}

/// `GET /api/reads/bulk`: the cards the account may approve in bulk, by name, with the rules they need.
pub(crate) async fn list(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<BulkReads>, ApiError> {
    let s = session(&app, &headers).await?;
    let cards = s.candidates().await?;
    Ok(Json(BulkReads {
        rules: rules_of(&cards),
        cards,
    }))
}

/// `PUT /api/reads/bulk`: one `correct` verdict per named card that is still a candidate on the named hash; the
/// others come back as stale and get nothing.
pub(crate) async fn approve(
    State(app): State<AppState>,
    request: Request,
) -> Result<Json<BulkApprovalResult>, ApiError> {
    if app.stopping() {
        return Err(ApiError::unavailable());
    }
    let headers = request.headers().clone();
    let s = session(&app, &headers).await?;
    let body: BulkApprovalBody = read_json_limited(request, BODY_LIMIT).await?;
    if body.cards.is_empty() || body.cards.len() > MAX_BULK_APPROVALS {
        return Err(ApiError::invalid(format!(
            "name between 1 and {MAX_BULK_APPROVALS} cards"
        )));
    }
    let current: HashMap<String, String> = s
        .candidates()
        .await?
        .into_iter()
        .map(|card| (card.oracle_id, card.read_hash))
        .collect();
    let reads = s.cards.database().reads();
    let mut result = BulkApprovalResult {
        approved: Vec::new(),
        stale: Vec::new(),
    };
    let mut seen = HashSet::new();
    for card in body.cards {
        if !seen.insert(card.oracle_id.clone()) {
            continue;
        }
        if current.get(&card.oracle_id) != Some(&card.read_hash) {
            result.stale.push(card.oracle_id);
            continue;
        }
        let key = DraftKey {
            oracle_id: OracleId(card.oracle_id.clone()),
            read_hash: card.read_hash,
        };
        let stored = reads
            .append(Some(s.user), &key, Verdict::Correct, None)
            .await
            .map_err(|error| failed(&error))?;
        if stored.is_some() {
            result.approved.push(card.oracle_id);
        } else {
            result.stale.push(card.oracle_id);
        }
    }
    Ok(Json(result))
}

#[cfg(test)]
mod tests {
    //! Goal: a candidate groups under its first rule, and the rules list keeps the fixed order of the rules in use.
    use super::*;

    #[test]
    fn a_card_groups_under_its_first_rule_and_rules_keep_their_order() {
        let clause = ClauseMatch {
            face: 0,
            printed: "Discard your hand.".into(),
            written: "Discard X cards, where X is the number of cards in your hand.".into(),
            rules: vec![Rule::DiscardHand],
        };
        let card = wire_card(
            "id",
            "Slate of Ancestry",
            "h",
            std::slice::from_ref(&clause),
        );
        assert_eq!(card.rule, "discard_hand");
        let mut other = card.clone();
        other.clauses[0].rules = vec!["self_name".into()];
        let names: Vec<String> = rules_of(&[card, other])
            .into_iter()
            .map(|r| r.name)
            .collect();
        assert_eq!(names, ["self_name", "discard_hand"]);
    }
}
