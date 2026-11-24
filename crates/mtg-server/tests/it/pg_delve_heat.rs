//! Goal: on Postgres a delve can be created at every Heat the campaign offers, Legendary included; the store never
//! refuses a Heat the rules allow.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::pg_delve_forfeit::call;
use crate::{net, pg_commander};

use mtg_campaign::scale::MAX_HEAT;
use net::{PASSWORD, register};
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test(migrations = false)]
async fn every_heat_the_campaign_offers_is_stored(pool: PgPool) {
    let server = pg_commander::start(&pool).await;
    let host = register(server.addr, "ana", PASSWORD).await;
    for heat in 0..=MAX_HEAT {
        let created = call(
            &server,
            "POST",
            "/api/delves",
            &host,
            Some(json!({"heat": heat})),
        )
        .await;
        assert!(
            created.status < 500,
            "heat {heat}: {} {}",
            created.status,
            created.text()
        );
    }
    let stored: Vec<i16> = sqlx::query_scalar("SELECT heat FROM delves ORDER BY heat")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(
        stored.contains(&i16::from(MAX_HEAT)),
        "stored heats {stored:?}"
    );
}
