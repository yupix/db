use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

use crate::error::AppError;

pub fn hash(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AppError::Internal(format!("Password hash error: {}", e)))?
        .to_string();

    Ok(hash)
}

pub fn verify(password: &str, hash: &str) -> Result<bool, AppError> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| AppError::Internal(format!("Password hash parse error: {}", e)))?;

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify() {
        let password = "my-secret-password";
        let hashed = hash(password).unwrap();
        assert!(verify(password, &hashed).unwrap());
    }

    #[test]
    fn test_verify_wrong_password() {
        let hashed = hash("correct-password").unwrap();
        assert!(!verify("wrong-password", &hashed).unwrap());
    }

    #[test]
    fn test_verify_invalid_hash() {
        assert!(verify("password", "not-a-valid-hash").is_err());
    }
}
