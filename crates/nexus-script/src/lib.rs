//! Small MATLAB-style AST interpreter plus a resource-bounded Rhai engine.
mod indexing;
pub mod workspace;
use nexus_core::{fft, ifft, window, Generator, Matrix, Window};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, f64::consts::PI, time::Instant};
use thiserror::Error;

const MAX_ELEMENTS: usize = 1_000_000;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "value")]
pub enum Value {
    Real(Matrix),
    Complex(Vec<[f64; 2]>),
    ComplexColumn(Vec<[f64; 2]>),
    Text(String),
}
impl Value {
    pub fn numeric_samples(&self) -> Result<Vec<f64>, ScriptError> {
        match self {
            Self::Real(m) => Ok(m.data.clone()),
            Self::Complex(v) | Self::ComplexColumn(v) => {
                Ok(v.iter().map(|v| v[0].hypot(v[1])).collect())
            }
            Self::Text(_) => Err(ScriptError::Eval("numeric array required".into())),
        }
    }
    pub fn display(&self) -> String {
        match self {
            Self::Real(m) if m.data.len() == 1 => format!("{:.8}", m.data[0]),
            Self::Real(m) => {
                let mut lines = Vec::new();
                for r in 0..m.rows.min(12) {
                    lines.push(
                        (0..m.cols.min(8))
                            .map(|c| format!("{:>12.6}", m.data[r * m.cols + c]))
                            .collect::<Vec<_>>()
                            .join(" "),
                    );
                }
                if m.rows > 12 || m.cols > 8 {
                    lines.push(format!(
                        "… {} × {} array (use Variable Editor for all values)",
                        m.rows, m.cols
                    ));
                }
                lines.join("\n")
            }
            Self::Complex(v) | Self::ComplexColumn(v) => v
                .iter()
                .take(8)
                .map(|p| format!("{:.6} {:+.6}i", p[0], p[1]))
                .collect::<Vec<_>>()
                .join("\n"),
            Self::Text(s) => s.clone(),
        }
    }
    pub fn real(&self) -> Result<&Matrix, ScriptError> {
        match self {
            Self::Real(m) => Ok(m),
            _ => Err(ScriptError::Eval("real numeric array required".into())),
        }
    }
    pub fn scalar(&self) -> Result<f64, ScriptError> {
        let m = self.real()?;
        if m.data.len() != 1 {
            return Err(ScriptError::Eval("scalar required".into()));
        }
        Ok(m.data[0])
    }
    pub fn shape(&self) -> (usize, usize) {
        match self {
            Self::Real(m) => (m.rows, m.cols),
            Self::Complex(v) => (1, v.len()),
            Self::ComplexColumn(v) => (v.len(), 1),
            Self::Text(s) => (1, s.len()),
        }
    }
    pub fn bytes(&self) -> usize {
        match self {
            Self::Real(m) => m.data.len() * 8,
            Self::Complex(v) | Self::ComplexColumn(v) => v.len() * 16,
            Self::Text(s) => s.len(),
        }
    }
    pub fn preview(&self) -> String {
        match self {
            Self::Real(m) => {
                let values = m
                    .data
                    .iter()
                    .take(6)
                    .map(|v| format!("{v:.5}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{values}{}]", if m.data.len() > 6 { ", …" } else { "" })
            }
            Self::Complex(v) | Self::ComplexColumn(v) => format!("{} complex samples", v.len()),
            Self::Text(s) => s.clone(),
        }
    }
}
#[derive(Debug, Error)]
pub enum ScriptError {
    #[error("parse: {0}")]
    Parse(String),
    #[error("evaluation: {0}")]
    Eval(String),
    #[error(transparent)]
    Math(#[from] nexus_core::MathError),
}
#[derive(Clone, Debug, Serialize)]
pub struct Evaluation {
    pub value: Value,
    pub elapsed_ms: f64,
    pub output: String,
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(f64),
    Ident(String),
    Text(String),
    Symbol(char),
    Op(String),
    End,
}
fn lex(source: &str) -> Result<Vec<Token>, ScriptError> {
    let chars: Vec<_> = source.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    let mut delimiters = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c == '%' || (c == '/' && chars.get(i + 1) == Some(&'/')) {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '\n' {
            out.push(Token::Symbol('\n'));
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(|x| x.is_ascii_digit()))
        {
            let start = i;
            i += 1;
            while i < chars.len()
                && (chars[i].is_ascii_digit()
                    || chars[i] == '.'
                        && chars.get(i + 1) != Some(&'*')
                        && chars.get(i + 1) != Some(&'/')
                        && chars.get(i + 1) != Some(&'^'))
            {
                i += 1;
            }
            if i < chars.len() && matches!(chars[i], 'e' | 'E') {
                i += 1;
                if i < chars.len() && matches!(chars[i], '+' | '-') {
                    i += 1;
                }
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let text: String = chars[start..i].iter().collect();
            out.push(Token::Number(text.parse().map_err(|_| {
                ScriptError::Parse(format!("invalid number {text}"))
            })?));
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push(Token::Ident(chars[start..i].iter().collect()));
            continue;
        }
        if c == '"'
            || c == '\''
                && !matches!(
                    out.last(),
                    Some(Token::Number(_) | Token::Ident(_) | Token::Symbol(')' | ']' | '\''))
                )
        {
            let quote = c;
            i += 1;
            let mut value = String::new();
            let mut closed = false;
            while i < chars.len() {
                if chars[i] == quote {
                    if chars.get(i + 1) == Some(&quote) {
                        value.push(quote);
                        i += 2;
                        continue;
                    }
                    i += 1;
                    closed = true;
                    break;
                }
                value.push(chars[i]);
                i += 1;
            }
            if !closed {
                return Err(ScriptError::Parse("unterminated string".into()));
            }
            out.push(Token::Text(value));
            continue;
        }
        if c == '.'
            && chars
                .get(i + 1)
                .is_some_and(|x| matches!(x, '*' | '/' | '^'))
        {
            out.push(Token::Op(format!(".{}", chars[i + 1])));
            i += 2;
            continue;
        }
        // In a matrix literal, whitespace before an attached sign starts a new
        // element: [1 -2] differs from [1 - 2] and [1-2]. Parenthesized expressions
        // retain normal arithmetic regardless of surrounding matrix whitespace.
        if matches!(c, '+' | '-')
            && delimiters.last() == Some(&'[')
            && i > 0
            && chars[i - 1].is_whitespace()
            && chars.get(i + 1).is_some_and(|c| !c.is_whitespace())
            && matches!(
                out.last(),
                Some(Token::Number(_) | Token::Ident(_) | Token::Symbol(')' | ']' | '\''))
            )
        {
            out.push(Token::Symbol(','));
        }
        if "+-*/^\\".contains(c) {
            out.push(Token::Op(c.to_string()));
        } else if "()[],;:='".contains(c) {
            if matches!(c, '(' | '[') {
                delimiters.push(c);
            }
            if matches!(c, ')' | ']') {
                delimiters.pop();
            }
            out.push(Token::Symbol(c));
        } else {
            return Err(ScriptError::Parse(format!("unexpected character {c}")));
        }
        i += 1;
    }
    if out.len() > 4096 {
        return Err(ScriptError::Parse(
            "script exceeds 4096 tokens; use generated arrays".into(),
        ));
    }
    out.push(Token::End);
    Ok(out)
}
#[derive(Debug)]
enum Expr {
    AllIndex,
    Number(f64),
    Name(String),
    Text(String),
    Unary(Box<Expr>),
    Binary(String, Box<Expr>, Box<Expr>),
    Range(Box<Expr>, Box<Expr>, Option<Box<Expr>>),
    Matrix(Vec<Vec<Expr>>),
    Call(String, Vec<Expr>),
    Transpose(Box<Expr>),
}
struct Parser {
    tokens: Vec<Token>,
    at: usize,
    depth: usize,
}
impl Parser {
    fn token(&self) -> &Token {
        &self.tokens[self.at]
    }
    fn next(&mut self) -> Token {
        let t = self.token().clone();
        if t != Token::End {
            self.at += 1;
        }
        t
    }
    fn eat(&mut self, c: char) -> bool {
        if self.token() == &Token::Symbol(c) {
            self.next();
            true
        } else {
            false
        }
    }
    fn expect(&mut self, c: char) -> Result<(), ScriptError> {
        if self.eat(c) {
            Ok(())
        } else {
            Err(ScriptError::Parse(format!(
                "expected '{c}', found {:?}",
                self.token()
            )))
        }
    }
    fn expr(&mut self, min: u8) -> Result<Expr, ScriptError> {
        self.depth += 1;
        if self.depth > 128 {
            return Err(ScriptError::Parse(
                "expression nesting limit reached".into(),
            ));
        }
        let result = self.expr_inner(min);
        self.depth -= 1;
        result
    }
    fn expr_inner(&mut self, min: u8) -> Result<Expr, ScriptError> {
        let mut lhs = match self.next() {
            Token::Number(n) => Expr::Number(n),
            Token::Text(t) => Expr::Text(t),
            Token::Symbol(':') => Expr::AllIndex,
            Token::Ident(n) => {
                if self.eat('(') {
                    let mut args = Vec::new();
                    if !self.eat(')') {
                        loop {
                            args.push(self.expr(0)?);
                            if self.eat(')') {
                                break;
                            }
                            self.expect(',')?;
                        }
                    }
                    Expr::Call(n, args)
                } else {
                    Expr::Name(n)
                }
            }
            Token::Op(op) if op == "-" => Expr::Unary(Box::new(self.expr(6)?)),
            Token::Op(op) if op == "+" => self.expr(6)?,
            Token::Symbol('(') => {
                let e = self.expr(0)?;
                self.expect(')')?;
                e
            }
            Token::Symbol('[') => {
                let mut rows = Vec::new();
                let mut row = Vec::new();
                while !self.eat(']') {
                    if self.token() == &Token::End {
                        return Err(ScriptError::Parse("unclosed matrix".into()));
                    }
                    if self.eat(';') || self.eat('\n') {
                        if !row.is_empty() {
                            rows.push(std::mem::take(&mut row));
                        }
                        continue;
                    }
                    row.push(self.expr(0)?);
                    self.eat(',');
                }
                if !row.is_empty() {
                    rows.push(row);
                }
                Expr::Matrix(rows)
            }
            t => {
                return Err(ScriptError::Parse(format!(
                    "expected expression, found {t:?}"
                )))
            }
        };
        loop {
            if self.token() == &Token::Symbol('\'') {
                self.next();
                lhs = Expr::Transpose(Box::new(lhs));
                continue;
            }
            if self.token() == &Token::Symbol(':') {
                if min > 1 {
                    break;
                }
                self.next();
                let middle = self.expr(2)?;
                let end = if self.eat(':') {
                    Some(Box::new(self.expr(2)?))
                } else {
                    None
                };
                lhs = Expr::Range(Box::new(lhs), Box::new(middle), end);
                continue;
            }
            let (op, prec) = match self.token() {
                Token::Op(op) => (
                    op.clone(),
                    match op.as_str() {
                        "+" | "-" => 3,
                        "*" | "/" | ".*" | "./" | "\\" => 5,
                        "^" | ".^" => 7,
                        _ => 0,
                    },
                ),
                _ => break,
            };
            if prec < min {
                break;
            }
            self.next();
            let rhs = self.expr(if prec == 7 { prec } else { prec + 1 })?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum PlotKind {
    Line,
    Scatter,
    Bar,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlotSeries {
    pub name: String,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub kind: PlotKind,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Figure {
    pub id: u32,
    pub title: String,
    pub x_label: String,
    pub y_label: String,
    pub grid: bool,
    pub series: Vec<PlotSeries>,
}
impl Figure {
    fn new(id: u32) -> Self {
        Self {
            id,
            title: format!("Figure {id}"),
            x_label: "x".into(),
            y_label: "y".into(),
            grid: true,
            series: Vec::new(),
        }
    }
}
pub const HELP:&str="NexusLab command window\n\nWorkspace: who, whos, clear, clear name\nMath: sin, cos, sqrt, exp, log, log10, abs, floor, ceil, round\nArrays: size, length, zeros, ones, eye, diag, reshape, transpose\nStatistics: stats, mean, median, std, rms, ptp, sum, min, max, diff, cumsum, sort, norm\nLinear algebra: inv, det, eig, pinv(A), pinv(A,tolerance), solve, A^n, A / B, A * B, A \\ b\nRegression: polyfit(x,y,degree), polyval(coefficients,x)\nDSP: fft, ifft, psd(x,fs,segment), freqz(coeff,fs), hann, fir_lowpass, filter(x,taps), butter_lowpass(order,cutoff,fs), butter_highpass, butter_bandpass, sosfilt(x,sos), iir_lowpass\nFigures: figure(), plot(y), plot(x,y), scatter(x,y), bar(x,y)\nStyling: title(\"text\"), xlabel(\"text\"), ylabel(\"text\"), hold on, hold off\nIndexing: A(row,col), A(:,2), A(1:end), A(:), A(indices)=values, v(index) — indices start at 1\n\nUse semicolons to suppress output. Use rhai: for loops and conditionals.\nDesktop commands: clc, pwd, cd folder, run file.nx";

pub struct Session {
    pub variables: BTreeMap<String, Value>,
    pub plot: Option<(Vec<f64>, Vec<f64>)>,
    engine: rhai::Engine,
    scope: rhai::Scope<'static>,
    rng: Generator,
    eval_depth: usize,
    index_limit: Option<usize>,
    printed: std::sync::Arc<std::sync::Mutex<String>>,
    pub figures: BTreeMap<u32, Figure>,
    current_figure: u32,
    hold: bool,
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}
impl Session {
    pub fn active_figure(&self) -> u32 {
        self.current_figure
    }
    pub fn cancellation_token(&self) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
        self.cancelled.clone()
    }
    pub fn replace_workspace(
        &mut self,
        mut variables: BTreeMap<String, Value>,
    ) -> Result<(), ScriptError> {
        workspace::validate(&variables)?;
        variables.insert("pi".into(), Value::Real(Matrix::scalar(PI)));
        self.variables = variables;
        self.scope = rhai::Scope::new();
        self.figures.clear();
        self.plot = None;
        Ok(())
    }
    pub fn insert_variable(&mut self, name: String, value: Value) -> Result<(), ScriptError> {
        if name == "pi" {
            return Err(ScriptError::Eval("pi is a constant".into()));
        }
        let mut variables = self.variables.clone();
        variables.insert(name, value);
        workspace::validate(&variables)?;
        self.variables = variables;
        self.scope = rhai::Scope::new();
        Ok(())
    }
    pub fn remove_variable(&mut self, name: &str) -> Result<(), ScriptError> {
        if name == "pi" {
            return Err(ScriptError::Eval("pi is a constant".into()));
        }
        self.variables.remove(name);
        self.scope = rhai::Scope::new();
        Ok(())
    }
    fn workspace_command(&mut self, source: &str) -> Result<Option<String>, ScriptError> {
        let words: Vec<_> = source.split_whitespace().collect();
        let Some(command) = words.first() else {
            return Ok(None);
        };
        let message = match *command {
            "help" if words.len() == 1 => HELP.into(),
            "who" if words.len() == 1 => self
                .variables
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join("  "),
            "whos" if words.len() == 1 => {
                let mut text =
                    "Name                 Type       Dimensions       Bytes\n".to_string();
                for (name, value) in &self.variables {
                    let (r, c) = value.shape();
                    let kind = match value {
                        Value::Real(_) => "f64",
                        Value::Complex(_) | Value::ComplexColumn(_) => "c128",
                        Value::Text(_) => "text",
                    };
                    text.push_str(&format!(
                        "{name:<20} {kind:<10} {r}x{c:<12} {}\n",
                        value.bytes()
                    ));
                }
                text
            }
            "clear" => {
                if words.len() == 1 || words[1] == "all" {
                    self.replace_workspace(BTreeMap::new())?;
                    "Workspace cleared".into()
                } else {
                    for name in &words[1..] {
                        self.remove_variable(name)?;
                    }
                    format!("Cleared {}", words[1..].join(", "))
                }
            }
            "hold" if words.len() == 2 && matches!(words[1], "on" | "off") => {
                self.hold = words[1] == "on";
                format!("Hold {}", words[1])
            }
            "close" if words.as_slice() == ["close", "all"] => {
                self.figures.clear();
                self.plot = None;
                "Figures closed".into()
            }
            _ => return Ok(None),
        };
        Ok(Some(message))
    }
    pub fn new() -> Self {
        let mut engine = rhai::Engine::new();
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let progress_cancel = cancelled.clone();
        engine.on_progress(move |_| {
            if progress_cancel.load(std::sync::atomic::Ordering::Relaxed) {
                Some(rhai::Dynamic::from("Execution cancelled"))
            } else {
                None
            }
        });
        let printed = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let print_target = printed.clone();
        engine.on_print(move |text| {
            let mut output = print_target.lock().unwrap();
            if output.len() + text.len() < 65536 {
                output.push_str(text);
                output.push('\n');
            }
        });
        engine.on_debug(|_, _, _| {});
        engine
            .set_max_variables(1024)
            .set_max_functions(128)
            .set_max_map_size(1024);
        engine
            .set_max_operations(200_000)
            .set_max_call_levels(32)
            .set_max_expr_depths(64, 32)
            .set_max_array_size(MAX_ELEMENTS)
            .set_max_string_size(65536);
        engine.register_fn("sin", f64::sin);
        engine.register_fn("cos", f64::cos);
        engine.register_fn("sqrt", f64::sqrt);
        engine.register_fn("fft", |array: rhai::Array| -> rhai::Array {
            let real: Vec<f64> = array
                .iter()
                .map(|v| {
                    v.clone()
                        .try_cast::<f64>()
                        .unwrap_or_else(|| v.clone().try_cast::<rhai::INT>().unwrap_or(0) as f64)
                })
                .collect();
            fft(&real)
                .iter()
                .map(|c| {
                    rhai::Dynamic::from_array(vec![
                        rhai::Dynamic::from(c.re),
                        rhai::Dynamic::from(c.im),
                    ])
                })
                .collect()
        });
        engine.register_fn("serial_ports", || -> rhai::Array {
            nexus_hardware::ports()
                .unwrap_or_default()
                .iter()
                .map(|p| rhai::Dynamic::from(p.name.clone()))
                .collect()
        });
        let mut s = Self {
            variables: BTreeMap::new(),
            plot: None,
            engine,
            scope: rhai::Scope::new(),
            rng: Generator::new(128000.0),
            eval_depth: 0,
            index_limit: None,
            printed,
            figures: BTreeMap::new(),
            current_figure: 1,
            hold: false,
            cancelled,
        };
        s.variables
            .insert("pi".into(), Value::Real(Matrix::scalar(PI)));
        s.variables.insert(
            "t".into(),
            Value::Real(Matrix::vector(
                (0..1280).map(|i| i as f64 / 128000.0).collect(),
            )),
        );
        s.evaluate("sig1 = 2.5 * sin(2 * pi * 1000 * t)")
            .expect("built-in example is valid");
        s
    }
    pub fn evaluate(&mut self, source: &str) -> Result<Evaluation, ScriptError> {
        self.cancelled
            .store(false, std::sync::atomic::Ordering::Relaxed);
        if let Some(message) = self.workspace_command(source.trim())? {
            return Ok(Evaluation {
                value: Value::Text(message.clone()),
                elapsed_ms: 0.0,
                output: message,
            });
        }
        if source.len() > 65536 {
            return Err(ScriptError::Eval("script exceeds 64 KiB limit".into()));
        }
        let started = Instant::now();
        self.printed.lock().unwrap().clear();
        let mut transcript = String::new();
        let value = if let Some(code) = source.trim().strip_prefix("rhai:") {
            let value = self.eval_rhai(code)?;
            transcript = value.display();
            value
        } else {
            let mut p = Parser {
                tokens: lex(source)?,
                at: 0,
                depth: 0,
            };
            let mut last = Value::Real(Matrix::scalar(0.0));
            while p.token() != &Token::End {
                if p.eat(';') || p.eat('\n') {
                    continue;
                }
                let first = p.expr(0)?;
                let assignment = p.eat('=');
                let (target, e) = if assignment {
                    (Some(first), p.expr(0)?)
                } else {
                    (None, first)
                };
                let name = target
                    .as_ref()
                    .map(|target| match target {
                        Expr::Name(n) | Expr::Call(n, _) => Ok(n.clone()),
                        _ => Err(ScriptError::Parse(
                            "assignment requires a variable or indexed variable".into(),
                        )),
                    })
                    .transpose()?;
                let suppressed = p.eat(';');
                if p.token() != &Token::End && !suppressed && !p.eat('\n') {
                    return Err(ScriptError::Parse("expected statement separator".into()));
                }
                last = self.eval(&e)?;
                if self.variables.values().map(Value::bytes).sum::<usize>() + last.bytes()
                    > 64 * 1024 * 1024
                {
                    return Err(ScriptError::Eval(
                        "workspace memory limit of 64 MiB reached".into(),
                    ));
                }
                if !suppressed && transcript.len() < 65536 {
                    if let Some(n) = &name {
                        transcript.push_str(&format!("{n} =\n"));
                    }
                    transcript.push_str(&last.display());
                    transcript.push('\n');
                }
                if let Some(n) = name {
                    if n == "pi" {
                        return Err(ScriptError::Eval("pi is a constant".into()));
                    }
                    if let Some(Expr::Call(_, indices)) = target {
                        self.assign_indices(&n, &indices, &last)?;
                    } else {
                        self.variables.insert(n, last.clone());
                    }
                }
            }
            self.variables.insert("ans".into(), last.clone());
            last
        };
        Ok(Evaluation {
            output: format!(
                "{}{}",
                self.printed.lock().unwrap(),
                if transcript.trim().is_empty() {
                    "Done"
                } else {
                    transcript.trim_end()
                }
            ),
            value,
            elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
        })
    }
    fn eval_rhai(&mut self, source: &str) -> Result<Value, ScriptError> {
        for (n, v) in &self.variables {
            let d = match v {
                Value::Real(m) if m.data.len() == 1 => rhai::Dynamic::from(m.data[0]),
                Value::Real(m) => rhai::Dynamic::from_array(
                    m.data.iter().map(|v| rhai::Dynamic::from(*v)).collect(),
                ),
                _ => continue,
            };
            self.scope.set_or_push(n.clone(), d);
        }
        let result = self
            .engine
            .eval_with_scope::<rhai::Dynamic>(&mut self.scope, source)
            .map_err(|e| ScriptError::Eval(e.to_string()))?;
        for (name, _, v) in self.scope.iter() {
            if name != "pi" {
                if let Some(value) = from_dynamic(v) {
                    self.variables.insert(name.into(), value);
                }
            }
        }
        Ok(from_dynamic(result).unwrap_or(Value::Text("Done".into())))
    }
    fn eval(&mut self, e: &Expr) -> Result<Value, ScriptError> {
        if self.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(ScriptError::Eval("Execution cancelled".into()));
        }
        self.eval_depth += 1;
        if self.eval_depth > 128 {
            self.eval_depth -= 1;
            return Err(ScriptError::Eval("evaluation nesting limit reached".into()));
        }
        let result = self.eval_inner(e);
        self.eval_depth -= 1;
        result
    }
    fn eval_inner(&mut self, e: &Expr) -> Result<Value, ScriptError> {
        Ok(match e {
            Expr::AllIndex => {
                let limit = self
                    .index_limit
                    .ok_or_else(|| ScriptError::Eval("bare : is only valid in an index".into()))?;
                Value::Real(Matrix::new(
                    limit,
                    1,
                    (1..=limit).map(|i| i as f64).collect(),
                )?)
            }
            Expr::Name(n) if n == "end" && self.index_limit.is_some() => {
                Value::Real(Matrix::scalar(self.index_limit.unwrap() as f64))
            }
            Expr::Number(n) => Value::Real(Matrix::scalar(*n)),
            Expr::Text(s) => Value::Text(s.clone()),
            Expr::Name(n) => self
                .variables
                .get(n)
                .cloned()
                .ok_or_else(|| ScriptError::Eval(format!("unknown variable '{n}'")))?,
            Expr::Unary(e) => Value::Real(self.eval(e)?.real()?.map(|x| -x)),
            Expr::Transpose(e) => match self.eval(e)? {
                Value::Real(m) => Value::Real(m.transpose()),
                Value::Complex(v) => {
                    Value::ComplexColumn(v.into_iter().map(|[re, im]| [re, -im]).collect())
                }
                Value::ComplexColumn(v) => {
                    Value::Complex(v.into_iter().map(|[re, im]| [re, -im]).collect())
                }
                Value::Text(_) => {
                    return Err(ScriptError::Eval("numeric transpose required".into()))
                }
            },
            Expr::Binary(op, a, b) => {
                let a = self.eval(a)?;
                let b = self.eval(b)?;
                let (a, b) = (a.real()?, b.real()?);
                Value::Real(match op.as_str() {
                    "+" => a.elementwise(b, |a, b| a + b)?,
                    "-" => a.elementwise(b, |a, b| a - b)?,
                    "*" => {
                        if a.rows.saturating_mul(a.cols).saturating_mul(b.cols) > 50_000_000 {
                            return Err(ScriptError::Eval(
                                "matrix multiplication work limit reached".into(),
                            ));
                        }
                        a.matmul(b)?
                    }
                    ".*" => a.elementwise(b, |a, b| a * b)?,
                    "./" => a.elementwise(b, |a, b| a / b)?,
                    "/" if b.data.len() == 1 => a.elementwise(b, |a, b| a / b)?,
                    "/" => b.transpose().solve(&a.transpose())?.transpose(),
                    ".^" => a.elementwise(b, f64::powf)?,
                    "^" if b.data.len() == 1 => a.power(b.data[0])?,
                    "^" => return Err(ScriptError::Eval(
                        "matrix power requires a scalar exponent; use .^ for element-wise powers"
                            .into(),
                    )),
                    "\\" => {
                        if a.rows > 256 || a.cols > 256 || b.cols > 256 {
                            return Err(ScriptError::Eval("solve dimension limit is 256".into()));
                        }
                        a.solve(b)?
                    }
                    _ => unreachable!(),
                })
            }
            Expr::Range(a, b, c) => {
                let start = self.eval(a)?.scalar()?;
                let middle = self.eval(b)?.scalar()?;
                let (step, end) = if let Some(c) = c {
                    (middle, self.eval(c)?.scalar()?)
                } else {
                    (1.0, middle)
                };
                if !start.is_finite() || !step.is_finite() || !end.is_finite() || step == 0.0 {
                    return Err(ScriptError::Eval(
                        "range needs finite values and nonzero step".into(),
                    ));
                }
                let count = ((end - start) / step + 1e-10).floor() + 1.0;
                if count > MAX_ELEMENTS as f64 {
                    return Err(ScriptError::Eval(
                        "range exceeds one million elements".into(),
                    ));
                }
                Value::Real(Matrix::vector(
                    (0..count.max(0.0) as usize)
                        .map(|i| start + i as f64 * step)
                        .collect(),
                ))
            }
            Expr::Matrix(rows) => {
                let mut data = Vec::new();
                let mut cols = None;
                for row in rows {
                    let start = data.len();
                    for e in row {
                        let v = self.eval(e)?;
                        let m = v.real()?;
                        if m.rows > 1 {
                            return Err(ScriptError::Eval(
                                "nested matrix must be a row vector".into(),
                            ));
                        }
                        data.extend_from_slice(&m.data);
                    }
                    let n = data.len() - start;
                    if cols.is_some_and(|c| c != n) {
                        return Err(ScriptError::Eval(
                            "matrix rows have different lengths".into(),
                        ));
                    }
                    cols = Some(n);
                    if data.len() > MAX_ELEMENTS {
                        return Err(ScriptError::Eval("matrix element limit reached".into()));
                    }
                }
                Value::Real(Matrix::new(rows.len(), cols.unwrap_or(0), data)?)
            }
            Expr::Call(name, args) => {
                if let Some(Value::Real(matrix)) = self.variables.get(name).cloned() {
                    return self.read_indices(&matrix, args);
                }
                let args = args
                    .iter()
                    .map(|e| self.eval(e))
                    .collect::<Result<Vec<_>, _>>()?;
                self.call(name, &args)?
            }
        })
    }
    fn call(&mut self, name: &str, args: &[Value]) -> Result<Value, ScriptError> {
        if let Some(Value::Real(m)) = self.variables.get(name) {
            let indices = args
                .iter()
                .map(Value::scalar)
                .collect::<Result<Vec<_>, _>>()?;
            let index = |n: f64, limit: usize| {
                if n.is_finite() && n.fract() == 0.0 && n >= 1.0 && n <= limit as f64 {
                    Ok(n as usize - 1)
                } else {
                    Err(ScriptError::Eval(format!(
                        "{name}: index is outside 1..{limit}"
                    )))
                }
            };
            let offset = match indices.as_slice() {
                [n] => {
                    let i = index(*n, m.data.len())?;
                    (i % m.rows) * m.cols + i / m.rows
                }
                [r, c] => index(*r, m.rows)? * m.cols + index(*c, m.cols)?,
                _ => {
                    return Err(ScriptError::Eval(
                        "indexing expects one or two indices".into(),
                    ))
                }
            };
            return Ok(Value::Real(Matrix::scalar(m.data[offset])));
        }
        let arg = |i: usize| {
            args.get(i)
                .ok_or_else(|| ScriptError::Eval(format!("{name}: missing argument {}", i + 1)))
        };
        let count = |x: f64| -> Result<usize, ScriptError> {
            if !x.is_finite() || x < 0.0 || x.fract() != 0.0 || x > MAX_ELEMENTS as f64 {
                Err(ScriptError::Eval(
                    "size must be an integer from 0 to 1000000".into(),
                ))
            } else {
                Ok(x as usize)
            }
        };
        let arity = match name {
            "figure" => args.len().min(1),
            "plot" => {
                if args.len() == 1 {
                    1
                } else {
                    2
                }
            }
            "help" => 0,
            "pinv" => {
                if args.len() == 2 {
                    2
                } else {
                    1
                }
            }
            "zeros" | "ones" | "randn" | "eye" | "size" => {
                if args.len() == 2 {
                    2
                } else {
                    1
                }
            }
            "reshape" | "polyfit" => 3,
            "polyval" | "scatter" | "bar" | "row" | "column" => 2,
            "linspace" | "fir_lowpass" | "fir_highpass" => 3,
            "fir_bandpass" | "iir_lowpass" | "iir_highpass" | "iir_bandpass" => 4,
            "solve" | "fir" | "filter" | "sosfilt" | "freqz" => 2,
            "psd" | "butter_lowpass" | "butter_highpass" => 3,
            "butter_bandpass" => 4,
            "serial_ports" | "usb_devices" => 0,
            _ => 1,
        };
        if args.len() != arity {
            return Err(ScriptError::Eval(format!(
                "{name} expects {arity} arguments"
            )));
        }
        Ok(match name {
            "eig" => {
                let values = arg(0)?.real()?.eigenvalues()?;
                if values.iter().all(|v| v.im == 0.0) {
                    Value::Real(Matrix::new(
                        values.len(),
                        1,
                        values.iter().map(|v| v.re).collect(),
                    )?)
                } else {
                    Value::ComplexColumn(values.iter().map(|v| [v.re, v.im]).collect())
                }
            }
            "pinv" => Value::Real(
                arg(0)?
                    .real()?
                    .pseudoinverse(args.get(1).map(Value::scalar).transpose()?)?,
            ),
            "stats" | "median" | "rms" | "ptp" => {
                let s = nexus_core::statistics(&arg(0)?.numeric_samples()?)?;
                if name == "stats" {
                    Value::Text(format!("Count: {}\nMean: {:.8}\nMedian: {:.8}\nSample standard deviation: {:.8}\nRMS: {:.8}\nPeak-to-peak: {:.8}",s.count,s.mean,s.median,s.std_dev,s.rms,s.peak_to_peak))
                } else {
                    Value::Real(Matrix::scalar(match name {
                        "median" => s.median,
                        "rms" => s.rms,
                        _ => s.peak_to_peak,
                    }))
                }
            }
            "psd" => Value::Real(nexus_core::welch_psd(
                &arg(0)?.real()?.data,
                arg(1)?.scalar()?,
                count(arg(2)?.scalar()?)?,
            )?),
            "butter_lowpass" | "butter_highpass" | "butter_bandpass" => {
                let kind = match name {
                    "butter_lowpass" => nexus_core::FilterKind::Lowpass,
                    "butter_highpass" => nexus_core::FilterKind::Highpass,
                    _ => nexus_core::FilterKind::Bandpass,
                };
                let low = arg(1)?.scalar()?;
                let (high, rate) = if name == "butter_bandpass" {
                    (arg(2)?.scalar()?, arg(3)?.scalar()?)
                } else {
                    (low, arg(2)?.scalar()?)
                };
                Value::Real(nexus_core::butterworth(
                    count(arg(0)?.scalar()?)?,
                    kind,
                    low,
                    high,
                    rate,
                )?)
            }
            "sosfilt" => Value::Real(Matrix::vector(nexus_core::filter_sos(
                &arg(0)?.real()?.data,
                arg(1)?.real()?,
            )?)),
            "freqz" => {
                let coeff = arg(0)?.real()?;
                let rate = arg(1)?.scalar()?;
                if !rate.is_finite()
                    || rate <= 0.0
                    || coeff.data.is_empty()
                    || coeff.data.len() > 4095
                    || !coeff.data.iter().all(|x| x.is_finite())
                {
                    return Err(ScriptError::Eval(
                        "freqz needs finite coefficients (maximum 4095) and a positive sample rate"
                            .into(),
                    ));
                }
                let mut response = Vec::with_capacity(1026);
                for i in 0..513 {
                    let f = i as f64 * rate / 1024.0;
                    let h = if coeff.cols == 6 {
                        nexus_core::sos_response(coeff, f, rate)?
                    } else {
                        let z = nexus_core::Complex::from_polar(1.0, -2.0 * PI * f / rate);
                        coeff
                            .data
                            .iter()
                            .rev()
                            .fold(nexus_core::Complex::new(0.0, 0.0), |h, c| h * z + c)
                    };
                    response.extend([f, 20.0 * h.norm().max(1e-8).log10()]);
                }
                Value::Real(Matrix::new(513, 2, response)?)
            }
            "help" => Value::Text(HELP.into()),
            "disp" => {
                let message = arg(0)?.display();
                self.printed
                    .lock()
                    .unwrap()
                    .push_str(&format!("{message}\n"));
                Value::Text(String::new())
            }
            "sin" | "cos" | "tan" | "sqrt" | "exp" | "log" | "log10" | "floor" | "ceil"
            | "round" | "abs" => {
                if name == "abs" && matches!(arg(0)?, Value::Complex(_) | Value::ComplexColumn(_)) {
                    let value = arg(0)?;
                    let (rows, cols) = value.shape();
                    return Ok(Value::Real(Matrix::new(
                        rows,
                        cols,
                        value.numeric_samples()?,
                    )?));
                }
                let f: fn(f64) -> f64 = match name {
                    "sin" => f64::sin,
                    "cos" => f64::cos,
                    "tan" => f64::tan,
                    "sqrt" => f64::sqrt,
                    "exp" => f64::exp,
                    "log" => f64::ln,
                    "log10" => f64::log10,
                    "floor" => f64::floor,
                    "ceil" => f64::ceil,
                    "round" => f64::round,
                    _ => f64::abs,
                };
                Value::Real(arg(0)?.real()?.map(f))
            }
            "fft" => Value::Complex(
                fft(&arg(0)?.real()?.data)
                    .iter()
                    .map(|v| [v.re, v.im])
                    .collect(),
            ),
            "ifft" => {
                let (Value::Complex(v) | Value::ComplexColumn(v)) = arg(0)? else {
                    return Err(ScriptError::Eval("ifft requires complex spectrum".into()));
                };
                Value::Real(Matrix::vector(
                    ifft(
                        &v.iter()
                            .map(|v| nexus_core::Complex::new(v[0], v[1]))
                            .collect::<Vec<_>>(),
                    )
                    .iter()
                    .map(|v| v.re)
                    .collect(),
                ))
            }
            "transpose" => Value::Real(arg(0)?.real()?.transpose()),
            "row" | "column" => {
                let m = arg(0)?.real()?;
                let index = count(arg(1)?.scalar()?)?;
                let limit = if name == "row" { m.rows } else { m.cols };
                if index == 0 || index > limit {
                    return Err(ScriptError::Eval(format!(
                        "{name} index is outside 1..{limit}"
                    )));
                }
                Value::Real(Matrix::vector(if name == "row" {
                    m.data[(index - 1) * m.cols..index * m.cols].to_vec()
                } else {
                    (0..m.rows)
                        .map(|r| m.data[r * m.cols + index - 1])
                        .collect()
                }))
            }
            "det" => {
                let m = arg(0)?.real()?;
                if m.rows > 256 {
                    return Err(ScriptError::Eval("det dimension limit is 256".into()));
                }
                Value::Real(Matrix::scalar(m.determinant()?))
            }
            "eye" => {
                let rows = count(arg(0)?.scalar()?)?;
                let cols = if args.len() == 2 {
                    count(arg(1)?.scalar()?)?
                } else {
                    rows
                };
                if rows.saturating_mul(cols) > MAX_ELEMENTS {
                    return Err(ScriptError::Eval("eye exceeds array size limit".into()));
                }
                Value::Real(Matrix::new(
                    rows,
                    cols,
                    (0..rows * cols)
                        .map(|i| if i / cols == i % cols { 1.0 } else { 0.0 })
                        .collect(),
                )?)
            }
            "diag" => {
                let m = arg(0)?.real()?;
                if m.rows == 1 || m.cols == 1 {
                    let n = m.data.len();
                    if n.saturating_mul(n) > MAX_ELEMENTS {
                        return Err(ScriptError::Eval("diag exceeds array size limit".into()));
                    }
                    Value::Real(Matrix::new(
                        n,
                        n,
                        (0..n * n)
                            .map(|i| if i / n == i % n { m.data[i / n] } else { 0.0 })
                            .collect(),
                    )?)
                } else {
                    Value::Real(Matrix::vector(
                        (0..m.rows.min(m.cols))
                            .map(|i| m.data[i * m.cols + i])
                            .collect(),
                    ))
                }
            }
            "reshape" => {
                let m = arg(0)?.real()?;
                let r = count(arg(1)?.scalar()?)?;
                let c = count(arg(2)?.scalar()?)?;
                if r.checked_mul(c) != Some(m.data.len()) {
                    return Err(ScriptError::Eval(
                        "reshape must preserve element count".into(),
                    ));
                }
                let mut data = vec![0.0; m.data.len()];
                for i in 0..m.data.len() {
                    let old_row = i % m.rows;
                    let old_col = i / m.rows;
                    data[(i % r) * c + i / r] = m.data[old_row * m.cols + old_col];
                }
                Value::Real(Matrix::new(r, c, data)?)
            }
            "polyfit" => Value::Real(Matrix::vector(nexus_core::polyfit(
                &arg(0)?.real()?.data,
                &arg(1)?.real()?.data,
                count(arg(2)?.scalar()?)?,
            )?)),
            "polyval" => {
                let coefficients = arg(0)?.real()?;
                let x = arg(1)?.real()?;
                if coefficients.data.len().saturating_mul(x.data.len()) > 50_000_000 {
                    return Err(ScriptError::Eval("polyval work limit reached".into()));
                }
                Value::Real(Matrix::new(
                    x.rows,
                    x.cols,
                    nexus_core::polyval(&coefficients.data, &x.data),
                )?)
            }
            "inv" => {
                let m = arg(0)?.real()?;
                if m.rows > 256 {
                    return Err(ScriptError::Eval("inverse dimension limit is 256".into()));
                }
                Value::Real(m.inverse()?)
            }
            "solve" => {
                let m = arg(0)?.real()?;
                if m.rows > 256 {
                    return Err(ScriptError::Eval("solve dimension limit is 256".into()));
                }
                Value::Real(m.solve(arg(1)?.real()?)?)
            }
            "size" => {
                let (r, c) = arg(0)?.shape();
                if args.len() == 2 {
                    let dim = count(arg(1)?.scalar()?)?;
                    if dim == 0 {
                        return Err(ScriptError::Eval("size dimension begins at 1".into()));
                    }
                    return Ok(Value::Real(Matrix::scalar(match dim {
                        1 => r,
                        2 => c,
                        _ => 1,
                    } as f64)));
                }
                Value::Real(Matrix::vector(vec![r as f64, c as f64]))
            }
            "length" => Value::Real(Matrix::scalar(
                arg(0)?.shape().0.max(arg(0)?.shape().1) as f64
            )),
            "sum" | "mean" => {
                let v = &arg(0)?.real()?.data;
                let sum: f64 = v.iter().sum();
                Value::Real(Matrix::scalar(if name == "mean" {
                    sum / v.len().max(1) as f64
                } else {
                    sum
                }))
            }
            "min" | "max" | "std" | "norm" => {
                let data = &arg(0)?.real()?.data;
                if data.is_empty() {
                    return Err(ScriptError::Eval(format!("{name} requires nonempty data")));
                }
                let value = match name {
                    "min" => data.iter().copied().fold(f64::INFINITY, f64::min),
                    "max" => data.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                    "norm" => data.iter().map(|x| x * x).sum::<f64>().sqrt(),
                    _ => {
                        let mean = data.iter().sum::<f64>() / data.len() as f64;
                        (data.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
                            / data.len().saturating_sub(1).max(1) as f64)
                            .sqrt()
                    }
                };
                Value::Real(Matrix::scalar(value))
            }
            "diff" => Value::Real(Matrix::vector(
                arg(0)?
                    .real()?
                    .data
                    .windows(2)
                    .map(|p| p[1] - p[0])
                    .collect(),
            )),
            "cumsum" => {
                let m = arg(0)?.real()?;
                let mut sum = 0.0;
                Value::Real(m.map(|x| {
                    sum += x;
                    sum
                }))
            }
            "sort" => {
                let mut m = arg(0)?.real()?.clone();
                m.data.sort_by(f64::total_cmp);
                Value::Real(m)
            }
            "linspace" => {
                let (a, b, n) = (
                    arg(0)?.scalar()?,
                    arg(1)?.scalar()?,
                    count(arg(2)?.scalar()?)?,
                );
                Value::Real(Matrix::vector(
                    (0..n)
                        .map(|i| {
                            if n == 1 {
                                b
                            } else {
                                a + (b - a) * i as f64 / (n - 1) as f64
                            }
                        })
                        .collect(),
                ))
            }
            "zeros" | "ones" | "randn" => {
                let size = arg(0)?.real()?;
                let (r, c) = if args.len() == 2 {
                    (count(arg(0)?.scalar()?)?, count(arg(1)?.scalar()?)?)
                } else if size.data.len() == 2 {
                    (count(size.data[0])?, count(size.data[1])?)
                } else {
                    let n = count(arg(0)?.scalar()?)?;
                    (n, n)
                };
                if r.saturating_mul(c) > MAX_ELEMENTS {
                    return Err(ScriptError::Eval("array size limit reached".into()));
                }
                Value::Real(Matrix::new(
                    r,
                    c,
                    (0..r * c)
                        .map(|_| match name {
                            "ones" => 1.0,
                            "randn" => self.rng.gaussian(),
                            _ => 0.0,
                        })
                        .collect(),
                )?)
            }
            "hann" | "hamming" | "blackmanharris" => Value::Real(Matrix::vector(window(
                match name {
                    "hann" => Window::Hann,
                    "hamming" => Window::Hamming,
                    _ => Window::BlackmanHarris,
                },
                count(arg(0)?.scalar()?)?,
            ))),
            "fir" | "filter" => {
                let x = &arg(0)?.real()?.data;
                let h = &arg(1)?.real()?.data;
                if x.len().saturating_mul(h.len()) > 50_000_000 {
                    return Err(ScriptError::Eval("FIR work limit reached".into()));
                }
                Value::Real(Matrix::vector(nexus_core::filter_fir(x, h)))
            }
            "figure" => {
                let id = if args.is_empty() {
                    self.figures.keys().max().copied().unwrap_or(0) + 1
                } else {
                    count(arg(0)?.scalar()?)? as u32
                };
                if id == 0 || id > 16 {
                    return Err(ScriptError::Eval("figure id must be in 1..16".into()));
                }
                self.current_figure = id;
                self.figures.entry(id).or_insert_with(|| Figure::new(id));
                self.hold = false;
                Value::Text(format!("Figure {id}"))
            }
            "title" | "xlabel" | "ylabel" | "hold" | "grid" => {
                let Value::Text(text) = arg(0)? else {
                    return Err(ScriptError::Eval(format!(
                        "{name} requires a quoted string"
                    )));
                };
                if name == "hold" {
                    if !matches!(text.as_str(), "on" | "off") {
                        return Err(ScriptError::Eval("hold expects on or off".into()));
                    }
                    self.hold = text == "on";
                } else {
                    let figure = self
                        .figures
                        .entry(self.current_figure)
                        .or_insert_with(|| Figure::new(self.current_figure));
                    match name {
                        "title" => figure.title = text.clone(),
                        "xlabel" => figure.x_label = text.clone(),
                        "ylabel" => figure.y_label = text.clone(),
                        _ => figure.grid = text == "on",
                    }
                }
                Value::Text(String::new())
            }
            "plot" | "scatter" | "bar" => {
                let (x, y) = if args.len() == 1 {
                    let y = arg(0)?.real()?.data.clone();
                    ((1..=y.len()).map(|i| i as f64).collect(), y)
                } else {
                    (arg(0)?.real()?.data.clone(), arg(1)?.real()?.data.clone())
                };
                if x.len() != y.len() {
                    return Err(ScriptError::Eval(
                        "plot vectors must have equal length".into(),
                    ));
                }
                if x.is_empty() || !x.iter().chain(&y).all(|v| v.is_finite()) {
                    return Err(ScriptError::Eval(
                        "plot needs nonempty finite vectors".into(),
                    ));
                }
                let existing: usize = self
                    .figures
                    .iter()
                    .filter(|(id, _)| self.hold || **id != self.current_figure)
                    .flat_map(|(_, f)| &f.series)
                    .map(|s| s.x.len())
                    .sum();
                if existing.saturating_add(x.len()) > 4_000_000 {
                    return Err(ScriptError::Eval("figure data exceeds 64 MiB limit".into()));
                }
                let figure = self
                    .figures
                    .entry(self.current_figure)
                    .or_insert_with(|| Figure::new(self.current_figure));
                if !self.hold {
                    figure.series.clear();
                }
                if figure.series.len() >= 32 {
                    return Err(ScriptError::Eval("figure supports up to 32 series".into()));
                }
                figure.series.push(PlotSeries {
                    name: format!("Series {}", figure.series.len() + 1),
                    x: x.clone(),
                    y: y.clone(),
                    kind: match name {
                        "scatter" => PlotKind::Scatter,
                        "bar" => PlotKind::Bar,
                        _ => PlotKind::Line,
                    },
                });
                self.plot = Some((x, y));
                Value::Text(format!("Figure {} updated", self.current_figure))
            }
            "fir_lowpass" | "fir_highpass" => {
                let taps = count(arg(0)?.scalar()?)?;
                let cutoff = arg(1)?.scalar()?;
                let rate = arg(2)?.scalar()?;
                Value::Real(Matrix::vector(if name == "fir_lowpass" {
                    nexus_core::fir_lowpass(taps, cutoff, rate)?
                } else {
                    nexus_core::fir_highpass(taps, cutoff, rate)?
                }))
            }
            "fir_bandpass" => Value::Real(Matrix::vector(nexus_core::fir_bandpass(
                count(arg(0)?.scalar()?)?,
                arg(1)?.scalar()?,
                arg(2)?.scalar()?,
                arg(3)?.scalar()?,
            )?)),
            "iir_lowpass" | "iir_highpass" | "iir_bandpass" => {
                let kind = match name {
                    "iir_lowpass" => nexus_core::FilterKind::Lowpass,
                    "iir_highpass" => nexus_core::FilterKind::Highpass,
                    _ => nexus_core::FilterKind::Bandpass,
                };
                let mut filter = nexus_core::Biquad::new(
                    kind,
                    arg(1)?.scalar()?,
                    arg(2)?.scalar()?,
                    arg(3)?.scalar()?,
                )?;
                Value::Real(arg(0)?.real()?.map(|x| filter.process(x)))
            }
            "serial_ports" => Value::Text(
                nexus_hardware::ports()
                    .map_err(|e| ScriptError::Eval(e.to_string()))?
                    .iter()
                    .map(|p| p.name.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
            "usb_devices" => Value::Text(format!(
                "{:?}",
                nexus_hardware::usb_devices().map_err(|e| ScriptError::Eval(e.to_string()))?
            )),
            _ => return Err(ScriptError::Eval(format!("unknown function '{name}'"))),
        })
    }
}
fn from_dynamic(v: rhai::Dynamic) -> Option<Value> {
    if v.is::<f64>() {
        return Some(Value::Real(Matrix::scalar(v.cast())));
    }
    if v.is::<rhai::INT>() {
        return Some(Value::Real(Matrix::scalar(v.cast::<rhai::INT>() as f64)));
    }
    if v.is::<rhai::Array>() {
        let array = v.cast::<rhai::Array>();
        if !array.is_empty() && array.iter().all(|v| v.is::<rhai::Array>()) {
            let pairs = array
                .into_iter()
                .map(|v| {
                    let pair = v.cast::<rhai::Array>();
                    if pair.len() != 2 {
                        return None;
                    }
                    let number = |v: rhai::Dynamic| {
                        if v.is::<f64>() {
                            Some(v.cast::<f64>())
                        } else {
                            v.try_cast::<rhai::INT>().map(|v| v as f64)
                        }
                    };
                    Some([number(pair[0].clone())?, number(pair[1].clone())?])
                })
                .collect::<Option<Vec<_>>>()?;
            return Some(Value::Complex(pairs));
        }
        let data = array
            .into_iter()
            .map(|v| {
                if v.is::<f64>() {
                    Some(v.cast::<f64>())
                } else {
                    v.try_cast::<rhai::INT>().map(|x| x as f64)
                }
            })
            .collect::<Option<Vec<_>>>()?;
        return Some(Value::Real(Matrix::vector(data)));
    }
    if v.is::<rhai::ImmutableString>() {
        return Some(Value::Text(v.cast::<rhai::ImmutableString>().to_string()));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn octave_style_algebra_and_constructor_semantics() {
        let mut s = Session::new();
        s.evaluate("A=[1 -2;0 1]; square=A^2; elementwise=A.^2; inverse=A^-1; identity=A/A; Z=zeros(3); O=ones(2,3); R=randn([2 3]); E=eye(2,3); rows=size(E,1); columns=size(E,2); values=eig(A); P=pinv([1 2;2 4],0.01)").unwrap();
        assert_eq!(
            s.variables["square"].real().unwrap().data,
            vec![1., -4., 0., 1.]
        );
        assert_eq!(
            s.variables["elementwise"].real().unwrap().data,
            vec![1., 4., 0., 1.]
        );
        assert_eq!(s.variables["Z"].shape(), (3, 3));
        assert_eq!(s.variables["O"].shape(), (2, 3));
        assert_eq!(s.variables["R"].shape(), (2, 3));
        assert_eq!(
            s.variables["E"].real().unwrap().data,
            vec![1., 0., 0., 0., 1., 0.]
        );
        assert_eq!(s.variables["values"].shape(), (2, 1));
        s.evaluate(
            "rotation=[0 -1;1 0]; lambda=eig(rotation); conjugated=lambda'; magnitudes=abs(lambda)",
        )
        .unwrap();
        assert_eq!(s.variables["lambda"].shape(), (2, 1));
        assert_eq!(s.variables["conjugated"].shape(), (1, 2));
        assert_eq!(s.variables["magnitudes"].real().unwrap().data, vec![1., 1.]);
        assert!(s.evaluate("pinv(A,-1)").is_err());
        assert!(s.evaluate("A^0.5").is_err());
        assert!(s
            .evaluate("polyfit([1e200 2e200 3e200],[1 2 3],2)")
            .is_err());
        assert_eq!(s.evaluate("[1 - 2]").unwrap().value.scalar().unwrap(), -1.);
        assert_eq!(
            s.evaluate("[(1 -2) 3]").unwrap().value.real().unwrap().data,
            vec![-1., 3.]
        );
    }
    #[test]
    fn advanced_dsp_bindings_validate_and_share_results() {
        let mut session = Session::new();
        session.evaluate("samples=sin(2*pi*64*(0:0.00048828125:1)); coeff=butter_lowpass(4,200,2048); filtered=sosfilt(samples,coeff); density=psd(samples,2048,256); response=freqz(coeff,2048); summary=stats(samples); med=median([1,2,3,4]); rms_value=rms([3,4]);").unwrap();
        assert_eq!(session.variables["coeff"].shape(), (2, 6));
        assert_eq!(session.variables["density"].shape(), (129, 2));
        assert_eq!(session.variables["response"].shape(), (513, 2));
        assert_eq!(session.variables["med"].scalar().unwrap(), 2.5);
        assert!(session.variables["summary"].display().contains("Median"));
        assert!(session.evaluate("psd(samples,0,256)").is_err());
        assert!(session.evaluate("butter_lowpass(4,1200,2048)").is_err());
    }
    #[test]
    fn professional_commands_and_figures_share_workspace() {
        let mut s = Session::new();
        s.evaluate("clear").unwrap();
        assert_eq!(s.variables.len(), 1);
        let r = s.evaluate("a = [1,2;3,4];\nb = a(2,1)\n").unwrap();
        assert_eq!(s.variables["b"].scalar().unwrap(), 3.0);
        assert!(!r.output.contains("a ="));
        assert!(r.output.contains("b ="));
        assert_eq!(s.evaluate("a(2)").unwrap().value.scalar().unwrap(), 3.0);
        s.evaluate("x = 0:0.5:2; y = 2*x+1; p = polyfit(x,y,1); fit = polyval(p,x); plot(x,fit); title('Regression'); xlabel('Time'); ylabel('Value')").unwrap();
        assert_eq!(s.figures[&1].title, "Regression");
        s.evaluate("hold on").unwrap();
        s.evaluate("scatter(x,y)").unwrap();
        assert_eq!(s.figures[&1].series.len(), 2);
        assert!(s.evaluate("a(0)").is_err());
        assert!(s.evaluate("whos").unwrap().output.contains("fit"));
        s.evaluate("clear fit").unwrap();
        assert!(!s.variables.contains_key("fit"));
        s.evaluate("close all").unwrap();
        assert!(s.figures.is_empty());
    }
    #[test]
    fn matrix_helpers_follow_column_major_reshape() {
        let mut s = Session::new();
        s.evaluate("A = reshape(1:6,2,3); d = det([4,1;2,3]); E = eye(2)")
            .unwrap();
        assert_eq!(
            s.variables["A"].real().unwrap().data,
            vec![1.0, 3.0, 5.0, 2.0, 4.0, 6.0]
        );
        assert_eq!(s.variables["d"].scalar().unwrap(), 10.0);
        assert_eq!(
            s.variables["E"].real().unwrap().data,
            vec![1.0, 0.0, 0.0, 1.0]
        );
    }
    #[test]
    fn matlab_matrix_and_vectors() {
        let mut s = Session::new();
        s.evaluate("A = [1, 2; 3, 4]; B = A * A; t = 0:0.1:1; sig = sin(2*pi*t) + randn(size(t)); f = fft(sig)").unwrap();
        assert_eq!(
            s.variables["B"].real().unwrap().data,
            vec![7.0, 10.0, 15.0, 22.0]
        );
        assert_eq!(s.variables["t"].shape(), (1, 11));
        assert_eq!(s.variables["f"].shape(), (1, 11));
    }
    #[test]
    fn errors_and_precedence() {
        let mut s = Session::new();
        assert_eq!(
            s.evaluate("-2^2 + 3*4").unwrap().value.scalar().unwrap(),
            8.0
        );
        assert!(s.evaluate("0:0:10").is_err());
        assert!(s.evaluate("[1,2;3]").is_err());
        assert!(s.evaluate("sin(1,2)").is_err());
        assert!(s.evaluate("x = 0:1000001").is_err());
        assert!(s.evaluate("missing + 1").is_err());
    }
    #[test]
    fn rhai_is_bounded_and_shared() {
        let mut s = Session::new();
        s.evaluate("rhai: let result = 6 * 7; result").unwrap();
        assert_eq!(s.variables["result"].scalar().unwrap(), 42.0);
        assert!(s.evaluate("rhai: loop {}").is_err());
    }
    #[test]
    fn rhai_output_and_complex_workspace() {
        let mut s = Session::new();
        let result = s
            .evaluate("rhai: print(\"hello\"); let spectrum = fft([1,2,3,4]); spectrum")
            .unwrap();
        assert!(result.output.starts_with("hello\n"));
        assert!(matches!(s.variables["spectrum"], Value::Complex(_)));
        assert_eq!(s.variables["spectrum"].shape(), (1, 4));
        let deep = std::iter::repeat_n("1", 300).collect::<Vec<_>>().join("+");
        assert!(s.evaluate(&deep).is_err());
    }
}
