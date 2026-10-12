//! Instrument-neutral SCPI argument and IEEE 488.2 definite-length block support.
use crate::{encode_scpi, parse_scpi, HardwareError, SerialConfig};
use serde::Serialize;
use std::{
    io::{Read, Write},
    time::Duration,
};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Argument {
    Number { value: f64, unit: String },
    Integer(i64),
    Boolean(bool),
    Text(String),
    Keyword(String),
}
pub fn arguments(source: &str) -> Result<Vec<Argument>, HardwareError> {
    if source.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut pieces = Vec::new();
    let mut quote = None;
    let mut start = 0;
    for (i, c) in source.char_indices() {
        if matches!(c, '\'' | '"') {
            if quote == Some(c) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(c);
            }
        }
        if c == ',' && quote.is_none() {
            pieces.push(&source[start..i]);
            start = i + 1;
        }
    }
    if quote.is_some() {
        return Err(HardwareError::Protocol("unterminated SCPI string".into()));
    }
    pieces.push(&source[start..]);
    pieces
        .into_iter()
        .map(|s| {
            let s = s.trim();
            if s.is_empty() {
                return Err(HardwareError::Protocol("empty SCPI argument".into()));
            }
            if s.starts_with(['\'', '"']) {
                let q = s.chars().next().unwrap();
                if !s.ends_with(q) || s.len() < 2 {
                    return Err(HardwareError::Protocol("invalid quoted argument".into()));
                }
                return Ok(Argument::Text(
                    s[1..s.len() - 1].replace(&format!("{q}{q}"), &q.to_string()),
                ));
            }
            let upper = s.to_ascii_uppercase();
            if upper == "ON" {
                return Ok(Argument::Boolean(true));
            }
            if upper == "OFF" {
                return Ok(Argument::Boolean(false));
            }
            if let Some(prefix) = upper.strip_prefix('#') {
                let radix = match prefix.chars().next() {
                    Some('H') => 16,
                    Some('Q') => 8,
                    Some('B') => 2,
                    _ => {
                        return Err(HardwareError::Protocol(
                            "invalid radix prefix; binary blocks use decode_block".into(),
                        ))
                    }
                };
                return i64::from_str_radix(&prefix[1..], radix)
                    .map(Argument::Integer)
                    .map_err(|e| HardwareError::Protocol(e.to_string()));
            }
            // Find the longest numeric prefix, retaining the optional physical unit.
            let end = (1..=s.len())
                .filter(|&i| s.is_char_boundary(i))
                .rev()
                .find(|&i| s[..i].parse::<f64>().is_ok());
            if let Some(end) = end {
                let value = s[..end].parse::<f64>().unwrap();
                let unit = s[end..].trim();
                if !value.is_finite()
                    || !unit
                        .chars()
                        .all(|c| c.is_ascii_alphabetic() || matches!(c, '%' | '/' | '_'))
                {
                    return Err(HardwareError::Protocol(
                        "invalid numeric SCPI argument".into(),
                    ));
                }
                return Ok(Argument::Number {
                    value,
                    unit: unit.into(),
                });
            }
            if !s
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '+' | '-'))
            {
                return Err(HardwareError::Protocol("invalid SCPI keyword".into()));
            }
            Ok(Argument::Keyword(upper))
        })
        .collect()
}
pub fn encode_arguments(values: &[Argument]) -> String {
    values
        .iter()
        .map(|v| match v {
            Argument::Number { value, unit } => format!("{value}{unit}"),
            Argument::Integer(i) => format!("{i}"),
            Argument::Boolean(b) => {
                if *b {
                    "ON".into()
                } else {
                    "OFF".into()
                }
            }
            Argument::Text(s) => format!("\"{}\"", s.replace('"', "\"\"")),
            Argument::Keyword(s) => s.to_ascii_uppercase(),
        })
        .collect::<Vec<_>>()
        .join(",")
}
pub fn encode_block(payload: &[u8]) -> Result<Vec<u8>, HardwareError> {
    let len = payload.len().to_string();
    if len.len() > 9 {
        return Err(HardwareError::Protocol(
            "SCPI block exceeds nine length digits".into(),
        ));
    }
    let mut block = format!("#{}{len}", len.len()).into_bytes();
    block.extend_from_slice(payload);
    Ok(block)
}
pub fn decode_block(bytes: &[u8]) -> Result<(&[u8], usize), HardwareError> {
    if bytes.first() != Some(&b'#') {
        return Err(HardwareError::Protocol(
            "SCPI block must start with #".into(),
        ));
    }
    let n = bytes
        .get(1)
        .copied()
        .filter(u8::is_ascii_digit)
        .ok_or_else(|| HardwareError::Protocol("missing block length digit".into()))?
        .saturating_sub(b'0') as usize;
    if n == 0 {
        return Err(HardwareError::Protocol(
            "indefinite #0 blocks require an external message boundary".into(),
        ));
    }
    let digits = bytes
        .get(2..2 + n)
        .ok_or_else(|| HardwareError::Protocol("incomplete block header".into()))?;
    if !digits.iter().all(u8::is_ascii_digit) {
        return Err(HardwareError::Protocol("invalid block length".into()));
    }
    let len = std::str::from_utf8(digits)
        .unwrap()
        .parse::<usize>()
        .map_err(|e| HardwareError::Protocol(e.to_string()))?;
    let end = (2 + n)
        .checked_add(len)
        .ok_or_else(|| HardwareError::Protocol("block length overflow".into()))?;
    let payload = bytes
        .get(2 + n..end)
        .ok_or_else(|| HardwareError::Protocol("incomplete block payload".into()))?;
    Ok((payload, end))
}

