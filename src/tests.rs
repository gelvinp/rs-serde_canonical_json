use serde::{Serialize, Deserialize};
use serde_json::{self, Serializer};
use std::collections::HashMap;
use crate::CanonicalFormatter;


#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct TestStruct1
{
    a: bool,
    b: bool,
    c: String,
    d: TestStruct2,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct TestStruct2
{
    h: HashMap<String, bool>,
    g: Option<isize>,
    f: String,
    e: Vec<isize>,
}


#[test]
fn canonical()
{
    let mut hash_map = HashMap::with_capacity(3);
    hash_map.insert("i".to_owned(), true);
    hash_map.insert("k".to_owned(), false);
    hash_map.insert("j".to_owned(), true);

    let dut = TestStruct1
    {
        a: true,
        b: false,
        c: "Hello, \"Canonical\"".to_string(),
        d: TestStruct2
        {
            h: hash_map,
            g: None,
            f: "Here is another".to_owned(),
            e: vec![2, 4, 19, -128],
        }
    };

    const EXPECTED: &str = r#"{"a":true,"b":false,"c":"Hello, \"Canonical\"","d":{"e":[2,4,19,-128],"f":"Here is another","g":null,"h":{"i":true,"j":true,"k":false}}}"#;

    let mut ser = Serializer::with_formatter(Vec::new(), CanonicalFormatter::new());
    dut.serialize(&mut ser).unwrap();
    let string = String::from_utf8(ser.into_inner()).unwrap();

    assert_eq!(string, EXPECTED);

    let deserialized: TestStruct1 = serde_json::from_str(&string).unwrap();
    
    assert_eq!(dut, deserialized);
}