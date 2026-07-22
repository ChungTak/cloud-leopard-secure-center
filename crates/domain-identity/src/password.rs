//! Password hashing and verification using Argon2id.

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use foundation::PlatformError;
use rand_core::OsRng;

/// Argon2id hasher with configurable parameters.
#[derive(Debug, Clone)]
pub struct Argon2idPasswordHasher {
    argon2: Argon2<'static>,
}

impl Argon2idPasswordHasher {
    /// Create a hasher using the provided Argon2 parameters.
    pub fn new(params: argon2::Params) -> Self {
        let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
        Self { argon2 }
    }

    /// Hash a plaintext password into a PHC string.
    pub fn hash(&self, password: &str) -> Result<String, PlatformError> {
        let salt = SaltString::generate(&mut OsRng);
        self.argon2
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|e| PlatformError::invalid("password", e.to_string()))
    }

    /// Verify a plaintext password against a PHC hash string.
    pub fn verify(&self, password: &str, hash: &str) -> Result<bool, PlatformError> {
        let parsed = PasswordHash::new(hash)
            .map_err(|e| PlatformError::invalid("password_hash", e.to_string()))?;
        match self.argon2.verify_password(password.as_bytes(), &parsed) {
            Ok(()) => Ok(true),
            Err(argon2::password_hash::Error::Password) => Ok(false),
            Err(e) => Err(PlatformError::invalid("password_hash", e.to_string())),
        }
    }

    /// Return true if the existing hash was produced with different parameters.
    pub fn needs_rehash(&self, hash: &str) -> Result<bool, PlatformError> {
        let parsed = PasswordHash::new(hash)
            .map_err(|e| PlatformError::invalid("password_hash", e.to_string()))?;
        if parsed.algorithm.to_string() != argon2::Algorithm::Argon2id.to_string() {
            return Ok(true);
        }
        let params = self.argon2.params();
        let m_cost: Option<u32> = parsed.params.get_decimal("m");
        let t_cost: Option<u32> = parsed.params.get_decimal("t");
        let p_cost: Option<u32> = parsed.params.get_decimal("p");

        Ok(m_cost != Some(params.m_cost())
            || t_cost != Some(params.t_cost())
            || p_cost != Some(params.p_cost()))
    }
}

impl Default for Argon2idPasswordHasher {
    fn default() -> Self {
        Self::new(argon2::Params::DEFAULT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_or_panic<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
        match result {
            Ok(v) => v,
            Err(e) => panic!("{e}"),
        }
    }

    #[test]
    fn verify_correct_password_succeeds() {
        let hasher = Argon2idPasswordHasher::default();
        let hash = ok_or_panic(hasher.hash("hunter2"));
        assert!(ok_or_panic(hasher.verify("hunter2", &hash)));
    }

    #[test]
    fn verify_wrong_password_fails() {
        let hasher = Argon2idPasswordHasher::default();
        let hash = ok_or_panic(hasher.hash("hunter2"));
        assert!(!ok_or_panic(hasher.verify("wrong", &hash)));
    }

    #[test]
    fn different_passwords_produce_different_hashes() {
        let hasher = Argon2idPasswordHasher::default();
        let a = ok_or_panic(hasher.hash("hunter2"));
        let b = ok_or_panic(hasher.hash("hunter2"));
        assert_ne!(a, b);
    }

    #[test]
    fn needs_rehash_when_params_differ() {
        let weak_params = match argon2::Params::new(4096, 2, 1, None) {
            Ok(p) => p,
            Err(e) => panic!("{e}"),
        };
        let strong_params = match argon2::Params::new(65536, 3, 1, None) {
            Ok(p) => p,
            Err(e) => panic!("{e}"),
        };
        let weak = Argon2idPasswordHasher::new(weak_params);
        let strong = Argon2idPasswordHasher::new(strong_params);
        let hash = ok_or_panic(weak.hash("hunter2"));
        assert!(ok_or_panic(strong.needs_rehash(&hash)));
    }
}
