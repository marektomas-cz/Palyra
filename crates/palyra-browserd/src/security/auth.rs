use crate::*;

impl BrowserRuntimeState {
    pub(crate) async fn authorize(
        &self,
        metadata: &tonic::metadata::MetadataMap,
    ) -> Result<(), Status> {
        let Some(expected_token) = self.auth_token.as_ref() else {
            return Ok(());
        };
        let supplied = metadata
            .get(AUTHORIZATION_HEADER)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        let expected = format!("Bearer {expected_token}");
        if !constant_time_eq_bytes(supplied.trim().as_bytes(), expected.as_bytes()) {
            return Err(Status::unauthenticated("missing or invalid browser service token"));
        }
        Ok(())
    }
}

pub(crate) fn constant_time_eq_bytes(left: &[u8], right: &[u8]) -> bool {
    let max_len = left.len().max(right.len());
    let mut difference = left.len() ^ right.len();
    for index in 0..max_len {
        let left_byte = left.get(index).copied().unwrap_or(0);
        let right_byte = right.get(index).copied().unwrap_or(0);
        difference |= usize::from(left_byte ^ right_byte);
    }
    difference == 0
}
