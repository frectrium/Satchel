//! Admin login for the hosted service: only admins upload to S3, list or
//! delete. Passwords are stored as Argon2id hashes (`satchel-files
//! hash-password`), sessions live in memory and travel in an HttpOnly,
//! SameSite=Strict cookie. The local install has no login.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::http::{header, HeaderMap};

pub const COOKIE: &str = "satchel_session";
const SESSION_FOR: Duration = Duration::from_secs(12 * 3600);

pub struct Auth {
    /// User name to Argon2 PHC string.
    users: HashMap<String, String>,
    sessions: Mutex<HashMap<String, (String, Instant)>>,
    /// Send the cookie with `Secure`. Off only for plain-HTTP tests.
    secure: bool,
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
        })
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
    fn bad_config_is_rejected() {
        assert!(Auth::parse("", true).is_err());
        assert!(Auth::parse("admin:not-a-hash", true).is_err());
        assert!(Auth::parse("nocolon", true).is_err());
    }
}
