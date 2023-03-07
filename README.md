# serde_canonical_json

This crate provides a [Canonical JSON](https://wiki.laptop.org/go/Canonical_JSON) formatter for serde_json.

## Usage

```rust
use serde::Serialize;
use serde_json::Serializer;
use serde_canonical_json::CanonicalFormatter;


#[derive(Serialize)]
struct Data
{
    c: isize,
    b: bool,
    a: String,
}


fn main()
{
    let data = Data { c: 120, b: false, a: "Hello!".to_owned() };

    let mut ser = Serializer::with_formatter(Vec::new(), CanonicalFormatter::new());

    data.serialize(&mut ser).expect("Failed to serialize");
    
    let json = String::from_utf8(ser.into_inner()).expect("Failed to convert buffer to string");

    assert_eq!(json, r#"{"a":"Hello!","b":false,"c":120}"#);
}