trait InstrumentIo: Read + Write + Send {}
impl<T: Read + Write + Send> InstrumentIo for T {}
pub struct SerialInstrument {
    io: Box<dyn InstrumentIo>,
}
impl SerialInstrument {
    pub fn open(config: &SerialConfig) -> Result<Self, HardwareError> {
        let io = serialport::new(&config.port, config.baud)
            .parity(config.parity)
            .flow_control(config.flow_control)
            .timeout(Duration::from_secs(2))
            .open()?;
        Ok(Self { io: Box::new(io) })
    }
    pub fn write(&mut self, command: &str) -> Result<(), HardwareError> {
        self.io
            .write_all(encode_scpi(&parse_scpi(command)?).as_bytes())?;
        self.io.flush()?;
        Ok(())
    }
    pub fn query(&mut self, command: &str) -> Result<String, HardwareError> {
        if !parse_scpi(command)?.iter().any(|c| c.query) {
            return Err(HardwareError::Protocol("query requires a ? header".into()));
        }
        self.write(command)?;
        let mut out = Vec::new();
        let mut b = [0u8; 1];
        while out.len() < 65536 {
            self.io.read_exact(&mut b)?;
            if b[0] == b'\n' {
                return String::from_utf8(out).map_err(|e| HardwareError::Protocol(e.to_string()));
            }
            out.push(b[0]);
        }
        Err(HardwareError::Protocol(
            "SCPI response exceeds 64 KiB".into(),
        ))
    }
    pub fn query_block(&mut self, command: &str) -> Result<Vec<u8>, HardwareError> {
        if !parse_scpi(command)?.iter().any(|c| c.query) {
            return Err(HardwareError::Protocol("query requires a ? header".into()));
        }
        self.write(command)?;
        let mut prefix = [0u8; 2];
        self.io.read_exact(&mut prefix)?;
        if prefix[0] != b'#' || !matches!(prefix[1], b'1'..=b'9') {
            return Err(HardwareError::Protocol(
                "expected definite-length binary response".into(),
            ));
        }
        let n = (prefix[1] - b'0') as usize;
        let mut digits = vec![0; n];
        self.io.read_exact(&mut digits)?;
        let len = std::str::from_utf8(&digits)
            .map_err(|e| HardwareError::Protocol(e.to_string()))?
            .parse::<usize>()
            .map_err(|e| HardwareError::Protocol(e.to_string()))?;
        if len > 16 * 1024 * 1024 {
            return Err(HardwareError::Protocol("SCPI block exceeds 16 MiB".into()));
        }
        let mut result = vec![0; len];
        self.io.read_exact(&mut result)?;
        let mut terminator = [0u8; 1];
        self.io.read_exact(&mut terminator)?;
        if terminator[0] == b'\r' {
            self.io.read_exact(&mut terminator)?;
        }
        if terminator[0] != b'\n' {
            return Err(HardwareError::Protocol(
                "expected newline after SCPI block".into(),
            ));
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strings_units_radix() {
        let a = arguments("1.2E3Hz,OFF,\"a,b;\"\"c\",#HFF").unwrap();
        assert_eq!(
            a[0],
            Argument::Number {
                value: 1200.0,
                unit: "Hz".into()
            }
        );
        assert_eq!(a[3], Argument::Integer(255));
        assert!(matches!(&a[2],Argument::Text(s) if s=="a,b;\"c"));
        let a = arguments("2V,ON,\"hello\",AUTO").unwrap();
        assert_eq!(arguments(&encode_arguments(&a)).unwrap(), a);
    }
    #[test]
    fn binary_blocks_allow_delimiters_in_payload() {
        let block = encode_block(b"a;\n\0b").unwrap();
        let (payload, n) = decode_block(&block).unwrap();
        assert_eq!(payload, b"a;\n\0b");
        assert_eq!(n, block.len());
        assert!(decode_block(b"#210short").is_err());
        assert!(decode_block(b"#0unknown").is_err());
    }
}
