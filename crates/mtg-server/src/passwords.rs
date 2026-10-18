//! Argon2id password hashing: off the async threads, bounded in how many run at once, and with a
//! stand-in hash so an unknown username costs as much time as a wrong password.
//!
//! The stored form is the PHC string (`$argon2id$v=19$m=..,t=..,p=..$salt$hash`), so the cost
//! that made a hash travels with it and a later change of cost leaves old hashes valid.

use std::fmt;
use std::sync::Arc;

use argon2::password_hash::phc::PasswordHash;
use argon2::{Algorithm, Argon2, Params, PasswordHasher, PasswordVerifier, Version};
use thiserror::Error;
use tokio::sync::{OnceCell, Semaphore};

use crate::secret::SessionToken;

/// Why a cost is not one Argon2 accepts.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid Argon2 cost: {0}")]
pub struct HashCostError(String);

/// How much work one password hash takes: memory in KiB, passes over it and lanes.
///
/// The default is the OWASP minimum for Argon2id (19 MiB, 2 passes, 1 lane), about 30 ms on a
/// modern core. Tests use [`HashCost::cheapest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashCost(Params);

impl HashCost {
    /// A cost of `memory_kib` KiB (at least 8 per lane), `iterations` passes (at least 1) and
    /// `parallelism` lanes (at least 1).
    pub fn new(
        memory_kib: u32,
        iterations: u32,
        parallelism: u32,
    ) -> Result<HashCost, HashCostError> {
        Params::new(memory_kib, iterations, parallelism, None)
            .map(HashCost)
            .map_err(|error| HashCostError(error.to_string()))
    }

    /// The least work Argon2 allows (8 KiB, one pass, one lane): for tests, never for a server.
    pub fn cheapest() -> HashCost {
        let least = Params::new(
            Params::MIN_M_COST,
            Params::MIN_T_COST,
            Params::MIN_P_COST,
            None,
        );
        HashCost(least.unwrap_or(Params::DEFAULT))
    }

    /// Memory in KiB.
    pub fn memory_kib(&self) -> u32 {
        self.0.m_cost()
    }

    /// Passes over the memory.
    pub fn iterations(&self) -> u32 {
        self.0.t_cost()
    }

    /// Lanes.
    pub fn parallelism(&self) -> u32 {
        self.0.p_cost()
    }
}

impl Default for HashCost {
    fn default() -> Self {
        HashCost(Params::DEFAULT)
    }
}

/// Why a hash could not be made.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum HashError {
    /// Argon2 or the random source refused.
    #[error("password hashing failed: {0}")]
    Hash(String),
    /// The blocking task was cancelled or panicked.
    #[error("the hashing task stopped")]
    Stopped,
}

struct Shared {
    argon: Argon2<'static>,
    /// Hashes running at once; each holds its memory for tens of milliseconds.
    permits: Semaphore,
    /// A real hash of a password nobody knows, checked when the username does not exist.
    stand_in: OnceCell<String>,
}

/// Hashes and checks passwords. Cheap to clone.
#[derive(Clone)]
pub(crate) struct Passwords {
    shared: Arc<Shared>,
}

impl fmt::Debug for Passwords {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Passwords").finish_non_exhaustive()
    }
}

