//! Validated workspace persistence and numeric CSV interchange shared by frontends.
use crate::{ScriptError, Value};
use nexus_core::Matrix;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io::Write, path::Path};

pub const MAX_WORKSPACE_BYTES: usize = 64 * 1024 * 1024;
pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
pub fn validate(variables: &BTreeMap<String, Value>) -> Result<(), ScriptError> {
    let mut bytes = 0usize;
    if variables.len() > 1024 {
        return Err(ScriptError::Eval("workspace exceeds 1024 variables".into()));
    }
    for (name, value) in variables {
        if !valid_name(name) {
            return Err(ScriptError::Eval(format!("invalid variable name: {name}")));
        }
        match value {
            Value::Real(m) => {
                if m.rows.checked_mul(m.cols) != Some(m.data.len()) || m.data.len() > 1_000_000 {
                    return Err(ScriptError::Eval(format!("invalid dimensions for {name}")));
                }
                if !m.data.iter().all(|v| v.is_finite()) {
                    return Err(ScriptError::Eval(format!(
                        "{name} contains nonfinite values and cannot be saved"
                    )));
                }
            }
            Value::Complex(v) => {
                if v.len() > 1_000_000 || !v.iter().flatten().all(|v| v.is_finite()) {
                    return Err(ScriptError::Eval(format!("invalid complex array: {name}")));
                }
            }
            Value::Text(s) => {
                if s.len() > 65536 {
                    return Err(ScriptError::Eval(format!("{name} exceeds text limit")));
                }
            }
        }
        bytes = bytes.saturating_add(value.bytes());
    }
    if bytes > MAX_WORKSPACE_BYTES {
        return Err(ScriptError::Eval("workspace exceeds 64 MiB".into()));
    }
    Ok(())
}
#[derive(Serialize, Deserialize)]
struct WorkspaceFile {
    format: String,
    version: u32,
    variables: BTreeMap<String, Value>,
}
pub fn save(path: &Path, variables: &BTreeMap<String, Value>) -> Result<(), ScriptError> {
    validate(variables)?;
    let data = WorkspaceFile {
        format: "nexuslab-workspace".into(),
        version: 1,
        variables: variables.clone(),
    };
    let bytes = serde_json::to_vec(&data).map_err(io_error)?;
    atomic_write(path, &bytes)
}
pub fn load(path: &Path) -> Result<BTreeMap<String, Value>, ScriptError> {
    let bytes = read_limited(path, 256 * 1024 * 1024)?;
    let data: WorkspaceFile = serde_json::from_slice(&bytes).map_err(io_error)?;
    if data.format != "nexuslab-workspace" || data.version != 1 {
        return Err(ScriptError::Eval(
            "unsupported workspace file format".into(),
        ));
    }
    validate(&data.variables)?;
    Ok(data.variables)
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), ScriptError> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(io_error)?;
    temp.write_all(bytes).map_err(io_error)?;
    temp.as_file().sync_all().map_err(io_error)?;
    temp.persist(path).map_err(io_error)?;
    Ok(())
}
pub fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>, ScriptError> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(io_error)?;
    if file.metadata().map_err(io_error)?.len() > limit {
        return Err(ScriptError::Eval(format!(
            "file exceeds {} MiB limit",
            limit / 1024 / 1024
        )));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() as u64 > limit {
        return Err(ScriptError::Eval("file grew beyond size limit".into()));
    }
    Ok(bytes)
}
fn io_error(error: impl std::fmt::Display) -> ScriptError {
    ScriptError::Eval(error.to_string())
}

pub fn import_csv(path: &Path) -> Result<Matrix, ScriptError> {
    parse_csv(&read_limited(path, 64 * 1024 * 1024)?)
}
pub fn parse_csv(bytes: &[u8]) -> Result<Matrix, ScriptError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .trim(csv::Trim::All)
        .comment(Some(b'#'))
        .from_reader(bytes);
    let mut data = Vec::new();
    let mut rows = 0;
    let mut cols = None;
    for (line, record) in reader.records().enumerate() {
        let record = record.map_err(io_error)?;
        if line == 0 && record.iter().all(|v| v.parse::<f64>().is_err()) {
            cols = Some(record.len());
            continue;
        }
        let width = *cols.get_or_insert(record.len());
        if record.len() != width || width == 0 {
            return Err(ScriptError::Eval(format!(
                "CSV row {} has inconsistent columns",
                line + 1
            )));
        }
        for cell in &record {
            let value = cell.parse::<f64>().map_err(|_| {
                ScriptError::Eval(format!(
                    "CSV row {} contains a nonnumeric cell: {cell}",
                    line + 1
                ))
            })?;
            if !value.is_finite() {
                return Err(ScriptError::Eval(format!(
                    "CSV row {} contains nonfinite data",
                    line + 1
                )));
            }
            data.push(value);
            if data.len() > 1_000_000 {
                return Err(ScriptError::Eval("CSV exceeds one million cells".into()));
            }
        }
        rows += 1;
    }
    if rows == 0 {
        return Err(ScriptError::Eval("CSV has no numeric rows".into()));
    }
    Ok(Matrix::new(rows, cols.unwrap_or(0), data)?)
}
pub fn export_csv(path: &Path, value: &Value) -> Result<(), ScriptError> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    match value {
        Value::Real(m) => {
            if m.cols == 0 {
                return Err(ScriptError::Eval("cannot export an empty matrix".into()));
            }
            if m.rows.checked_mul(m.cols) != Some(m.data.len()) {
                return Err(ScriptError::Eval("invalid matrix dimensions".into()));
            }
            for row in m.data.chunks(m.cols) {
                writer
                    .write_record(row.iter().map(|v| v.to_string()))
                    .map_err(io_error)?;
            }
        }
        Value::Complex(v) => {
            writer
                .write_record(["real", "imaginary"])
                .map_err(io_error)?;
            for pair in v {
                writer
                    .write_record(pair.map(|v| v.to_string()))
                    .map_err(io_error)?;
            }
        }
        Value::Text(_) => {
            return Err(ScriptError::Eval(
                "CSV export requires numerical data".into(),
            ))
        }
    }
    atomic_write(path, &writer.into_inner().map_err(io_error)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn csv_and_workspace_roundtrip_preserve_shape() {
        let dir = tempfile::tempdir().unwrap();
        let m = parse_csv(b"time,value\r\n0,1.25\r\n1,2.5\r\n").unwrap();
        assert_eq!((m.rows, m.cols), (2, 2));
        let variables = BTreeMap::from([("measurements".into(), Value::Real(m.clone()))]);
        let path = dir.path().join("session.nxw");
        save(&path, &variables).unwrap();
        assert_eq!(load(&path).unwrap(), variables);
        save(&path, &variables).unwrap(); // Existing files are replaced atomically on Windows too.
        let csv = dir.path().join("data.csv");
        export_csv(&csv, &Value::Real(m.clone())).unwrap();
        assert_eq!(import_csv(&csv).unwrap(), m);
    }
    #[test]
    fn malformed_data_does_not_enter_workspace() {
        assert!(parse_csv(b"1,2\n3\n").is_err());
        assert!(parse_csv(b"1,bad\n2,3").is_err());
        assert!(parse_csv(b"x,y\nNaN,2").is_err());
        let bad = Value::Real(Matrix {
            rows: 2,
            cols: 2,
            data: vec![1.0],
        });
        assert!(validate(&BTreeMap::from([("x".into(), bad)])).is_err());
        assert!(!valid_name("2x"));
    }
}
