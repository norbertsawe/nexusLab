use super::*;
use nexus_script::{workspace, ScriptError};

pub(super) enum Request {
    Statistics(String),
    Evaluate {
        code: String,
        label: String,
        editor: bool,
    },
    ImportCsv {
        path: PathBuf,
        name: String,
    },
    SaveWorkspace(PathBuf),
    LoadWorkspace(PathBuf),
    Delete(String),
    Put(String, Value),
    ExportCsv(PathBuf, Value),
    ExportFigure(PathBuf, Figure, super::figures::ExportFormat),
}
pub(super) struct Job {
    pub request: Request,
    pub edit: Option<(String, Value)>,
}
pub(super) struct ScriptResult {
    pub statistics: Option<nexus_core::Statistics>,
    pub variables: BTreeMap<String, Value>,
    pub figures: BTreeMap<u32, Figure>,
    pub active_figure: u32,
    pub message: String,
    pub success: bool,
    pub plotted: bool,
    pub editor: bool,
    pub imported: Option<String>,
}
pub(super) struct ScriptWorker {
    pub tx: Option<mpsc::Sender<Job>>,
    pub rx: mpsc::Receiver<ScriptResult>,
    pub cancel: Arc<std::sync::atomic::AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl ScriptWorker {
    pub fn new(ctx: egui::Context) -> Self {
        let (tx, requests) = mpsc::channel::<Job>();
        let (responses, rx) = mpsc::channel();
        let mut session = Session::new();
        session
            .replace_workspace(BTreeMap::new())
            .expect("empty workspace is valid");
        let cancel = session.cancellation_token();
        let worker = std::thread::spawn(move || {
            while let Ok(job) = requests.recv() {
                let mut editor = false;
                let mut imported = None;
                let mut statistics = None;
                let result: Result<String, ScriptError> = (|| {
                    if let Some((name, value)) = job.edit {
                        session.insert_variable(name, value)?;
                    }
                    match job.request {
                        Request::Statistics(name) => {
                            let value = session.variables.get(&name).ok_or_else(|| {
                                ScriptError::Eval("variable no longer exists".into())
                            })?;
                            statistics = Some(nexus_core::statistics(&value.numeric_samples()?)?);
                            Ok(format!("Statistics calculated for {name}"))
                        }
                        Request::Evaluate {
                            code,
                            label,
                            editor: from_editor,
                        } => {
                            editor = from_editor;
                            let result = session
                                .evaluate(&code)
                                .map_err(|e| ScriptError::Eval(format!("{label}: {e}")))?;
                            Ok(format!(
                                "{}\nCompleted in {:.3} ms",
                                result.output, result.elapsed_ms
                            ))
                        }
                        Request::ImportCsv { path, name } => {
                            let m = workspace::import_csv(&path)?;
                            let (r, c) = (m.rows, m.cols);
                            session.insert_variable(name.clone(), Value::Real(m))?;
                            imported = Some(name.clone());
                            Ok(format!(
                                "Imported {name}: {r} × {c} from {}",
                                path.display()
                            ))
                        }
                        Request::SaveWorkspace(path) => {
                            workspace::save(&path, &session.variables)?;
                            Ok(format!("Workspace saved to {}", path.display()))
                        }
                        Request::LoadWorkspace(path) => {
                            session.replace_workspace(workspace::load(&path)?)?;
                            Ok(format!("Workspace loaded from {}", path.display()))
                        }
                        Request::Delete(name) => {
                            session.remove_variable(&name)?;
                            Ok(format!("Deleted {name}"))
                        }
                        Request::Put(name, value) => {
                            session.insert_variable(name.clone(), value)?;
                            Ok(format!("Applied edits to {name}"))
                        }
                        Request::ExportCsv(path, value) => {
                            workspace::export_csv(&path, &value)?;
                            Ok(format!("CSV exported to {}", path.display()))
                        }
                        Request::ExportFigure(path, figure, format) => {
                            workspace::atomic_write(
                                &path,
                                &super::figures::export_bytes(&figure, format)
                                    .map_err(ScriptError::Eval)?,
                            )?;
                            Ok(format!("Figure exported to {}", path.display()))
                        }
                    }
                })();
                let (success, message) = match result {
                    Ok(message) => (true, message),
                    Err(e) => (false, e.to_string()),
                };
                let plotted = session.plot.take().is_some();
                if responses
                    .send(ScriptResult {
                        statistics,
                        variables: session.variables.clone(),
                        figures: session.figures.clone(),
                        active_figure: session.active_figure(),
                        message,
                        success,
                        plotted,
                        editor,
                        imported,
                    })
                    .is_err()
                {
                    break;
                }
                ctx.request_repaint();
            }
        });
        Self {
            tx: Some(tx),
            rx,
            cancel,
            worker: Some(worker),
        }
    }
}
impl Drop for ScriptWorker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.tx.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editor_import_edit_save_and_load_use_one_session() {
        let worker = ScriptWorker::new(egui::Context::default());
        let send = |request, edit| {
            worker
                .tx
                .as_ref()
                .unwrap()
                .send(Job { request, edit })
                .unwrap();
            worker.rx.recv_timeout(Duration::from_secs(5)).unwrap()
        };
        let r = send(
            Request::Evaluate {
                code: documents::ANALYSIS.into(),
                label: "analysis.nx".into(),
                editor: true,
            },
            None,
        );
        assert!(r.success, "{}", r.message);
        assert!(r.variables.contains_key("rmse"));
        assert_eq!(r.figures[&1].series.len(), 2);
        let dir = tempfile::tempdir().unwrap();
        let csv = dir.path().join("data.csv");
        std::fs::write(&csv, "time,value\n0,10\n1,20\n").unwrap();
        let r = send(
            Request::ImportCsv {
                path: csv,
                name: "measurements".into(),
            },
            None,
        );
        assert!(r.success);
        assert_eq!(r.imported.as_deref(), Some("measurements"));
        let edited = Value::Real(Matrix::new(2, 2, vec![0.0, 11.0, 1.0, 22.0]).unwrap());
        let file = dir.path().join("workspace.nxw");
        let r = send(
            Request::SaveWorkspace(file.clone()),
            Some(("measurements".into(), edited.clone())),
        );
        assert!(r.success);
        assert_eq!(r.variables["measurements"], edited);
        let r = send(
            Request::Evaluate {
                code: "clear".into(),
                label: "test".into(),
                editor: false,
            },
            None,
        );
        assert_eq!(r.variables.len(), 1);
        let r = send(Request::LoadWorkspace(file), None);
        assert!(r.success);
        assert_eq!(r.variables["measurements"], edited);
        let r = send(
            Request::Evaluate {
                code: "plot(column(measurements,1),column(measurements,2))".into(),
                label: "test".into(),
                editor: false,
            },
            None,
        );
        assert!(r.success);
        assert_eq!(r.figures[&1].series[0].y, vec![11.0, 22.0]);
    }
}
