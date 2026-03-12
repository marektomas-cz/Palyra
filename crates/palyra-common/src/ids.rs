use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CanonicalIdError {
    #[error("canonical ID must be exactly 26 characters")]
    InvalidLength,
    #[error("canonical ID contains invalid character '{0}'")]
    InvalidCharacter(char),
}

pub fn validate_canonical_id(input: &str) -> Result<(), CanonicalIdError> {
    if input.len() != 26 {
        return Err(CanonicalIdError::InvalidLength);
    }
    for ch in input.chars() {
        if !is_valid_crockford_char(ch) {
            return Err(CanonicalIdError::InvalidCharacter(ch));
        }
    }
    Ok(())
}

fn is_valid_crockford_char(ch: char) -> bool {
    ch.is_ascii_digit() || matches!(ch, 'A'..='H' | 'J'..='K' | 'M'..='N' | 'P'..='T' | 'V'..='Z')
}
