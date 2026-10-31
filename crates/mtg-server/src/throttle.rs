//! Limits on guessing and polling: how many failed logins one address and one username may cause,
//! how many accounts one address may try to register, how many join codes one address may miss and
//! how often it may ask for the lobby list or the precons, per window. Kept in memory, per process.
//!
//! Fixed windows: a counter starts at the first attempt and is forgotten one window later. A
//! login or a code lookup is counted when it starts, so concurrent guesses cannot all slip under
//! the limit, and handed back when it succeeds. A blocked attempt is not counted, so waiting out
//! the window is enough. The username counter exists whether or not the account does, so the limit
//! reveals nothing about it. Code misses and the lobby list have windows of their own, shorter
//! than the others.

use std::collections::HashMap;
use std::fmt;
use std::net::{IpAddr, Ipv6Addr};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use crate::config::AttemptLimits;
use crate::user_id::UserId;

/// Most counters kept. Past this, a key that is not tracked yet goes unlimited until a sweep
/// frees room, which takes a flood from tens of thousands of distinct addresses.
const MAX_TRACKED: usize = 50_000;
/// Size at which the first sweep of expired counters runs.
const FIRST_SWEEP: usize = 1024;

/// What a counter counts. Nothing here is `Debug`: the keys hold usernames, and a name tried in a
/// failed login may be a password typed into the wrong box.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Key {
    /// Logins from one network that were started and not (yet) successful.
    LoginAddress(IpAddr),
    /// Logins for one username (lower case) that were started and not (yet) successful.
    LoginName(String),
    /// Registrations tried from one network.
    RegisterAddress(IpAddr),
    /// Accounts one admin account made through the admin route.
    AdminCreate(UserId),
    /// Join code lookups from one network that were started and did not (yet) find a game.
    CodeAddress(IpAddr),
    /// Requests for the lobby list from one network, answered or not.
    ListAddress(IpAddr),
    /// Requests for the precon list or one precon from one network, answered or not.
    PreconAddress(IpAddr),
}

/// How long a counter of this kind lasts.
fn window_of(limits: &AttemptLimits, key: &Key) -> Duration {
    match key {
        Key::CodeAddress(_) => limits.code_window,
        Key::ListAddress(_) | Key::PreconAddress(_) => limits.lobby_window,
        Key::LoginAddress(_)
        | Key::LoginName(_)
        | Key::RegisterAddress(_)
        | Key::AdminCreate(_) => limits.window,
    }
}

#[derive(Clone, Copy)]
struct Counter {
    started: Instant,
    count: u32,
}

#[derive(Default)]
struct State {
    counters: HashMap<Key, Counter>,
    /// Size at which expired counters are swept next.
    sweep_at: usize,
}

impl State {
    /// Drops expired counters once the table has doubled since the last sweep.
    fn sweep(&mut self, now: Instant, limits: &AttemptLimits) {
        if self.counters.len() < self.sweep_at.max(FIRST_SWEEP) {
            return;
        }
        self.counters.retain(|key, counter| {
            now.saturating_duration_since(counter.started) < window_of(limits, key)
        });
        self.sweep_at = self.counters.len().saturating_mul(2);
    }
}

/// The caller is over a limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blocked {
    /// How long until the longest of the limits it hit resets.
    pub retry_after: Duration,
}

impl Blocked {
    /// `retry_after` in whole seconds, rounded up and never zero, for the `Retry-After` header.
    pub fn seconds(&self) -> u64 {
        let whole = self.retry_after.as_secs();
        if self.retry_after.subsec_nanos() > 0 {
            whole + 1
        } else {
            whole.max(1)
        }
    }
}

/// A login that was counted when it began; pass it to [`Throttle::succeeded`] or
/// [`Throttle::abandoned`] to take the count back.
pub(crate) struct Attempt {
    counted: Vec<Key>,
    exhausted: bool,
}

impl Attempt {
    /// Whether this was the last try the address or the username gets in this window.
    pub(crate) fn exhausted(&self) -> bool {
        self.exhausted
    }
}

impl fmt::Debug for Attempt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Attempt")
            .field("exhausted", &self.exhausted)
            .finish_non_exhaustive()
    }
}

/// The counters and their limits.
pub(crate) struct Throttle {
    limits: AttemptLimits,
    state: Mutex<State>,
}

