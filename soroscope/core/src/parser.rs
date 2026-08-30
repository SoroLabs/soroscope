use serde_json::Value;
use soroban_sdk::xdr::{
    Hash, Limits, ScAddress, ScMap, ScMapEntry, ScString, ScSymbol, ScVal, ScVec, StringM, Uint256,
    VecM, WriteXdr,
};
use stellar_strkey::Strkey;
use thiserror::Error;

#[derive(Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum ParserError {
    #[error("Invalid JSON type at {location}: expected {expected}, found {found}")]
    InvalidType {
        location: String,
        expected: String,
        found: String,
    },

    #[error("Invalid symbol at {location}: {details}")]
    InvalidSymbol { location: String, details: String },

    #[error("Invalid hex bytes at {location}: {details}")]
    InvalidHex { location: String, details: String },
}

pub struct ArgParser;

impl ArgParser {
    /// Parse a JSON string into an ScVal
    pub fn parse(json: &str) -> Result<ScVal, ParserError> {
        let value: Value = serde_json::from_str(json).map_err(|e| ParserError::InvalidType {
            location: "$".to_string(),
            expected: "valid JSON".to_string(),
            found: e.to_string(),
        })?;
        Self::parse_value(&value, "$")
    }

    /// Parse a serde_json::Value into an ScVal recursively
    pub fn parse_value(value: &Value, path: &str) -> Result<ScVal, ParserError> {
        match value {
            Value::Null => Ok(ScVal::Void),
            Value::Bool(b) => Ok(ScVal::Bool(*b)),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(ScVal::I64(i))
                } else if let Some(u) = n.as_u64() {
                    Ok(ScVal::U64(u))
                } else {
                    Err(ParserError::InvalidType {
                        location: path.to_string(),
                        expected: "integer".to_string(),
                        found: format!("number {}", n),
                    })
                }
            }
            Value::String(s) => {
                // Address detection
                if (s.starts_with('G') || s.starts_with('C')) && s.len() == 56 {
                    if let Ok(addr) = Self::parse_address(s) {
                        return Ok(ScVal::Address(addr));
                    }
                }

                // Symbol detection (prefixed with :)
                if let Some(sym_str) = s.strip_prefix(':') {
                    let sym: ScSymbol =
                        sym_str.try_into().map_err(|_| ParserError::InvalidSymbol {
                            location: path.to_string(),
                            details: "Symbol must be 1-32 characters".to_string(),
                        })?;
                    return Ok(ScVal::Symbol(sym));
                }

                // Hex bytes detection (prefixed with 0x)
                if let Some(hex_str) = s.strip_prefix("0x") {
                    let bytes = hex::decode(hex_str).map_err(|e| ParserError::InvalidHex {
                        location: path.to_string(),
                        details: e.to_string(),
                    })?;
                    return Ok(ScVal::Bytes(bytes.try_into().map_err(|_| {
                        ParserError::InvalidHex {
                            location: path.to_string(),
                            details: "Bytes exceed maximum allowed size".to_string(),
                        }
                    })?));
                }

                // Default: Treat as String
                let string_m: StringM =
                    s.as_bytes()
                        .to_vec()
                        .try_into()
                        .map_err(|_| ParserError::InvalidType {
                            location: path.to_string(),
                            expected: "shorter string".to_string(),
                            found: "string length exceeds limit".to_string(),
                        })?;
                Ok(ScVal::String(ScString(string_m)))
            }
            Value::Array(arr) => {
                let mut vec = Vec::new();
                for (i, v) in arr.iter().enumerate() {
                    vec.push(Self::parse_value(v, &format!("{}[{}]", path, i))?);
                }
                let vec_m: VecM<ScVal> = vec.try_into().map_err(|_| ParserError::InvalidType {
                    location: path.to_string(),
                    expected: "shorter vector".to_string(),
                    found: "vector size exceeds limit".to_string(),
                })?;
                Ok(ScVal::Vec(Some(ScVec(vec_m))))
            }
            Value::Object(obj) => {
                let mut entries = Vec::new();
                for (k, v) in obj {
                    let key_sym: ScSymbol =
                        k.as_str()
                            .try_into()
                            .map_err(|_| ParserError::InvalidSymbol {
                                location: format!("{}.{}", path, k),
                                details: "Key name too long for symbol".to_string(),
                            })?;
                    let key = ScVal::Symbol(key_sym);
                    let val = Self::parse_value(v, &format!("{}.{}", path, k))?;
                    entries.push(ScMapEntry { key, val });
                }

                entries.sort_by(|a, b| {
                    let a_bytes = a.key.to_xdr(Limits::none()).unwrap_or_default();
                    let b_bytes = b.key.to_xdr(Limits::none()).unwrap_or_default();
                    a_bytes.cmp(&b_bytes)
                });

                let map_m: VecM<ScMapEntry> =
                    entries.try_into().map_err(|_| ParserError::InvalidType {
                        location: path.to_string(),
                        expected: "smaller map".to_string(),
                        found: "map size exceeds limit".to_string(),
                    })?;
                Ok(ScVal::Map(Some(ScMap(map_m))))
            }
        }
    }

    fn parse_address(address: &str) -> Result<ScAddress, String> {
        let strkey = Strkey::from_string(address).map_err(|e| e.to_string())?;

        match strkey {
            Strkey::Contract(contract) => Ok(ScAddress::Contract(Hash(contract.0))),
            Strkey::PublicKeyEd25519(pubkey) => {
                Ok(ScAddress::Account(soroban_sdk::xdr::AccountId(
                    soroban_sdk::xdr::PublicKey::PublicKeyTypeEd25519(Uint256(pubkey.0)),
                )))
            }
            _ => Err("Unsupported address type".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::xdr::ScVal;

    #[test]
    fn test_parse_primitives() {
        assert!(matches!(ArgParser::parse("null").unwrap(), ScVal::Void));
        assert!(matches!(
            ArgParser::parse("true").unwrap(),
            ScVal::Bool(true)
        ));
        assert!(matches!(
            ArgParser::parse("false").unwrap(),
            ScVal::Bool(false)
        ));
        assert!(matches!(ArgParser::parse("123").unwrap(), ScVal::I64(123)));
        assert!(matches!(
            ArgParser::parse("-456").unwrap(),
            ScVal::I64(-456)
        ));
    }

    #[test]
    fn test_parse_string_and_symbol() {
        let s = ArgParser::parse("\"hello\"").unwrap();
        match s {
            ScVal::String(st) => {
                let bytes: Vec<u8> = st.0.into();
                assert_eq!(String::from_utf8(bytes).unwrap(), "hello");
            }
            _ => panic!("Expected String variant"),
        }

        let sym = ArgParser::parse("\":my_sym\"").unwrap();
        match sym {
            ScVal::Symbol(s) => {
                let bytes: Vec<u8> = s.0.into();
                assert_eq!(String::from_utf8(bytes).unwrap(), "my_sym");
            }
            _ => panic!("Expected Symbol variant"),
        }
    }

    #[test]
    fn test_parse_address() {
        // Valid strkeys from snapshots
        let account = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAGO6V";
        let contract = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM";

        let result = ArgParser::parse(&format!("\"{}\"", account)).unwrap();
        assert!(matches!(result, ScVal::Address(ScAddress::Account(_))));

        let result = ArgParser::parse(&format!("\"{}\"", contract)).unwrap();
        assert!(matches!(result, ScVal::Address(ScAddress::Contract(_))));
    }

    #[test]
    fn test_parse_complex_nested() {
        let json = r#"{
            "admin": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAGO6V",
            "config": {
                "threshold": 3,
                "active": true
            },
            "tags": [":tag1", ":tag2"]
        }"#;

        let result = ArgParser::parse(json).unwrap();
        if let ScVal::Map(Some(map)) = result {
            assert_eq!(map.0.len(), 3);
        } else {
            panic!("Expected Map");
        }
    }

    #[test]
    fn test_error_path() {
        let json = r#"{"a": {"b": [1, 1.5]}}"#;
        let result = ArgParser::parse(json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("$.a.b[1]"));
        assert!(err.contains("expected integer, found number 1.5"));
    }
}
