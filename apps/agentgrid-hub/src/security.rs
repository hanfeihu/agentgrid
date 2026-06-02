use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::http::{header, HeaderMap};
use rand_core::OsRng;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub(crate) fn agent_token_hash(agent_id: &str, token: &str) -> String {
    sha256_hex(format!("agentgrid-agent-token-v1:{agent_id}:{token}").as_bytes())
}

pub(crate) fn legacy_user_password_hash(email: &str, password: &str) -> String {
    sha256_hex(format!("agentgrid-user-password-v1:{email}:{password}").as_bytes())
}

pub(crate) fn hash_user_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| anyhow::anyhow!("password hashing failed: {error}"))
}

pub(crate) fn verify_user_password(
    email: &str,
    password: &str,
    stored_hash: &str,
) -> anyhow::Result<bool> {
    if stored_hash.starts_with("$argon2") {
        let parsed = PasswordHash::new(stored_hash)
            .map_err(|error| anyhow::anyhow!("invalid password hash: {error}"))?;
        return Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok());
    }
    Ok(stored_hash == legacy_user_password_hash(email, password))
}

pub(crate) fn password_hash_needs_upgrade(stored_hash: &str) -> bool {
    !stored_hash.starts_with("$argon2")
}

pub(crate) fn session_token_hash(token: &str) -> String {
    sha256_hex(format!("agentgrid-session-v1:{token}").as_bytes())
}

pub(crate) fn email_code_hash(email: &str, code: &str) -> String {
    sha256_hex(format!("agentgrid-email-code-v1:{email}:{code}").as_bytes())
}

pub(crate) fn node_join_token_hash(node_id: &str, token: &str) -> String {
    sha256_hex(format!("agentgrid-node-join-token-v1:{node_id}:{token}").as_bytes())
}

pub(crate) fn bearer_token_from_headers(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    value
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToString::to_string)
}

pub(crate) fn generate_session_token() -> String {
    format!("ags_{}", Uuid::new_v4().simple())
}

pub(crate) fn generate_node_join_token() -> String {
    format!("agj_{}", Uuid::new_v4().simple())
}

pub(crate) fn generate_email_code() -> String {
    let value = Uuid::new_v4().as_u128() % 1_000_000;
    format!("{value:06}")
}

pub(crate) fn token_hint(token: &str) -> String {
    let chars = token.chars().collect::<Vec<_>>();
    if chars.len() <= 6 {
        return "******".to_string();
    }
    let tail = chars[chars.len().saturating_sub(4)..]
        .iter()
        .collect::<String>();
    format!("****{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argon2_password_hash_verifies_and_rejects_wrong_password() {
        let hash = hash_user_password("correct horse battery staple").unwrap();

        assert!(hash.starts_with("$argon2"));
        assert!(
            verify_user_password("user@example.com", "correct horse battery staple", &hash)
                .unwrap()
        );
        assert!(!verify_user_password("user@example.com", "wrong password", &hash).unwrap());
        assert!(!password_hash_needs_upgrade(&hash));
    }

    #[test]
    fn legacy_password_hash_still_verifies_and_requires_upgrade() {
        let legacy = legacy_user_password_hash("user@example.com", "old-password");

        assert!(verify_user_password("user@example.com", "old-password", &legacy).unwrap());
        assert!(!verify_user_password("user@example.com", "wrong", &legacy).unwrap());
        assert!(password_hash_needs_upgrade(&legacy));
    }

    #[test]
    fn generated_tokens_use_expected_prefixes() {
        assert!(generate_session_token().starts_with("ags_"));
        assert!(generate_node_join_token().starts_with("agj_"));
        assert_eq!(generate_email_code().len(), 6);
    }
}
