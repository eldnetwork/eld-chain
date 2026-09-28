use abci::types::EventAttribute;

/// Builds an indexed ABCI event attribute from UTF-8 key and value strings.
pub fn create_event_attribute(key: String, value: String) -> EventAttribute {
    let e = EventAttribute {
        key: key.as_bytes().to_vec(),
        value: value.as_bytes().to_vec(),
        index: true,
    };
    e
}
