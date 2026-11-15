//! Admin login for the hosted service: only admins upload to S3, list or
//! delete. Passwords are stored as Argon2id hashes (`satchel-files
//! hash-password`), sessions live in memory and travel in an HttpOnly,
//! SameSite=Strict cookie. The local install has no login.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::http::{header, HeaderMap};

pub const COOKIE: &str = "satchel_session";
const SESSION_FOR: Duration = Duration::from_secs(12 * 3600);

/// Password checks allowed at the same time. Each Argon2id run takes about
/// 19 MiB, so this caps memory on a small instance and paces guessing.
pub const MAX_CONCURRENT_CHECKS: usize = 2;
/// Wrong passwords for one user name before each attempt has to wait.
const FREE_FAILURES: u32 = 3;
/// The wait doubles with every further failure, up to this.
const MAX_WAIT: Duration = Duration::from_secs(60);

pub struct Auth {
    /// User name to Argon2 PHC string.
    users: HashMap<String, String>,
    sessions: Mutex<HashMap<String, (String, Instant)>>,
    /// Send the cookie with `Secure`. Off only for plain-HTTP tests.
    secure: bool,
    checks: Arc<tokio::sync::Semaphore>,
    /// Failed attempts per user name: how many, and when the last one was.
    failures: Mutex<HashMap<String, (u32, Instant)>>,
}

impl Auth {
    /// `admins` is `name:phc name:phc`, separated by spaces or new lines.
    /// PHC strings use `$` and `,` inside, but never whitespace.
    pub fn parse(admins: &str, secure: bool) -> anyhow::Result<Self> {
        let mut users = HashMap::new();
        for entry in admins.split_whitespace() {
            let (name, hash) = entry
                .split_once(':')
                .ok_or_else(|| anyhow::anyhow!("admin entries are name:hash"))?;
            PasswordHash::new(hash).map_err(|e| anyhow::anyhow!("bad hash for {name}: {e}"))?;
            users.insert(name.to_owned(), hash.to_owned());
        }
        if users.is_empty() {
            anyhow::bail!("no admins configured");
        }
        Ok(Self {
            users,
            sessions: Mutex::new(HashMap::new()),
            secure,
            checks: Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_CHECKS)),
            failures: Mutex::new(HashMap::new()),
        })
    }

    /// A slot to run one password check, or `None` if the others are busy.
    pub fn check_slot(&self) -> Option<tokio::sync::OwnedSemaphorePermit> {
        self.checks.clone().try_acquire_owned().ok()
    }

    /// How long this user name must wait before the next attempt.
    pub fn retry_after(&self, user: &str) -> Option<Duration> {
        let failures = self.failures.lock().expect("lock");
        let (count, last) = failures.get(user)?;
        let wait = wait_after(*count);
        let since = last.elapsed();
        (since < wait).then(|| wait - since)
    }

    /// Records the result of a check: success clears the user's failures.
    pub fn record(&self, user: &str, ok: bool) {
        let mut failures = self.failures.lock().expect("lock");
        if ok {
            failures.remove(user);
            return;
        }
        // Forget old failures so the map can't grow without bound.
        failures.retain(|_, (_, at)| at.elapsed() < Duration::from_secs(3600));
        let entry = failures
            .entry(user.to_owned())
            .or_insert((0, Instant::now()));
        entry.0 += 1;
        entry.1 = Instant::now();
    }

    /// Checks a password. Every attempt runs Argon2, so a wrong user name
    /// takes as long as a wrong password.
    pub fn verify(&self, user: &str, password: &str) -> bool {
        const DUMMY: &str =
            "$argon2id$v=19$m=19456,t=2,p=1$c2F0Y2hlbHNhbHQ$0Kq3R0x2a9tDJcJYdS1oDQ0w3bW1oR0d8Yx5dFq2m4A";
        let stored = self.users.get(user).map(String::as_str).unwrap_or(DUMMY);
        let ok = PasswordHash::new(stored)
            .map(|h| {
                Argon2::default()
                    .verify_password(password.as_bytes(), &h)
                    .is_ok()
            })
            .unwrap_or(false);
        ok && self.users.contains_key(user)
    }

    /// Starts a session and returns the `Set-Cookie` value.
    pub fn start(&self, user: &str) -> String {
        let mut raw = [0u8; 32];
        getrandom::fill(&mut raw).expect("the OS has randomness");
        let token = hex::encode(raw);
        let mut sessions = self.sessions.lock().expect("lock");
        sessions.retain(|_, (_, at)| at.elapsed() < SESSION_FOR);
        sessions.insert(token.clone(), (user.to_owned(), Instant::now()));
        format!(
            "{COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}{}",
            SESSION_FOR.as_secs(),
            if self.secure { "; Secure" } else { "" }
        )
    }

    /// The signed-in user, if the request carries a live session.
    pub fn user(&self, headers: &HeaderMap) -> Option<String> {
        let token = session_token(headers)?;
        let sessions = self.sessions.lock().expect("lock");
        let (user, at) = sessions.get(&token)?;
        (at.elapsed() < SESSION_FOR).then(|| user.clone())
    }

    /// Ends the request's session and returns a cookie that clears it.
    pub fn end(&self, headers: &HeaderMap) -> String {
        if let Some(token) = session_token(headers) {
            self.sessions.lock().expect("lock").remove(&token);
        }
        format!(
            "{COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0{}",
            if self.secure { "; Secure" } else { "" }
        )
    }
}

