use nexus_script::{workspace, ScriptError};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const ANALYSIS:&str="% Numerical analysis — press F5 or Ctrl+Enter to run\n% Fit a line to measured data and inspect the residuals.\nx = [0, 1, 2, 3, 4, 5];\ny = [1.2, 2.9, 5.1, 6.8, 9.2, 10.9];\ncoefficients = polyfit(x, y, 1);\nfitted = polyval(coefficients, x);\nresiduals = y - fitted;\nrmse = sqrt(mean(residuals .* residuals));\ndisp(coefficients);\ndisp(rmse);\n\nfigure(1);\nscatter(x, y);\nhold('on');\nplot(x, fitted);\ntitle('Measured data and linear fit');\nxlabel('Time (s)');\nylabel('Measurement');\n";
pub const MATRICES:&str="% Matrix operations and a linear system\nA = [4, 1; 2, 3];\nb = [1; 2];\nsolution = A \\ b\nidentity = A * inv(A)\ndeterminant = det(A)\n% Indexing starts at 1.\nfirst_value = solution(1)\n";
pub const FILTERING:&str="% DSP: remove high-frequency interference\nfs = 128000;\nt = 0:0.0000078125:0.01;\nraw = sin(2*pi*1000*t) + 0.3*sin(2*pi*15000*t);\ntaps = fir_lowpass(63, 3000, fs);\nfiltered = fir(raw, taps);\nfigure(1);\nplot(t, raw);\nhold('on');\nplot(t, filtered);\ntitle('Raw and filtered signals');\nxlabel('Time (s)');\nylabel('Amplitude');\n";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub id: u64,
    pub name: String,
    pub path: Option<PathBuf>,
    pub text: String,
    pub saved_text: String,
    pub rhai: bool,
}
impl Document {
    pub fn dirty(&self) -> bool {
        self.text != self.saved_text
    }
    pub fn source(&self) -> String {
        if self.rhai && !self.text.trim_start().starts_with("rhai:") {
            format!("rhai: {}", self.text)
        } else {
            self.text.clone()
        }
    }
    pub fn open(id: u64, path: &Path) -> Result<Self, ScriptError> {
        let bytes = workspace::read_limited(path, 1024 * 1024)?;
        let text = String::from_utf8(bytes)
            .map_err(|e| ScriptError::Eval(format!("Not a UTF-8 text file: {e}")))?
            .trim_start_matches('\u{feff}')
            .to_string();
        let path = path
            .canonicalize()
            .map_err(|e| ScriptError::Eval(e.to_string()))?;
        Ok(Self {
            id,
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            rhai: path.extension().is_some_and(|s| s == "rhai"),
            path: Some(path),
            saved_text: text.clone(),
            text,
        })
    }
    pub fn save_to(&mut self, path: PathBuf, explicit_overwrite: bool) -> Result<(), ScriptError> {
        if !explicit_overwrite && self.path.as_ref() == Some(&path) && path.exists() {
            let disk = String::from_utf8(workspace::read_limited(&path, 1024 * 1024)?)
                .map_err(|e| ScriptError::Eval(e.to_string()))?;
            if disk.trim_start_matches('\u{feff}') != self.saved_text {
                return Err(ScriptError::Eval("File changed on disk. Use Save As to keep your edits, or reopen the file to review the changes.".into()));
            }
        }
        workspace::atomic_write(&path, self.text.as_bytes())?;
        self.name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        self.path = Some(path);
        self.saved_text = self.text.clone();
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
pub struct Recovery {
    #[serde(default)]
    pub hardware_visible: bool,
    #[serde(default)]
    pub closed: Vec<String>,
    #[serde(default = "default_view")]
    pub view: String,
    #[serde(default)]
    pub detached: Vec<String>,
    pub version: u32,
    pub documents: Vec<Document>,
    pub active: usize,
    pub directory: PathBuf,
    pub history: Vec<String>,
}
fn default_view() -> String {
    "editor".into()
}
pub struct Documents {
    pub tabs: Vec<Document>,
    pub active: usize,
    next_id: u64,
}
impl Documents {
    pub fn new() -> Self {
        let mut docs = Self {
            tabs: Vec::new(),
            active: 0,
            next_id: 1,
        };
        docs.add("analysis.nx", ANALYSIS, false);
        docs
    }
    pub fn add(&mut self, name: &str, text: &str, rhai: bool) {
        self.tabs.push(Document {
            id: self.next_id,
            name: name.into(),
            path: None,
            text: text.into(),
            saved_text: text.into(),
            rhai,
        });
        self.next_id += 1;
        self.active = self.tabs.len() - 1;
    }
    pub fn open(&mut self, path: &Path) -> Result<(), ScriptError> {
        let canonical = path
            .canonicalize()
            .map_err(|e| ScriptError::Eval(e.to_string()))?;
        if let Some(index) = self
            .tabs
            .iter()
            .position(|d| d.path.as_ref() == Some(&canonical))
        {
            self.active = index;
            return Ok(());
        }
        if self.tabs.len() >= 32 {
            return Err(ScriptError::Eval(
                "Close a tab before opening another (maximum 32)".into(),
            ));
        }
        self.tabs.push(Document::open(self.next_id, &canonical)?);
        self.next_id += 1;
        self.active = self.tabs.len() - 1;
        Ok(())
    }
    pub fn current(&self) -> &Document {
        &self.tabs[self.active]
    }
    pub fn current_mut(&mut self) -> &mut Document {
        &mut self.tabs[self.active]
    }
    pub fn close(&mut self, index: usize) {
        self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.add("untitled.nx", "", false);
        } else if self.active > index {
            self.active -= 1;
        } else {
            self.active = self.active.min(self.tabs.len() - 1);
        }
    }
    pub fn restore(recovery: &Recovery) -> Option<Self> {
        if recovery.version != 1
            || recovery.documents.is_empty()
            || recovery.documents.len() > 32
            || recovery
                .documents
                .iter()
                .any(|d| d.text.len() > 1024 * 1024 || d.saved_text.len() > 1024 * 1024)
        {
            return None;
        }
        let next_id = recovery
            .documents
            .iter()
            .map(|d| d.id)
            .max()?
            .checked_add(1)?;
        Some(Self {
            tabs: recovery.documents.clone(),
            active: recovery.active.min(recovery.documents.len() - 1),
            next_id,
        })
    }
}
pub fn recovery_path() -> Option<PathBuf> {
    if std::env::var_os("NEXUSLAB_NO_RESTORE").is_some() {
        return None;
    }
    if let Some(path) = std::env::var_os("NEXUSLAB_STATE_DIR") {
        return Some(PathBuf::from(path).join("recovery.json"));
    }
    let root = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
    };
    root.map(|p| p.join("NexusLab").join("recovery.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saves_preserve_external_changes_and_support_save_as() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("original.nx");
        std::fs::write(&path, "x = 1").unwrap();
        let mut document = Document::open(1, &path).unwrap();
        document.text = "x = 2".into();
        document.save_to(path.clone(), false).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "x = 2");
        assert!(!document.dirty());
        std::fs::write(&path, "x = 3").unwrap();
        document.text = "x = 4".into();
        assert!(document.save_to(path.clone(), false).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "x = 3");
        document.save_to(dir.path().join("copy.nx"), true).unwrap();
        assert!(!document.dirty());
    }
    #[test]
    fn dirty_tabs_and_recovery_keep_unsaved_work() {
        let mut docs = Documents::new();
        docs.current_mut().text.push_str("\nx = 42");
        assert!(docs.current().dirty());
        let recovery = Recovery {
            hardware_visible: false,
            closed: vec!["editor".into(), "command".into()],
            view: "welcome".into(),
            detached: Vec::new(),
            version: 1,
            documents: docs.tabs.clone(),
            active: 0,
            directory: PathBuf::from("."),
            history: vec!["who".into()],
        };
        let restored = Documents::restore(&recovery).unwrap();
        assert!(restored.current().dirty());
        assert!(restored.current().text.ends_with("42"));
        docs.add("second.nx", "", false);
        docs.close(0);
        assert_eq!(docs.current().name, "second.nx");
    }
}
