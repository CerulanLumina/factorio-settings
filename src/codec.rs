use crate::types::FactorioVersion;
use anyhow::anyhow;
use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use indexmap::IndexMap;
use std::io::{Read, Write};

impl Codec for FactorioVersion {
    fn decode(input: &mut impl Read) -> anyhow::Result<FactorioVersion> {
        let [major, minor, patch, build] = {
            let mut vers = [0; 4];
            input.read_u16_into::<LE>(&mut vers)?;
            vers
        };
        Ok(FactorioVersion {
            major,
            minor,
            patch,
            build,
        })
    }

    fn encode(&self, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u16::<LE>(self.major)?;
        writer.write_u16::<LE>(self.minor)?;
        writer.write_u16::<LE>(self.patch)?;
        writer.write_u16::<LE>(self.build)?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Property {
    pub any_flag: bool,
    pub value: PropertyValue,
}

#[derive(Clone, Debug)]
pub enum PropertyValue {
    None,
    Bool(bool),
    Number(f64),
    String(String),
    List(Vec<Property>),
    Dictionary(IndexMap<String, Property>),
}

impl PropertyValue {

    pub fn as_bool(&self) -> Option<&bool> {
        match self {
            Self::Bool(b) => Some(b),
            _ => None,
        }
    }

    pub fn as_number(&self) -> Option<&f64> {
        match self {
            Self::Number(f) => Some(f),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&String> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&Vec<Property>> {
        match self {
            Self::List(l) => Some(l),
            _ => None,
        }
    }

    pub fn as_dictionary(&self) -> Option<&IndexMap<String, Property>> {
        match self {
            Self::Dictionary(map) => Some(map),
            _ => None,
        }
    }
}

impl Codec for Property {
    fn decode(input: &mut impl Read) -> anyhow::Result<Property> {
        let [vtype, any_flag] = {
            let mut tree_header = [0; 2];
            input.read_exact(&mut tree_header)?;
            tree_header
        };
        let value = match vtype {
            0 => PropertyValue::None,
            1 => PropertyValue::Bool(Codec::decode(input)?),
            2 => PropertyValue::Number(Codec::decode(input)?),
            3 => PropertyValue::String(Codec::decode(input)?),
            4 => PropertyValue::List(Codec::decode(input)?),
            5 => PropertyValue::Dictionary(Codec::decode(input)?),
            _ => return Err(anyhow!("Unknown type")),
        };
        Ok(Property {
            any_flag: loose_bool(any_flag),
            value,
        })
    }

    fn encode(&self, _writer: &mut impl Write) -> anyhow::Result<()> {
        todo!()
    }
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub version: FactorioVersion,
    pub properties: Property,
}

impl Settings {
    pub fn from_reader(reader: &mut impl Read) -> anyhow::Result<Settings> {
        Self::decode(reader)
    }
}

impl Codec for Settings {
    fn decode(input: &mut impl Read) -> anyhow::Result<Settings> {
        let version = FactorioVersion::decode(input)?;
        if input.read_u8()? != 0 {
            return Err(anyhow!("Byte at 0x8 should be false"));
        }
        let settings = Property::decode(input)?;
        Ok(Self {
            version,
            properties: settings,
        })
    }

    fn encode(&self, _writer: &mut impl Write) -> anyhow::Result<()> {
        todo!()
    }
}

trait Codec: Sized {
    fn decode(reader: &mut impl Read) -> anyhow::Result<Self>;
    fn encode(&self, writer: &mut impl Write) -> anyhow::Result<()>;
}

impl Codec for bool {
    fn decode(reader: &mut impl Read) -> anyhow::Result<Self> {
        reader
            .read_u8()
            .map(loose_bool)
            .map_err(anyhow::Error::from)
    }

    fn encode(&self, _writer: &mut impl Write) -> anyhow::Result<()> {
        todo!()
    }
}

impl Codec for f64 {
    fn decode(reader: &mut impl Read) -> anyhow::Result<Self> {
        Ok(reader.read_f64::<LE>()?)
    }

    fn encode(&self, _writer: &mut impl Write) -> anyhow::Result<()> {
        todo!()
    }
}

impl Codec for String {
    fn decode(reader: &mut impl Read) -> anyhow::Result<Self> {
        let empty_byte = reader.read_u8()?;
        if !loose_bool(empty_byte) {
            // if not empty
            let length = read_optimized_u32(reader)?;
            let mut vec = vec![0; length as usize];
            reader.read_exact(&mut vec[..])?;
            Ok(String::from_utf8(vec)?)
        } else {
            Ok(String::new())
        }
    }

    fn encode(&self, _writer: &mut impl Write) -> anyhow::Result<()> {
        todo!()
    }
}

impl Codec for Vec<Property> {
    fn decode(_reader: &mut impl Read) -> anyhow::Result<Self> {
        todo!()
    }

    fn encode(&self, _writer: &mut impl Write) -> anyhow::Result<()> {
        todo!()
    }
}

impl Codec for IndexMap<String, Property> {
    fn decode(reader: &mut impl Read) -> anyhow::Result<Self> {
        let count = reader.read_u32::<LE>()?;
        let mut map = IndexMap::with_capacity(count as usize);
        for _ in 0..count {
            let name = String::decode(reader)?;
            let value = Property::decode(reader)?;
            map.insert(name, value);
        }
        Ok(map)
    }

    fn encode(&self, _writer: &mut impl Write) -> anyhow::Result<()> {
        todo!()
    }
}

#[inline]
const fn loose_bool(input: u8) -> bool {
    matches!(input, 1)
}

#[inline]
fn read_optimized_u32(reader: &mut impl Read) -> anyhow::Result<u32> {
    Ok(match reader.read_u8()? {
        0xff => reader.read_u32::<LE>()?,
        byte => byte as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::{Codec, Property, PropertyValue, Settings};
    use crate::types::FactorioVersion;
    use hex_literal::hex;
    use std::fs::File;
    use std::io::{BufReader, Cursor, Write};

    use indexmap::IndexMap;

    #[test]
    fn simple_encoded() {
        let data = hex!("01 00 01 00 52 00 04 00 00 05 00 03 00 00 00 00 07 73 74 61 72 74 75 70 05 00 01 00 00 00 00 11 6D 79 2D 73 74 72 69 6E 67 2D 73 65 74 74 69 6E 67 05 00 01 00 00 00 00 05 76 61 6C 75 65 03 00 00 08 64 65 61 64 62 65 65 66 00 0E 72 75 6E 74 69 6D 65 2D 67 6C 6F 62 61 6C 05 00 00 00 00 00 00 10 72 75 6E 74 69 6D 65 2D 70 65 72 2D 75 73 65 72 05 00 00 00 00 00");
        let mut cursor = Cursor::new(data);
        let settings = Settings::decode(&mut cursor).expect("decoding settings");
        assert_eq!(
            settings.version,
            FactorioVersion {
                major: 1,
                minor: 1,
                patch: 82,
                build: 4
            },
            "version"
        );
        assert!(!settings.properties.any_flag, "should be false");
        println!("{:?}", &settings.properties);
        let root = get_map(&settings.properties);
        let startup_dict = get_map(root.get("startup").expect("missing startup"));
        let my_setting = get_map(
            startup_dict
                .get("my-string-setting")
                .expect("missing my-string-setting"),
        );
        let value = my_setting.get("value").expect("missing value");
        match &value.value {
            PropertyValue::String(s) => assert_eq!(s, "deadbeef", "incorrect value"),
            _ => panic!("Incorrect type"),
        }
    }

    #[test]
    fn complex() {
        let mut reader = BufReader::new(File::open("../test_data/complex-settings.dat").expect("opening file"));
        let settings = Settings::decode(&mut reader).expect("decoding settings");
    }

    fn get_map(prop: &Property) -> &IndexMap<String, Property> {
        match &prop.value {
            PropertyValue::Dictionary(map) => map,
            _ => panic!("expected dictionary"),
        }
    }
}