impl fmt::Debug for Throttle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Throttle")
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl Throttle {
    /// A throttle with no history.
    pub(crate) fn new(limits: AttemptLimits) -> Throttle {
        Throttle {
            limits,
            state: Mutex::new(State::default()),
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        // The table is never left half-updated, so a poisoned lock is still sound.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Counts a login by `username` (the lower-case key) from `address`, or says to wait.
    pub(crate) fn begin_login(
        &self,
        now: Instant,
        address: IpAddr,
        username: &str,
    ) -> Result<Attempt, Blocked> {
        let wanted = [
            (
                Key::LoginAddress(network(address)),
                self.limits.login_per_ip,
            ),
            (
                Key::LoginName(username.to_owned()),
                self.limits.login_per_username,
            ),
        ];
        self.admit(now, &wanted)
            .map(|(counted, exhausted)| Attempt { counted, exhausted })
    }

    /// Counts a lookup of a join code from `address`, or says to wait. Pass the attempt to
    /// [`Throttle::succeeded`] when the code was right, so only misses use up the budget.
    pub(crate) fn begin_code(&self, now: Instant, address: IpAddr) -> Result<Attempt, Blocked> {
        let wanted = [(
            Key::CodeAddress(network(address)),
            self.limits.code_misses_per_ip,
        )];
        self.admit(now, &wanted)
            .map(|(counted, exhausted)| Attempt { counted, exhausted })
    }

    /// Counts a request for the lobby list from `address`, or says to wait. Every request counts,
    /// whatever the answer was, and nothing hands it back.
    pub(crate) fn begin_listing(&self, now: Instant, address: IpAddr) -> Result<(), Blocked> {
        let wanted = [(
            Key::ListAddress(network(address)),
            self.limits.lobby_requests_per_ip,
        )];
        self.admit(now, &wanted).map(|_| ())
    }

    /// Counts a request for the precon list or one precon from `address`, or says to wait. Every
    /// request counts, whatever the answer was, and nothing hands it back.
    pub(crate) fn begin_precons(&self, now: Instant, address: IpAddr) -> Result<(), Blocked> {
        let wanted = [(
            Key::PreconAddress(network(address)),
            self.limits.precon_requests_per_ip,
        )];
        self.admit(now, &wanted).map(|_| ())
    }

    /// Takes back the count of a login that turned out right (the address keeps its other
    /// failures, the username starts afresh) or of a code that found its game.
    pub(crate) fn succeeded(&self, attempt: Attempt) {
        self.release(attempt, true);
    }

    /// Takes back the count of a login the server could not check (the database failed), which
    /// says nothing about whether somebody is guessing.
    pub(crate) fn abandoned(&self, attempt: Attempt) {
        self.release(attempt, false);
    }

    fn release(&self, attempt: Attempt, clear_name: bool) {
        let mut state = self.state();
        for key in attempt.counted {
            if clear_name && matches!(key, Key::LoginName(_)) {
                state.counters.remove(&key);
                continue;
            }
            let spent = state.counters.get_mut(&key).is_some_and(|counter| {
                counter.count = counter.count.saturating_sub(1);
                counter.count == 0
            });
            if spent {
                state.counters.remove(&key);
            }
        }
    }

    /// Counts a registration from `address`, or says to wait. `true` means this was the last
    /// registration the address gets in this window.
    pub(crate) fn begin_register(&self, now: Instant, address: IpAddr) -> Result<bool, Blocked> {
        let wanted = [(
            Key::RegisterAddress(network(address)),
            self.limits.register_per_ip,
        )];
        self.admit(now, &wanted).map(|(_, exhausted)| exhausted)
    }

    /// Counts an account `admin` is about to make through the admin route, or says to wait. It has a budget of
    /// its own per admin account: it never uses up the registrations of an address, and nothing hands it back.
    pub(crate) fn begin_admin_create(&self, now: Instant, admin: UserId) -> Result<(), Blocked> {
        let wanted = [(
            Key::AdminCreate(admin),
            self.limits.admin_creates_per_account,
        )];
        self.admit(now, &wanted).map(|_| ())
    }

    /// Counts one attempt on every key in `wanted` if none is at its limit; returns the keys
    /// counted and whether any of them has now reached its limit. Nothing is counted when any key
    /// is full.
    fn admit(&self, now: Instant, wanted: &[(Key, u32)]) -> Result<(Vec<Key>, bool), Blocked> {
        let age = |counter: &Counter| now.saturating_duration_since(counter.started);
        let mut state = self.state();
        state.sweep(now, &self.limits);

        let mut wait = None;
        for (key, max) in wanted {
            let window = window_of(&self.limits, key);
            let live = state
                .counters
                .get(key)
                .filter(|counter| age(counter) < window);
            if live.map_or(0, |counter| counter.count) >= *max {
                let left = live.map_or(window, |counter| window.saturating_sub(age(counter)));
                wait = Some(wait.map_or(left, |longest: Duration| longest.max(left)));
            }
        }
        if let Some(retry_after) = wait {
            return Err(Blocked { retry_after });
        }

        let mut counted = Vec::with_capacity(wanted.len());
        let mut exhausted = false;
        for (key, max) in wanted {
            if !state.counters.contains_key(key) && state.counters.len() >= MAX_TRACKED {
                continue;
            }
            let slot = state.counters.entry(key.clone()).or_insert(Counter {
                started: now,
                count: 0,
            });
            if age(slot) >= window_of(&self.limits, key) {
                *slot = Counter {
                    started: now,
                    count: 0,
                };
            }
            slot.count = slot.count.saturating_add(1);
            exhausted |= slot.count >= *max;
            counted.push(key.clone());
        }
        Ok((counted, exhausted))
    }

    /// Counters in the table, expired ones included.
    #[cfg(test)]
    fn tracked(&self) -> usize {
        self.state().counters.len()
    }
}

/// The part of an address one client controls: the whole IPv4 address, the /64 of an IPv6 one
/// (an IPv4 address in IPv6 clothes counts as the IPv4 address).
fn network(address: IpAddr) -> IpAddr {
    match address.to_canonical() {
        IpAddr::V4(v4) => IpAddr::V4(v4),
        IpAddr::V6(v6) => IpAddr::V6(Ipv6Addr::from(u128::from(v6) & (u128::MAX << 64))),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: each limit blocks exactly at its count and lifts exactly one window after the first attempt, blocked attempts cost nothing, a success hands its count back, addresses are grouped the way one client controls them, usernames and addresses do not share budgets, join code misses follow a window of their own, the lobby list follows another, the precon browser counts apart from it, an admin making accounts has a budget of its own, and the table stays bounded.
    use std::net::Ipv4Addr;

    use super::*;
    use rstest::rstest;

    const WINDOW: Duration = Duration::from_secs(60);
    /// Shorter than `WINDOW`, so a test can tell which window a counter follows.
    const CODE_WINDOW: Duration = Duration::from_secs(20);
    /// Shorter than both, so a test can tell the lobby list has a window of its own.
    const LOBBY_WINDOW: Duration = Duration::from_secs(10);

    fn limits() -> AttemptLimits {
        AttemptLimits {
            window: WINDOW,
            login_per_ip: 5,
            login_per_username: 3,
            register_per_ip: 2,
            admin_creates_per_account: 4,
            code_window: CODE_WINDOW,
            code_misses_per_ip: 4,
            lobby_window: LOBBY_WINDOW,
            lobby_requests_per_ip: 6,
            precon_requests_per_ip: 3,
        }
    }

    fn ip(text: &str) -> IpAddr {
        text.parse().unwrap()
    }

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn a_username_is_blocked_at_its_count_and_freed_one_window_after_the_first_attempt() {
        let throttle = Throttle::new(limits());
        let t0 = Instant::now();
        for n in 0..3 {
            let at = t0 + secs(n * 10);
            assert!(throttle.begin_login(at, ip("10.0.0.1"), "ann").is_ok());
        }
        let blocked = throttle.begin_login(t0 + secs(30), ip("10.0.0.2"), "ann");
        assert_eq!(blocked.unwrap_err().retry_after, secs(30));
        // Blocked tries are free and do not push the end of the window out.
        for n in 31..60 {
            let again = throttle.begin_login(t0 + secs(n), ip("10.0.0.3"), "ann");
            assert_eq!(again.unwrap_err().retry_after, secs(60 - n));
        }
        assert!(
            throttle
                .begin_login(t0 + secs(60), ip("10.0.0.4"), "ann")
                .is_ok()
        );
    }

    #[test]
    fn an_address_is_blocked_whatever_names_it_tries() {
        let throttle = Throttle::new(limits());
        let t0 = Instant::now();
        for n in 0..5 {
            let name = format!("user{n}");
            assert!(throttle.begin_login(t0, ip("10.0.0.1"), &name).is_ok());
        }
        let blocked = throttle.begin_login(t0 + secs(1), ip("10.0.0.1"), "somebody-new");
        assert_eq!(blocked.unwrap_err().retry_after, secs(59));
        // Other addresses and other names are untouched.
        assert!(throttle.begin_login(t0, ip("10.0.0.2"), "user0").is_ok());
        assert!(
            throttle
                .begin_login(t0 + secs(60), ip("10.0.0.1"), "x")
                .is_ok()
        );
    }

    #[test]
    fn a_blocked_attempt_counts_against_neither_key() {
        let throttle = Throttle::new(limits());
        let t0 = Instant::now();
        for _ in 0..3 {
            throttle.begin_login(t0, ip("10.0.0.1"), "ann").unwrap();
        }
        // The name is full; these must not use up the address's remaining budget of 2.
        for _ in 0..10 {
            assert!(throttle.begin_login(t0, ip("10.0.0.1"), "ann").is_err());
        }
        assert!(throttle.begin_login(t0, ip("10.0.0.1"), "bob").is_ok());
        assert!(throttle.begin_login(t0, ip("10.0.0.1"), "cy").is_ok());
        assert!(throttle.begin_login(t0, ip("10.0.0.1"), "di").is_err());
    }

    #[test]
    fn retry_after_is_the_longest_of_the_limits_hit() {
        let throttle = Throttle::new(limits());
        let t0 = Instant::now();
        // The name's window starts at t0, from other addresses.
        for _ in 0..3 {
            throttle.begin_login(t0, ip("10.0.0.9"), "ann").unwrap();
        }
        // The address's window starts 20 s later.
        for n in 0..5 {
            let name = format!("user{n}");
            throttle
                .begin_login(t0 + secs(20), ip("10.0.0.1"), &name)
                .unwrap();
        }
        let blocked = throttle.begin_login(t0 + secs(30), ip("10.0.0.1"), "ann");
        // The name frees up in 30 s, the address in 50.
        assert_eq!(blocked.unwrap_err().retry_after, secs(50));
    }

    #[test]
    fn a_success_hands_its_count_back() {
        let throttle = Throttle::new(limits());
        let t0 = Instant::now();
        for _ in 0..200 {
            let attempt = throttle.begin_login(t0, ip("10.0.0.1"), "ann").unwrap();
            throttle.succeeded(attempt);
        }
        assert_eq!(
            throttle.tracked(),
            0,
            "a clean history leaves nothing behind"
        );
    }

    #[test]
    fn a_success_clears_the_name_but_the_address_keeps_its_other_failures() {
        let throttle = Throttle::new(limits());
        let t0 = Instant::now();
        // Two failures for ann, then ann gets it right from the same address.
        for _ in 0..2 {
            throttle.begin_login(t0, ip("10.0.0.1"), "ann").unwrap();
        }
        let attempt = throttle.begin_login(t0, ip("10.0.0.1"), "ann").unwrap();
        throttle.succeeded(attempt);
        // The name is clean again, so it has its full three attempts, but the address still holds
        // the two failures and has three of its five left.
        assert!(throttle.begin_login(t0, ip("10.0.0.1"), "ann").is_ok());
        assert!(throttle.begin_login(t0, ip("10.0.0.1"), "ann").is_ok());
        assert!(throttle.begin_login(t0, ip("10.0.0.1"), "ann").is_ok());
        assert!(throttle.begin_login(t0, ip("10.0.0.1"), "ann").is_err());
        // The address is used up too: 2 + 3 counted.
        assert!(throttle.begin_login(t0, ip("10.0.0.1"), "bob").is_err());
        assert!(throttle.begin_login(t0, ip("10.0.0.2"), "bob").is_ok());
    }

    #[test]
    fn the_try_that_uses_up_a_limit_says_so_once() {
        let throttle = Throttle::new(limits());
        let t0 = Instant::now();
        let exhausted: Vec<bool> = (0..3)
            .map(|_| {
                throttle
                    .begin_login(t0, ip("10.0.0.1"), "ann")
                    .unwrap()
                    .exhausted()
            })
            .collect();
        // The name's limit is 3 and the address's 5: the third try reaches the name's.
        assert_eq!(exhausted, [false, false, true]);
        assert!(throttle.begin_login(t0, ip("10.0.0.1"), "ann").is_err());
        // An address reaches its own limit with the fifth try, whoever sends it.
        let throttle = Throttle::new(limits());
        let at_address: Vec<bool> = (0..5)
            .map(|n| {
                throttle
                    .begin_login(t0, ip("10.0.0.1"), &format!("user{n}"))
                    .unwrap()
                    .exhausted()
            })
            .collect();
        assert_eq!(at_address, [false, false, false, false, true]);
        // Registrations say the same.
        let throttle = Throttle::new(limits());
        assert_eq!(throttle.begin_register(t0, ip("10.0.0.1")), Ok(false));
        assert_eq!(throttle.begin_register(t0, ip("10.0.0.1")), Ok(true));
        assert!(throttle.begin_register(t0, ip("10.0.0.1")).is_err());
    }

    #[test]
    fn what_the_throttle_prints_holds_no_username() {
        let throttle = Throttle::new(limits());
        let attempt = throttle
            .begin_login(Instant::now(), ip("10.0.0.1"), "correcthorse")
            .unwrap();
        let shown = format!("{throttle:?} {throttle:#?} {attempt:?} {attempt:#?}");
        assert!(
            !shown.contains("correcthorse") && !shown.contains("10.0.0.1"),
            "{shown}"
        );
    }

    #[test]
    fn an_abandoned_login_gives_back_both_counts_but_clears_nothing_else() {
        let throttle = Throttle::new(limits());
        let t0 = Instant::now();
        // Two real failures for ann, then a check the database could not do.
        for _ in 0..2 {
            throttle.begin_login(t0, ip("10.0.0.1"), "ann").unwrap();
        }
        for _ in 0..50 {
            let attempt = throttle.begin_login(t0, ip("10.0.0.1"), "ann").unwrap();
            throttle.abandoned(attempt);
        }
        // The two failures still count: one more fits under the name's three, then it is full.
        assert!(throttle.begin_login(t0, ip("10.0.0.1"), "ann").is_ok());
        assert!(throttle.begin_login(t0, ip("10.0.0.1"), "ann").is_err());
    }

    #[test]
    fn registrations_have_a_budget_of_their_own() {
        let throttle = Throttle::new(limits());
        let t0 = Instant::now();
        assert!(throttle.begin_register(t0, ip("10.0.0.1")).is_ok());
        assert!(throttle.begin_register(t0, ip("10.0.0.1")).is_ok());
        let blocked = throttle.begin_register(t0 + secs(5), ip("10.0.0.1"));
        assert_eq!(blocked.unwrap_err().retry_after, secs(55));
        assert!(throttle.begin_register(t0, ip("10.0.0.2")).is_ok());
        // Logins from that address are not affected, and the other way round.
        assert!(throttle.begin_login(t0, ip("10.0.0.1"), "ann").is_ok());
        assert!(
            throttle
                .begin_register(t0 + secs(60), ip("10.0.0.1"))
                .is_ok()
        );
    }

    #[rstest]
    #[case::same_v4("10.0.0.1", "10.0.0.1", true)]
    #[case::other_v4("10.0.0.1", "10.0.0.2", false)]
    #[case::same_v6_64("2001:db8:1:2::1", "2001:db8:1:2:ffff:ffff:ffff:ffff", true)]
    #[case::other_v6_64("2001:db8:1:2::1", "2001:db8:1:3::1", false)]
    #[case::mapped_v4("10.0.0.1", "::ffff:10.0.0.1", true)]
    #[case::mapped_other_v4("10.0.0.1", "::ffff:10.0.0.2", false)]
    #[case::v4_and_v6("10.0.0.1", "2001:db8::1", false)]
    fn addresses_are_grouped_the_way_a_client_controls_them(
        #[case] first: &str,
        #[case] second: &str,
        #[case] same: bool,
    ) {
        assert_eq!(network(ip(first)) == network(ip(second)), same);
        let throttle = Throttle::new(AttemptLimits {
            register_per_ip: 1,
            ..limits()
        });
        let t0 = Instant::now();
        assert!(throttle.begin_register(t0, ip(first)).is_ok());
        assert_eq!(throttle.begin_register(t0, ip(second)).is_err(), same);
    }

    #[test]
    fn an_admin_has_a_budget_of_its_own_that_never_touches_the_registrations_of_an_address() {
        let throttle = Throttle::new(limits());
        let (ann, bob) = (UserId::random(), UserId::random());
        let t0 = Instant::now();
        for n in 0..4 {
            assert!(throttle.begin_admin_create(t0 + secs(n), ann).is_ok());
        }
        let blocked = throttle.begin_admin_create(t0 + secs(10), ann);
        assert_eq!(blocked.unwrap_err().retry_after, secs(50));
        // Another admin, and the registrations of every address, are untouched.
        assert!(throttle.begin_admin_create(t0, bob).is_ok());
        for _ in 0..2 {
            assert!(throttle.begin_register(t0, ip("10.0.0.1")).is_ok());
        }
        assert!(throttle.begin_register(t0, ip("10.0.0.1")).is_err());
        // The window runs from the first try, and a refused try does not push it out.
        assert!(throttle.begin_admin_create(t0 + secs(60), ann).is_ok());
    }

    #[test]
    fn a_blocked_registration_costs_the_admin_budget_nothing() {
        let throttle = Throttle::new(limits());
        let ann = UserId::random();
        let t0 = Instant::now();
        for _ in 0..2 {
            throttle.begin_register(t0, ip("10.0.0.1")).unwrap();
        }
        assert!(throttle.begin_register(t0, ip("10.0.0.1")).is_err());
        for _ in 0..4 {
            assert!(throttle.begin_admin_create(t0, ann).is_ok());
        }
    }

    #[test]
    fn a_limit_of_zero_turns_everyone_away() {
        let throttle = Throttle::new(AttemptLimits {
            login_per_ip: 0,
            register_per_ip: 0,
            ..limits()
        });
        let t0 = Instant::now();
        assert_eq!(
            throttle
                .begin_login(t0, ip("10.0.0.1"), "ann")
                .unwrap_err()
                .retry_after,
            WINDOW
        );
        assert!(throttle.begin_register(t0, ip("10.0.0.1")).is_err());
    }

    #[rstest]
    #[case(Duration::from_millis(1), 1)]
    #[case(Duration::from_millis(999), 1)]
    #[case(Duration::from_secs(1), 1)]
    #[case(Duration::from_millis(1001), 2)]
    #[case(Duration::from_secs(59), 59)]
    #[case(Duration::ZERO, 1)]
    fn retry_after_rounds_up_to_whole_seconds(#[case] wait: Duration, #[case] seconds: u64) {
        assert_eq!(Blocked { retry_after: wait }.seconds(), seconds);
    }

    #[test]
    fn expired_counters_are_swept_and_the_table_stays_bounded() {
        let throttle = Throttle::new(limits());
        let t0 = Instant::now();
        // A flood of distinct usernames from many addresses fills the table with live counters.
        for n in 0..4_000u32 {
            let address = IpAddr::V4(Ipv4Addr::from(0x0a00_0000 + n));
            throttle
                .begin_login(t0, address, &format!("name{n}"))
                .unwrap();
        }
        assert_eq!(throttle.tracked(), 8_000);
        // One window later everything has expired. The table is swept once it has doubled since
        // the last sweep, so a little traffic is enough to clear it.
        for n in 0..150u32 {
            let address = IpAddr::V4(Ipv4Addr::from(0x0b00_0000 + n));
            throttle
                .begin_login(t0 + WINDOW, address, &format!("late{n}"))
                .unwrap();
        }
        assert!(
            throttle.tracked() < 1_000,
            "{} counters left",
            throttle.tracked()
        );
    }

    #[test]
    fn past_the_cap_new_names_go_untracked_but_tracked_ones_keep_their_limit() {
        let throttle = Throttle::new(AttemptLimits {
            login_per_ip: u32::MAX,
            ..limits()
        });
        let t0 = Instant::now();
        let flood = ip("10.0.0.1");
        for n in 0..(u32::try_from(MAX_TRACKED).unwrap() + 100) {
            // Never an error, however many names there are.
            throttle
                .begin_login(t0, flood, &format!("name{n}"))
                .unwrap();
        }
        assert_eq!(throttle.tracked(), MAX_TRACKED);
        // Names already tracked keep their limit.
        for _ in 0..2 {
            throttle.begin_login(t0, flood, "name0").unwrap();
        }
        assert!(throttle.begin_login(t0, flood, "name0").is_err());
    }

    #[test]
    fn a_full_table_lets_a_new_address_through_untracked() {
        let throttle = Throttle::new(limits());
        let t0 = Instant::now();
        for n in 0..MAX_TRACKED {
            let address = IpAddr::V6(Ipv6Addr::from((u128::try_from(n).unwrap() + 1) << 64));
            throttle.begin_register(t0, address).unwrap();
        }
        assert_eq!(throttle.tracked(), MAX_TRACKED);
        // The table is full, so this address goes untracked rather than locking anybody out.
        assert!(throttle.begin_register(t0, ip("10.0.0.1")).is_ok());
        assert_eq!(throttle.tracked(), MAX_TRACKED);
    }

    #[test]
    fn code_misses_block_at_their_count_and_lift_after_their_own_window() {
        let throttle = Throttle::new(limits());
        let t0 = Instant::now();
        let mut attempts = Vec::new();
        for n in 0..4 {
            let attempt = throttle.begin_code(t0 + secs(n), ip("10.0.0.1")).unwrap();
            attempts.push(attempt.exhausted());
        }
        assert_eq!(attempts, [false, false, false, true]);
        // The window began at the first miss and is 20 s long, not the 60 s of the logins.
        let blocked = throttle
            .begin_code(t0 + secs(5), ip("10.0.0.1"))
            .unwrap_err();
        assert_eq!(blocked.retry_after, secs(15));
        let blocked = throttle
            .begin_code(t0 + secs(19), ip("10.0.0.1"))
            .unwrap_err();
        assert_eq!(blocked.retry_after, secs(1));
        // Other addresses are untouched.
        assert!(throttle.begin_code(t0 + secs(5), ip("10.0.0.2")).is_ok());
        assert!(throttle.begin_code(t0 + secs(20), ip("10.0.0.1")).is_ok());
    }

    #[test]
    fn a_code_that_finds_its_game_costs_nothing() {
        let throttle = Throttle::new(limits());
        let t0 = Instant::now();
        for _ in 0..200 {
            let attempt = throttle.begin_code(t0, ip("10.0.0.1")).unwrap();
            throttle.succeeded(attempt);
        }
        assert_eq!(throttle.tracked(), 0);
        // Misses count, hits in between do not undo them.
        for _ in 0..3 {
            throttle.begin_code(t0, ip("10.0.0.1")).unwrap();
        }
        let hit = throttle.begin_code(t0, ip("10.0.0.1")).unwrap();
        throttle.succeeded(hit);
        assert!(throttle.begin_code(t0, ip("10.0.0.1")).is_ok());
        assert!(throttle.begin_code(t0, ip("10.0.0.1")).is_err());
    }

    #[test]
    fn code_misses_logins_and_registrations_do_not_share_a_budget() {
        let throttle = Throttle::new(limits());
        let t0 = Instant::now();
        for _ in 0..4 {
            throttle.begin_code(t0, ip("10.0.0.1")).unwrap();
        }
        assert!(throttle.begin_code(t0, ip("10.0.0.1")).is_err());
        assert!(throttle.begin_login(t0, ip("10.0.0.1"), "ann").is_ok());
        assert!(throttle.begin_register(t0, ip("10.0.0.1")).is_ok());
        for _ in 0..2 {
            throttle
                .begin_register(t0 + secs(30), ip("10.0.0.3"))
                .unwrap();
        }
        assert!(
            throttle
                .begin_register(t0 + secs(30), ip("10.0.0.3"))
                .is_err()
        );
        assert!(throttle.begin_code(t0 + secs(30), ip("10.0.0.3")).is_ok());
    }

    #[rstest]
    #[case::same_v4("10.0.0.1", "10.0.0.1", true)]
    #[case::other_v4("10.0.0.1", "10.0.0.2", false)]
    #[case::same_v6_64("2001:db8:1:2::1", "2001:db8:1:2:ffff:ffff:ffff:ffff", true)]
    #[case::other_v6_64("2001:db8:1:2::1", "2001:db8:1:3::1", false)]
    #[case::mapped_v4("10.0.0.1", "::ffff:10.0.0.1", true)]
    fn code_misses_are_counted_per_network(
        #[case] first: &str,
        #[case] second: &str,
        #[case] same: bool,
    ) {
        let throttle = Throttle::new(AttemptLimits {
            code_misses_per_ip: 1,
            ..limits()
        });
        let t0 = Instant::now();
        assert!(throttle.begin_code(t0, ip(first)).is_ok());
        assert_eq!(throttle.begin_code(t0, ip(second)).is_err(), same);
    }

    #[test]
    fn the_sweep_judges_each_counter_by_its_own_window() {
        let throttle = Throttle::new(limits());
        let t0 = Instant::now();
        // More code counters than the first sweep waits for, plus one login that lasts longer.
        for n in 0..1_100u32 {
            let address = IpAddr::V4(Ipv4Addr::from(0x0a00_0000 + n));
            throttle.begin_code(t0, address).unwrap();
        }
        for _ in 0..3 {
            throttle.begin_login(t0, ip("10.9.9.9"), "ann").unwrap();
        }
        assert_eq!(throttle.tracked(), 1_100 + 2);
        // 30 s on, the code counters (20 s) have expired and the login counters (60 s) have not.
        // The table is swept once it has doubled since the last sweep, which new traffic brings about.
        for n in 0..1_000u32 {
            let address = IpAddr::V4(Ipv4Addr::from(0x0b00_0000 + n));
            throttle.begin_register(t0 + secs(30), address).unwrap();
        }
        assert_eq!(throttle.tracked(), 2 + 1_000);
        assert!(
            throttle
                .begin_login(t0 + secs(30), ip("10.9.9.9"), "ann")
                .is_err(),
            "the username still has its three tries counted"
        );
    }

    #[test]
    fn lobby_requests_block_at_their_count_and_lift_after_their_own_window() {
        let throttle = Throttle::new(limits());
        let t0 = Instant::now();
        for n in 0..6 {
            assert!(throttle.begin_listing(t0 + secs(n), ip("10.0.0.1")).is_ok());
        }
        // The window began at the first request and is 10 s long, not the 20 s of the code misses.
        let blocked = throttle
            .begin_listing(t0 + secs(6), ip("10.0.0.1"))
            .unwrap_err();
        assert_eq!(blocked.retry_after, secs(4));
        // Blocked requests are free and do not push the end of the window out.
        let blocked = throttle
            .begin_listing(t0 + secs(9), ip("10.0.0.1"))
            .unwrap_err();
        assert_eq!(blocked.retry_after, secs(1));
        assert!(throttle.begin_listing(t0 + secs(6), ip("10.0.0.2")).is_ok());
        assert!(
            throttle
                .begin_listing(t0 + secs(10), ip("10.0.0.1"))
                .is_ok()
        );
    }

    #[test]
    fn the_lobby_list_shares_no_budget_with_logins_registrations_or_code_misses() {
        let throttle = Throttle::new(limits());
        let t0 = Instant::now();
        for _ in 0..6 {
            throttle.begin_listing(t0, ip("10.0.0.1")).unwrap();
        }
        assert!(throttle.begin_listing(t0, ip("10.0.0.1")).is_err());
        assert!(throttle.begin_login(t0, ip("10.0.0.1"), "ann").is_ok());
        assert!(throttle.begin_register(t0, ip("10.0.0.1")).is_ok());
        assert!(throttle.begin_code(t0, ip("10.0.0.1")).is_ok());
        // And the other way round: a spent code budget leaves the list alone.
        for _ in 0..4 {
            throttle.begin_code(t0, ip("10.0.0.3")).unwrap();
        }
        assert!(throttle.begin_code(t0, ip("10.0.0.3")).is_err());
        assert!(throttle.begin_listing(t0, ip("10.0.0.3")).is_ok());
    }

    #[test]
    fn precon_requests_block_at_their_count_in_the_list_window_and_share_no_budget() {
        let throttle = Throttle::new(limits());
        let t0 = Instant::now();
        for n in 0..3 {
            assert!(throttle.begin_precons(t0 + secs(n), ip("10.0.0.1")).is_ok());
        }
        let blocked = throttle
            .begin_precons(t0 + secs(3), ip("10.0.0.1"))
            .unwrap_err();
        assert_eq!(blocked.retry_after, secs(7));
        assert!(throttle.begin_precons(t0 + secs(3), ip("10.0.0.2")).is_ok());
        // The lobby list keeps its own budget.
        assert!(throttle.begin_listing(t0, ip("10.0.0.1")).is_ok());
        assert!(
            throttle
                .begin_precons(t0 + secs(10), ip("10.0.0.1"))
                .is_ok()
        );
    }

    #[rstest]
    #[case::same_v4("10.0.0.1", "10.0.0.1", true)]
    #[case::other_v4("10.0.0.1", "10.0.0.2", false)]
    #[case::same_v6_64("2001:db8:1:2::1", "2001:db8:1:2:ffff:ffff:ffff:ffff", true)]
    #[case::other_v6_64("2001:db8:1:2::1", "2001:db8:1:3::1", false)]
    #[case::mapped_v4("10.0.0.1", "::ffff:10.0.0.1", true)]
    fn lobby_requests_are_counted_per_network(
        #[case] first: &str,
        #[case] second: &str,
        #[case] same: bool,
    ) {
        let throttle = Throttle::new(AttemptLimits {
            lobby_requests_per_ip: 1,
            ..limits()
        });
        let t0 = Instant::now();
        assert!(throttle.begin_listing(t0, ip(first)).is_ok());
        assert_eq!(throttle.begin_listing(t0, ip(second)).is_err(), same);
    }

    #[test]
    fn a_lock_poisoned_by_a_panic_still_works() {
        let throttle = std::sync::Arc::new(Throttle::new(limits()));
        let poisoner = std::sync::Arc::clone(&throttle);
        let _ = std::thread::spawn(move || {
            let _held = poisoner.state.lock().unwrap();
            panic!("poison the lock");
        })
        .join();
        assert!(
            throttle
                .begin_register(Instant::now(), ip("10.0.0.1"))
                .is_ok()
        );
    }
}