/// No wait for the first few failures, then 1 s, 2 s, 4 s... up to a minute.
fn wait_after(failures: u32) -> Duration {
    if failures < FREE_FAILURES {
        return Duration::ZERO;
    }
    let exp = (failures - FREE_FAILURES).min(16);
    Duration::from_secs(1u64 << exp).min(MAX_WAIT)
}

fn session_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == COOKIE)
        .map(|(_, v)| v.to_owned())
}

/// An Argon2id hash of `password`, for `SATCHEL_ADMINS`.
pub fn hash_password(password: &str) -> String {
    let mut salt = [0u8; 16];
    getrandom::fill(&mut salt).expect("the OS has randomness");
    let salt = SaltString::encode_b64(&salt).expect("16 bytes is a valid salt");
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("hashing a password works")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn auth() -> Auth {
        Auth::parse(&format!("admin:{}", hash_password("correct horse")), true).unwrap()
    }

    fn with_cookie(set_cookie: &str) -> HeaderMap {
        let pair = set_cookie.split(';').next().unwrap();
        let mut h = HeaderMap::new();
        h.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("theme=dark; {pair}")).unwrap(),
        );
        h
    }

    #[test]
    fn the_right_password_signs_in() {
        let a = auth();
        assert!(a.verify("admin", "correct horse"));
        assert!(!a.verify("admin", "wrong"));
        assert!(!a.verify("nobody", "correct horse"));
    }

    #[test]
    fn sessions_last_until_signed_out() {
        let a = auth();
        let cookie = a.start("admin");
        assert!(
            cookie.contains("HttpOnly")
                && cookie.contains("SameSite=Strict")
                && cookie.contains("Secure")
        );
        let headers = with_cookie(&cookie);
        assert_eq!(a.user(&headers).as_deref(), Some("admin"));
        a.end(&headers);
        assert_eq!(a.user(&headers), None);
    }

    #[test]
    fn made_up_sessions_are_refused() {
        let a = auth();
        let headers = with_cookie(&format!("{COOKIE}=deadbeef"));
        assert_eq!(a.user(&headers), None);
        assert_eq!(a.user(&HeaderMap::new()), None);
    }

    #[test]
    fn repeated_failures_make_the_next_attempt_wait() {
        let a = auth();
        for _ in 0..FREE_FAILURES {
            assert_eq!(a.retry_after("admin"), None);
            a.record("admin", false);
        }
        // The third failure already makes the next attempt wait a second.
        let wait = a.retry_after("admin").expect("a wait after 3 failures");
        assert!(wait <= Duration::from_secs(1));
        assert_eq!(a.retry_after("someone-else"), None);
        a.record("admin", true);
        assert_eq!(a.retry_after("admin"), None);
    }

    #[test]
    fn waits_double_up_to_a_minute() {
        assert_eq!(wait_after(2), Duration::ZERO);
        assert_eq!(wait_after(3), Duration::from_secs(1));
        assert_eq!(wait_after(4), Duration::from_secs(2));
        assert_eq!(wait_after(8), Duration::from_secs(32));
        assert_eq!(wait_after(9), Duration::from_secs(60));
        assert_eq!(wait_after(500), Duration::from_secs(60));
    }

    #[test]
    fn only_a_few_checks_run_at_once() {
        let a = auth();
        let held: Vec<_> = (0..MAX_CONCURRENT_CHECKS)
            .map(|_| a.check_slot().unwrap())
            .collect();
        assert!(a.check_slot().is_none());
        drop(held);
        assert!(a.check_slot().is_some());
    }

    #[test]
    fn bad_config_is_rejected() {
        assert!(Auth::parse("", true).is_err());
        assert!(Auth::parse("admin:not-a-hash", true).is_err());
        assert!(Auth::parse("nocolon", true).is_err());
    }
}
