use crate::error::EldError;
use serde::Serialize;
use serde_json::Value;

pub fn to_json_string<T: Serialize>(value: &T) -> Result<String, EldError> {
    serde_json::to_string(value).map_err(|e| EldError::BasicValidationError {
        details: e.to_string(),
    })
}

pub fn to_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, EldError> {
    serde_json::to_vec(value).map_err(|e| EldError::BasicValidationError {
        details: e.to_string(),
    })
}

/// Decode a JSON array of integers in `0..=255`.
///
/// CADO `hash`, `latest_hash`, and `data` fields are number arrays on the wire.
/// Values outside that range, and non-integers, are errors.
pub fn json_number_array_as_bytes(values: &[Value], field: &str) -> Result<Vec<u8>, EldError> {
    values
        .iter()
        .map(|v| {
            v.as_u64()
                .and_then(|n| u8::try_from(n).ok())
                .ok_or_else(|| EldError::ValidationError {
                    field: field.to_string(),
                    value: v.to_string(),
                    details: "CADO JSON array element is not a byte (0-255)".to_string(),
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct Sample {
        name: String,
        count: u32,
    }

    #[test]
    fn to_json_string_serializes_struct() {
        let sample = Sample {
            name: "eld".to_string(),
            count: 2,
        };
        let json = to_json_string(&sample).expect("serialize to string");
        assert_eq!(json, r#"{"name":"eld","count":2}"#);
    }

    #[test]
    fn to_json_bytes_matches_string_utf8() {
        let sample = Sample {
            name: "eld".to_string(),
            count: 2,
        };
        let bytes = to_json_bytes(&sample).expect("serialize to bytes");
        let json = to_json_string(&sample).expect("serialize to string");
        assert_eq!(bytes, json.as_bytes());
    }

    #[test]
    fn json_number_array_keeps_every_byte() {
        let values = vec![
            serde_json::json!(0),
            serde_json::json!(255),
            serde_json::json!(7),
        ];
        assert_eq!(
            json_number_array_as_bytes(&values, "cado.data").unwrap(),
            vec![0, 255, 7]
        );
    }

    #[test]
    fn json_number_array_rejects_out_of_range() {
        for value in [
            serde_json::json!(256),
            serde_json::json!(-1),
            serde_json::json!(1.5),
        ] {
            let err =
                json_number_array_as_bytes(std::slice::from_ref(&value), "cado.hash").unwrap_err();
            let message = err.to_string();
            assert!(message.contains("not a byte"), "{message}");
            assert!(message.contains("cado.hash"), "{message}");
        }
    }
}