impl Passwords {
    /// A hasher with `cost` that runs at most `concurrency` hashes at once (at least one).
    pub(crate) fn new(cost: &HashCost, concurrency: usize) -> Passwords {
        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, cost.0.clone());
        Passwords {
            shared: Arc::new(Shared {
                argon,
                permits: Semaphore::new(concurrency.max(1)),
                stand_in: OnceCell::new(),
            }),
        }
    }

    /// The PHC string for `password`, with a fresh random salt.
    pub(crate) async fn hash(&self, password: String) -> Result<String, HashError> {
        let _permit = self
            .shared
            .permits
            .acquire()
            .await
            .map_err(|_| HashError::Stopped)?;
        let shared = Arc::clone(&self.shared);
        tokio::task::spawn_blocking(move || {
            shared
                .argon
                .hash_password(password.as_bytes())
                .map(|hash| hash.to_string())
                .map_err(|error| HashError::Hash(error.to_string()))
        })
        .await
        .map_err(|_| HashError::Stopped)?
    }

    /// Whether `password` matches `stored`. With no stored hash (an unknown username) it checks
    /// the stand-in instead, so the time taken is the same, and always answers `false`.
    pub(crate) async fn verify(&self, password: String, stored: Option<String>) -> bool {
        let real = stored.is_some();
        let phc = match stored {
            Some(phc) => phc,
            None => self.stand_in().await,
        };
        let Ok(_permit) = self.shared.permits.acquire().await else {
            return false;
        };
        let shared = Arc::clone(&self.shared);
        let matched = tokio::task::spawn_blocking(move || {
            // A stored value that does not parse (a damaged row) matches nothing.
            PasswordHash::new(&phc).is_ok_and(|parsed| {
                shared
                    .argon
                    .verify_password(password.as_bytes(), &parsed)
                    .is_ok()
            })
        })
        .await
        .unwrap_or(false);
        real && matched
    }

    /// Computes the stand-in now, so the first unknown username is not slower than the rest.
    pub(crate) async fn warm_up(&self) {
        self.stand_in().await;
    }

    /// The PHC string the unknown-user path checks against: made with the current cost from a
    /// random password that is thrown away.
    async fn stand_in(&self) -> String {
        let made = self
            .shared
            .stand_in
            .get_or_try_init(|| async {
                let secret =
                    SessionToken::generate().map_err(|error| HashError::Hash(error.to_string()))?;
                self.hash(secret.expose().to_owned()).await
            })
            .await;
        match made {
            Ok(phc) => phc.clone(),
            Err(error) => {
                // Nothing is cached, so the next unknown username tries again; this one is
                // checked against nothing and so fails a little faster than it should.
                tracing::error!(%error, "could not make the stand-in password hash");
                String::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: hashes verify only their own password, carry the configured cost and a fresh salt, damaged or missing hashes never match and cost a check anyway, the cost rules follow Argon2's, and many hashes at once with one permit all finish.
    use std::collections::HashSet;

    use super::*;
    use rstest::rstest;

    fn cheap() -> Passwords {
        Passwords::new(&HashCost::cheapest(), 2)
    }

    #[tokio::test]
    async fn a_hash_verifies_its_password_and_nothing_else() {
        let passwords = cheap();
        let phc = passwords
            .hash("correct horse battery".into())
            .await
            .unwrap();
        assert!(
            passwords
                .verify("correct horse battery".into(), Some(phc.clone()))
                .await
        );
        for wrong in [
            "correct horse batterY",
            "correct horse battery ",
            "",
            "Correct horse battery",
        ] {
            assert!(
                !passwords.verify(wrong.into(), Some(phc.clone())).await,
                "{wrong:?}"
            );
        }
    }

    #[rstest]
    #[case::ascii("plain old password")]
    #[case::unicode("pässwörd 日本語 🙂")]
    #[case::spaces("          ")]
    #[case::nul("with\0nul inside")]
    #[case::long(&"x".repeat(4096))]
    #[tokio::test]
    async fn any_password_round_trips(#[case] password: &str) {
        let passwords = cheap();
        let phc = passwords.hash(password.into()).await.unwrap();
        assert!(passwords.verify(password.into(), Some(phc.clone())).await);
        assert!(!passwords.verify(format!("{password}!"), Some(phc)).await);
    }

    #[tokio::test]
    async fn hashes_are_argon2id_with_the_configured_cost_and_never_hold_the_password() {
        let cost = HashCost::new(64, 2, 1).unwrap();
        let passwords = Passwords::new(&cost, 1);
        let phc = passwords.hash("a secret passphrase".into()).await.unwrap();
        assert!(phc.starts_with("$argon2id$v=19$m=64,t=2,p=1$"), "{phc}");
        assert!(!phc.contains("secret"));
        // The cost in the string is what verification uses, whatever the hasher is set to now.
        let other = Passwords::new(&HashCost::cheapest(), 1);
        assert!(other.verify("a secret passphrase".into(), Some(phc)).await);
    }

    #[tokio::test]
    async fn every_hash_has_a_salt_of_its_own() {
        let passwords = cheap();
        let mut seen = HashSet::new();
        for _ in 0..20 {
            seen.insert(passwords.hash("the same password".into()).await.unwrap());
        }
        assert_eq!(seen.len(), 20);
    }

    #[rstest]
    #[case::empty("")]
    #[case::words("not a hash")]
    #[case::wrong_algorithm(
        "$argon2i$v=19$m=8,t=1,p=1$c29tZXNhbHRzYWx0$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
    )]
    #[case::no_salt("$argon2id$v=19$m=8,t=1,p=1")]
    #[case::truncated("$argon2id$v=19$m=8,t=1,p=1$c29tZXNhbHRzYWx0$AAAA")]
    #[case::bcrypt("$2b$12$abcdefghijklmnopqrstuuABCDEFGHIJKLMNOPQRSTUVWXYZ01234")]
    #[tokio::test]
    async fn a_damaged_or_foreign_hash_matches_nothing(#[case] stored: &str) {
        let passwords = cheap();
        assert!(
            !passwords
                .verify("any password at all".into(), Some(stored.into()))
                .await
        );
    }

    #[tokio::test]
    async fn an_unknown_user_never_matches_even_with_the_stand_in_password() {
        let passwords = cheap();
        assert!(!passwords.verify("any password at all".into(), None).await);
        // The stand-in is a real hash made with the current cost.
        let stand_in = passwords.stand_in().await;
        assert!(
            stand_in.starts_with("$argon2id$v=19$m=8,t=1,p=1$"),
            "{stand_in}"
        );
        // Nothing but the thrown-away secret matches it, and even that answers false without a user.
        assert!(!passwords.verify(stand_in.clone(), None).await);
        assert!(!passwords.verify(String::new(), None).await);
    }

    #[tokio::test]
    async fn the_stand_in_is_made_once() {
        let passwords = cheap();
        passwords.warm_up().await;
        let first = passwords.stand_in().await;
        let clones = [passwords.clone(), passwords.clone()];
        for clone in clones {
            assert_eq!(clone.stand_in().await, first);
        }
    }

    #[tokio::test]
    async fn a_single_permit_still_lets_every_request_finish() {
        let passwords = Passwords::new(&HashCost::cheapest(), 1);
        let phc = passwords
            .hash("shared by all of them".into())
            .await
            .unwrap();
        let tasks: Vec<_> = (0..12)
            .map(|n| {
                let (passwords, phc) = (passwords.clone(), phc.clone());
                tokio::spawn(async move {
                    match n % 3 {
                        0 => passwords.hash(format!("password number {n}")).await.is_ok(),
                        1 => {
                            passwords
                                .verify("shared by all of them".into(), Some(phc))
                                .await
                        }
                        _ => !passwords.verify("shared by all of them".into(), None).await,
                    }
                })
            })
            .collect();
        for task in tasks {
            assert!(task.await.unwrap());
        }
    }

    #[rstest]
    #[case::cheapest(8, 1, 1, true)]
    #[case::default_cost(19_456, 2, 1, true)]
    #[case::two_lanes(16, 1, 2, true)]
    #[case::too_little_memory(7, 1, 1, false)]
    #[case::memory_per_lane(15, 1, 2, false)]
    #[case::no_memory(0, 1, 1, false)]
    #[case::no_passes(8, 0, 1, false)]
    #[case::no_lanes(8, 1, 0, false)]
    fn costs_follow_the_rules_of_argon2(
        #[case] memory: u32,
        #[case] passes: u32,
        #[case] lanes: u32,
        #[case] valid: bool,
    ) {
        assert_eq!(HashCost::new(memory, passes, lanes).is_ok(), valid);
    }

    #[test]
    fn the_default_is_the_owasp_minimum_and_the_cheapest_is_argons_floor() {
        let default = HashCost::default();
        assert_eq!(
            (
                default.memory_kib(),
                default.iterations(),
                default.parallelism()
            ),
            (19_456, 2, 1)
        );
        let cheapest = HashCost::cheapest();
        assert_eq!(
            (
                cheapest.memory_kib(),
                cheapest.iterations(),
                cheapest.parallelism()
            ),
            (8, 1, 1)
        );
    }

    #[test]
    fn debug_output_shows_nothing_of_the_hasher() {
        let shown = format!("{:?}", cheap());
        assert_eq!(shown, "Passwords { .. }");
    }
}
