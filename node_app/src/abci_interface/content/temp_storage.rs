//! Temp blob reads and writes used by the content connection.

use eld_common::error::EldError;

pub trait ContentTempStorage: Send + Sync {
    fn read_temp_blob(&self, content_id: &str) -> Result<Option<Vec<u8>>, EldError>;

    fn write_temp_blob(
        &self,
        content_id: &str,
        bytes: &[u8],
        expires_at_unix: u64,
    ) -> Result<(), EldError>;
}
