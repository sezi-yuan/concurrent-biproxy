mod de;
mod ser;
mod error;

pub use de::{from_bytes, from_bytes_le, Deserializer};
pub use error::{Error, Result};
pub use ser::{to_bytes, to_bytes_le, Serializer};

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use serde::{Serialize, Deserialize};

    use crate::{ser, de};

    #[derive(Serialize, Deserialize, Debug)]
    struct Inner {
        code: u32,
        message: String
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(tag = "tag", content = "tag_content")]
    enum Tag {
        A,
        B(u32),
        C(String)
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct Data<T> {
        signed: i16,
        unsigned: u16,
        message: String,
        list: Vec<String>,
        ports: Vec<u16>,
        map: HashMap<String, String>,
        inner: T,
        tag: Tag

    }


    #[test]
    pub fn test_serde() {
        let mut map = HashMap::new();
        map.insert("helo".to_string(), "world".to_string());
        map.insert("test".to_string(), "32".to_string());
        let data = Data {
            signed: -78,
            unsigned: 50000,
            message: "hello".into(),
            list: vec!["this".into(), "is".into(), "a".into()],
            ports: vec![8848, 8849],
            map,
            inner: Inner {
                code: 32,
                message: "ok".into()
            },
            tag: Tag::C("heihei".into())
        };

        let bytes = ser::to_bytes(&data).unwrap();
        println!("bytes => {:?}", bytes);
        let x = de::from_bytes::<Data<Inner>>(bytes.as_slice()).unwrap();
        println!("dat=> {:?}", x);

    }
}
