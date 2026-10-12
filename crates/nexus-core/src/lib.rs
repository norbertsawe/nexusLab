//! Numerical primitives shared by every frontend. No UI or device dependencies.
mod advanced;
pub use advanced::{butterworth, filter_sos, sos_response, statistics, welch_psd, Statistics};
use nalgebra::DMatrix;
use ndarray::{ArrayD, IxDyn};
use rustfft::{num_complex::Complex64, FftPlanner};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
use thiserror::Error;

pub use ringbuf::{HeapCons, HeapProd, HeapRb};
pub use rustfft as rustfft_export;
pub use rustfft::num_complex::Complex64 as Complex;

#[derive(Debug, Error)]
pub enum MathError {
    #[error("incompatible dimensions: {0}")]
    Shape(String),
    #[error("matrix is singular")]
    Singular,
    #[error("invalid numerical parameter: {0}")]
    Parameter(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Matrix {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<f64>,
}

impl Matrix {
    pub fn new(rows: usize, cols: usize, data: Vec<f64>) -> Result<Self, MathError> {
        if rows.checked_mul(cols) != Some(data.len()) {
            return Err(MathError::Shape(
                "rows × cols must equal data length".into(),
            ));
        }
        Ok(Self { rows, cols, data })
    }
    pub fn vector(data: Vec<f64>) -> Self {
        Self {
            rows: 1,
            cols: data.len(),
            data,
        }
    }
    pub fn scalar(x: f64) -> Self {
        Self::vector(vec![x])
    }
    pub fn map(&self, f: impl FnMut(f64) -> f64) -> Self {
        Self {
            rows: self.rows,
            cols: self.cols,
            data: self.data.iter().copied().map(f).collect(),
        }
    }
    pub fn elementwise(&self, rhs: &Self, f: impl Fn(f64, f64) -> f64) -> Result<Self, MathError> {
        if self.data.len() == 1 {
            return Ok(rhs.map(|x| f(self.data[0], x)));
        }
        if rhs.data.len() == 1 {
            return Ok(self.map(|x| f(x, rhs.data[0])));
        }
        if self.rows != rhs.rows || self.cols != rhs.cols {
            return Err(MathError::Shape(
                "element-wise operands must have the same shape".into(),
            ));
        }
        Self::new(
            self.rows,
            self.cols,
            self.data
                .iter()
                .zip(&rhs.data)
                .map(|(&a, &b)| f(a, b))
                .collect(),
        )
    }
    fn native(&self) -> DMatrix<f64> {
        DMatrix::from_row_slice(self.rows, self.cols, &self.data)
    }
    fn from_native(m: DMatrix<f64>) -> Self {
        let data = (0..m.nrows())
            .flat_map(|r| (0..m.ncols()).map(move |c| (r, c)))
            .map(|(r, c)| m[(r, c)])
            .collect();
        Self {
            rows: m.nrows(),
            cols: m.ncols(),
            data,
        }
    }
    pub fn transpose(&self) -> Self {
        Self::from_native(self.native().transpose())
    }
    pub fn matmul(&self, rhs: &Self) -> Result<Self, MathError> {
        if self.data.len() == 1 || rhs.data.len() == 1 {
            return self.elementwise(rhs, |a, b| a * b);
        }
        if self.cols != rhs.rows {
            return Err(MathError::Shape("matrix inner dimensions differ".into()));
        }
        Ok(Self::from_native(self.native() * rhs.native()))
    }
    pub fn inverse(&self) -> Result<Self, MathError> {
        if self.rows != self.cols || self.rows == 0 {
            return Err(MathError::Shape(
                "inverse requires a nonempty square matrix".into(),
            ));
        }
        self.native()
            .try_inverse()
            .map(Self::from_native)
            .ok_or(MathError::Singular)
    }
    pub fn determinant(&self) -> Result<f64, MathError> {
        if self.rows != self.cols || self.rows == 0 {
            return Err(MathError::Shape(
                "determinant requires a nonempty square matrix".into(),
            ));
        }
        Ok(self.native().determinant())
    }
    pub fn solve(&self, rhs: &Self) -> Result<Self, MathError> {
        if self.rows != self.cols || self.rows != rhs.rows || self.rows == 0 {
            return Err(MathError::Shape(
                "solve requires square A and matching B rows".into(),
            ));
        }
        self.native()
            .lu()
            .solve(&rhs.native())
            .map(Self::from_native)
            .ok_or(MathError::Singular)
    }
}

/// Least-squares polynomial fit using SVD; coefficients are in descending powers.
pub fn polyfit(x: &[f64], y: &[f64], degree: usize) -> Result<Vec<f64>, MathError> {
    if x.len() != y.len()
        || x.len() <= degree
        || degree > 16
        || x.len().saturating_mul((degree + 1).pow(2)) > 50_000_000
        || !x.iter().chain(y).all(|v| v.is_finite())
    {
        return Err(MathError::Parameter(
            "polyfit needs matching finite vectors, more samples than degree, and degree <= 16"
                .into(),
        ));
    }
    let design = DMatrix::from_fn(x.len(), degree + 1, |r, c| x[r].powi((degree - c) as i32));
    let rhs = DMatrix::from_column_slice(y.len(), 1, y);
    let result = design
        .svd(true, true)
        .solve(&rhs, 1e-12)
        .map_err(|e| MathError::Parameter(e.into()))?;
    Ok(result.column(0).iter().copied().collect())
}
pub fn polyval(coefficients: &[f64], x: &[f64]) -> Vec<f64> {
    x.iter()
        .map(|x| coefficients.iter().fold(0.0, |y, c| y * x + c))
        .collect()
}

/// Contiguous N-dimensional array storage; checked shape, view, and axis permutation.
#[derive(Clone, Debug)]
pub struct Tensor(pub ArrayD<f64>);
impl Tensor {
    pub fn new(shape: &[usize], data: Vec<f64>) -> Result<Self, MathError> {
        ArrayD::from_shape_vec(IxDyn(shape), data)
            .map(Self)
            .map_err(|e| MathError::Shape(e.to_string()))
    }
    pub fn permute(self, axes: &[usize]) -> Result<Self, MathError> {
        let mut sorted = axes.to_vec();
        sorted.sort_unstable();
        if sorted != (0..self.0.ndim()).collect::<Vec<_>>() {
            return Err(MathError::Shape("invalid axis permutation".into()));
        }
        Ok(Self(self.0.permuted_axes(IxDyn(axes))))
    }
}

pub fn fft(input: &[f64]) -> Vec<Complex64> {
    let mut out: Vec<_> = input.iter().map(|&x| Complex64::new(x, 0.0)).collect();
    if !out.is_empty() {
        FftPlanner::new()
            .plan_fft_forward(out.len())
            .process(&mut out);
    }
    out
}
pub fn ifft(input: &[Complex64]) -> Vec<Complex64> {
    let mut out = input.to_vec();
    if !out.is_empty() {
        FftPlanner::new()
            .plan_fft_inverse(out.len())
            .process(&mut out);
        let n = out.len() as f64;
        out.iter_mut().for_each(|x| *x /= n);
    }
    out
}

#[derive(Clone, Copy, Debug)]
pub enum Window {
    Hann,
    Hamming,
    BlackmanHarris,
}
pub fn window(kind: Window, n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| {
            if n <= 1 {
                return 1.0;
            }
            let p = 2.0 * PI * i as f64 / (n - 1) as f64;
            match kind {
                Window::Hann => 0.5 - 0.5 * p.cos(),
                Window::Hamming => 0.54 - 0.46 * p.cos(),
                Window::BlackmanHarris => {
                    0.35875 - 0.48829 * p.cos() + 0.14128 * (2.0 * p).cos()
                        - 0.01168 * (3.0 * p).cos()
                }
            }
        })
        .collect()
}
pub fn fir_lowpass(taps: usize, cutoff: f64, sample_rate: f64) -> Result<Vec<f64>, MathError> {
    if taps < 3
        || taps.is_multiple_of(2)
        || !cutoff.is_finite()
        || !sample_rate.is_finite()
        || cutoff <= 0.0
        || cutoff >= sample_rate / 2.0
    {
        return Err(MathError::Parameter(
            "odd taps >= 3 and 0 < cutoff < Nyquist required".into(),
        ));
    }
    let fc = cutoff / sample_rate;
    let w = window(Window::Hamming, taps);
    let mut h: Vec<_> = (0..taps)
        .map(|i| {
            let x = i as f64 - (taps - 1) as f64 / 2.0;
            (if x == 0.0 {
                2.0 * fc
            } else {
                (2.0 * PI * fc * x).sin() / (PI * x)
            }) * w[i]
        })
        .collect();
    let sum: f64 = h.iter().sum();
    h.iter_mut().for_each(|x| *x /= sum);
    Ok(h)
}
pub fn fir_highpass(taps: usize, cutoff: f64, sample_rate: f64) -> Result<Vec<f64>, MathError> {
    let mut h = fir_lowpass(taps, cutoff, sample_rate)?;
    h.iter_mut().for_each(|v| *v = -*v);
    h[taps / 2] += 1.0;
    Ok(h)
}
pub fn fir_bandpass(
    taps: usize,
    low: f64,
    high: f64,
    sample_rate: f64,
) -> Result<Vec<f64>, MathError> {
    if low >= high {
        return Err(MathError::Parameter("low must be below high".into()));
    }
    let a = fir_lowpass(taps, high, sample_rate)?;
    let b = fir_lowpass(taps, low, sample_rate)?;
    Ok(a.iter().zip(b).map(|(a, b)| a - b).collect())
}
pub fn filter_fir(input: &[f64], taps: &[f64]) -> Vec<f64> {
    (0..input.len())
        .map(|i| {
            taps.iter()
                .take(i + 1)
                .enumerate()
                .map(|(j, h)| input[i - j] * h)
                .sum()
        })
        .collect()
}

#[derive(Clone, Copy)]
pub enum FilterKind {
    Lowpass,
    Highpass,
    Bandpass,
}
pub struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    z: [f64; 2],
}
impl Biquad {
    pub fn new(
        kind: FilterKind,
        frequency: f64,
        q: f64,
        sample_rate: f64,
    ) -> Result<Self, MathError> {
        if !frequency.is_finite()
            || !sample_rate.is_finite()
            || !q.is_finite()
            || q <= 0.0
            || frequency <= 0.0
            || frequency >= sample_rate / 2.0
        {
            return Err(MathError::Parameter(
                "positive finite Q and frequency below Nyquist required".into(),
            ));
        }
        let w = 2.0 * PI * frequency / sample_rate;
        let c = w.cos();
        let alpha = w.sin() / (2.0 * q);
        let a0 = 1.0 + alpha;
        let b = match kind {
            FilterKind::Lowpass => [(1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0],
            FilterKind::Highpass => [(1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0],
            FilterKind::Bandpass => [alpha, 0.0, -alpha],
        };
        Ok(Self {
            b: b.map(|v| v / a0),
            a: [-2.0 * c / a0, (1.0 - alpha) / a0],
            z: [0.0; 2],
        })
    }
    pub fn process(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum Waveform {
    Sine,
    Square,
    Triangle,
    Saw,
    Noise,
}
pub struct Generator {
    pub waveform: Waveform,
    pub frequency: f64,
    pub amplitude: f64,
    pub sample_rate: f64,
    index: u64,
    seed: u64,
}
impl Generator {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            waveform: Waveform::Sine,
            frequency: 1000.0,
            amplitude: 2.5,
            sample_rate,
            index: 0,
            seed: 42,
        }
    }
    fn uniform(&mut self) -> f64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        ((self.seed >> 11) as f64 + 1.0) / ((1u64 << 53) as f64 + 1.0)
    }
    pub fn gaussian(&mut self) -> f64 {
        (-2.0 * self.uniform().ln()).sqrt() * (2.0 * PI * self.uniform()).cos()
    }
    pub fn sample(&mut self) -> [f64; 2] {
        let t = self.index as f64 / self.sample_rate;
        self.index += 1;
        let phase = (t * self.frequency).fract();
        let y = match self.waveform {
            Waveform::Sine => (2.0 * PI * phase).sin(),
            Waveform::Square => {
                if phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            Waveform::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
            Waveform::Saw => 2.0 * phase - 1.0,
            Waveform::Noise => self.gaussian(),
        };
        [t, self.amplitude * y]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn polynomial_regression_uses_all_samples() {
        let x = vec![-2.0, -1.0, 0.0, 1.0, 2.0];
        let y = polyval(&[2.0, -3.0, 5.0], &x);
        let c = polyfit(&x, &y, 2).unwrap();
        for (a, b) in c.iter().zip([2.0, -3.0, 5.0]) {
            assert!((a - b).abs() < 1e-10);
        }
        assert!(polyfit(&x, &y[..3], 2).is_err());
    }
    #[test]
    fn fft_roundtrip_and_peak() {
        let x: Vec<_> = (0..1024)
            .map(|i| (2.0 * PI * 32.0 * i as f64 / 1024.0).sin())
            .collect();
        let f = fft(&x);
        assert!((f[32].norm() - 512.0).abs() < 1e-8);
        for (a, b) in x.iter().zip(ifft(&f)) {
            assert!((a - b.re).abs() < 1e-10);
        }
    }
    #[test]
    fn matrices_solve() {
        let a = Matrix::new(2, 2, vec![1.0, 2.0, 3.0, 4.0]).unwrap();
        let eye = a.matmul(&a.inverse().unwrap()).unwrap();
        assert!((eye.data[0] - 1.0).abs() < 1e-12);
        let b = Matrix::new(2, 1, vec![5.0, 11.0]).unwrap();
        assert_eq!(a.solve(&b).unwrap().data, vec![1.0, 2.0]);
    }
    #[test]
    fn filters_dc_and_stability() {
        assert!(
            (fir_lowpass(31, 1000.0, 10000.0)
                .unwrap()
                .iter()
                .sum::<f64>()
                - 1.0)
                .abs()
                < 1e-12
        );
        assert!(
            fir_highpass(31, 1000.0, 10000.0)
                .unwrap()
                .iter()
                .sum::<f64>()
                .abs()
                < 1e-12
        );
        let mut b = Biquad::new(FilterKind::Lowpass, 1000.0, 0.707, 10000.0).unwrap();
        let mut y = 0.0;
        for _ in 0..1000 {
            y = b.process(1.0);
        }
        assert!((y - 1.0).abs() < 1e-10);
    }
}
