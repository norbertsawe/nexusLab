//! Checked statistics, Welch PSD, and stable Butterworth second-order sections.
use crate::{window, Complex, FilterKind, MathError, Matrix, Window};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Statistics {
    pub count: usize,
    pub mean: f64,
    pub median: f64,
    pub std_dev: f64,
    pub rms: f64,
    pub peak_to_peak: f64,
    pub minimum: f64,
    pub maximum: f64,
}
pub fn statistics(input: &[f64]) -> Result<Statistics, MathError> {
    if input.is_empty() || input.len() > 1_000_000 || !input.iter().all(|v| v.is_finite()) {
        return Err(MathError::Parameter(
            "statistics needs 1..=1000000 finite samples".into(),
        ));
    }
    let scale = input.iter().fold(0.0_f64, |a, x| a.max(x.abs()));
    let mut sorted = input.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = input.len();
    let mean = if scale == 0.0 {
        0.0
    } else {
        input.iter().map(|v| v / scale).sum::<f64>() / n as f64 * scale
    };
    let median = if n.is_multiple_of(2) {
        sorted[n / 2 - 1] / 2.0 + sorted[n / 2] / 2.0
    } else {
        sorted[n / 2]
    };
    let rms = if scale == 0.0 {
        0.0
    } else {
        (input.iter().map(|v| (v / scale).powi(2)).sum::<f64>() / n as f64).sqrt() * scale
    };
    let std_dev = if n <= 1 || scale == 0.0 {
        0.0
    } else {
        (input
            .iter()
            .map(|v| (v / scale - mean / scale).powi(2))
            .sum::<f64>()
            / (n - 1) as f64)
            .sqrt()
            * scale
    };
    let result = Statistics {
        count: n,
        mean,
        median,
        std_dev,
        rms,
        peak_to_peak: sorted[n - 1] - sorted[0],
        minimum: sorted[0],
        maximum: sorted[n - 1],
    };
    if ![mean, median, std_dev, rms, result.peak_to_peak]
        .iter()
        .all(|x| x.is_finite())
    {
        return Err(MathError::Parameter("statistics overflow".into()));
    }
    Ok(result)
}

/// One-sided Welch density (units squared/Hz), Hann window, 50% overlap,
/// per-segment mean removal. Integrating bins approximates signal variance.
pub fn welch_psd(input: &[f64], sample_rate: f64, segment: usize) -> Result<Matrix, MathError> {
    if input.len() < 4
        || input.len() > 1_000_000
        || !(4..=65536).contains(&segment)
        || segment > input.len()
        || !sample_rate.is_finite()
        || sample_rate <= 0.0
        || !input.iter().all(|x| x.is_finite())
    {
        return Err(MathError::Parameter(
            "PSD needs finite samples, positive sample rate and segment length 4..=min(65536,N)"
                .into(),
        ));
    }
    let weights = window(Window::Hann, segment);
    let norm = sample_rate * weights.iter().map(|w| w * w).sum::<f64>();
    if !norm.is_finite() || norm <= 0.0 {
        return Err(MathError::Parameter("PSD normalization overflow".into()));
    }
    let bins = segment / 2 + 1;
    let mut power = vec![0.0; bins];
    let mut count = 0;
    let plan = crate::rustfft_export::FftPlanner::new().plan_fft_forward(segment);
    let mut spectrum = vec![Complex::default(); segment];
    let mut scratch = vec![Complex::default(); plan.get_inplace_scratch_len()];
    for start in (0..=input.len() - segment).step_by(segment / 2) {
        let data = &input[start..start + segment];
        let mean = data.iter().sum::<f64>() / segment as f64;
        for ((value, weight), sample) in data.iter().zip(&weights).zip(&mut spectrum) {
            *sample = Complex::new((value - mean) * weight, 0.0);
        }
        plan.process_with_scratch(&mut spectrum, &mut scratch);
        for (k, p) in power.iter_mut().enumerate() {
            let factor = if k == 0 || (segment.is_multiple_of(2) && k == segment / 2) {
                1.0
            } else {
                2.0
            };
            *p += factor * spectrum[k].norm_sqr() / norm;
        }
        count += 1;
    }
    let data: Vec<_> = power
        .into_iter()
        .enumerate()
        .flat_map(|(k, p)| [k as f64 * sample_rate / segment as f64, p / count as f64])
        .collect();
    if !data.iter().all(|v| v.is_finite()) {
        return Err(MathError::Parameter("PSD overflow".into()));
    }
    Matrix::new(bins, 2, data)
}

/// Prewarped analog Butterworth prototype, bilinear transform, real SOS rows
/// [b0,b1,b2,1,a1,a2]. Bandpass has digital order 2*order.
pub fn butterworth(
    order: usize,
    kind: FilterKind,
    low: f64,
    high: f64,
    fs: f64,
) -> Result<Matrix, MathError> {
    if !(1..=12).contains(&order)
        || !fs.is_finite()
        || fs <= 0.0
        || !low.is_finite()
        || low <= 0.0
        || low >= fs / 2.0
        || (matches!(kind, FilterKind::Bandpass)
            && (!high.is_finite() || high <= low || high >= fs / 2.0))
    {
        return Err(MathError::Parameter(
            "Butterworth order 1..=12, positive sample rate, 0 < cutoff(s) < Nyquist required"
                .into(),
        ));
    }
    let warp = |f: f64| 2.0 * fs * (PI * f / fs).tan();
    let w = warp(low);
    let wh = if matches!(kind, FilterKind::Bandpass) {
        warp(high)
    } else {
        w
    };
    let mut poles = Vec::new();
    for k in 0..order {
        let angle = PI * (2 * k + order + 1) as f64 / (2 * order) as f64;
        let p = Complex::from_polar(1.0, angle);
        let analog = match kind {
            FilterKind::Lowpass => vec![p * w],
            FilterKind::Highpass => vec![Complex::new(w, 0.0) / p],
            FilterKind::Bandpass => {
                let b = p * (wh - w);
                let d = (b * b - Complex::new(4.0 * w * wh, 0.0)).sqrt();
                vec![(b + d) / 2.0, (b - d) / 2.0]
            }
        };
        for p in analog {
            poles.push((Complex::new(2.0 * fs, 0.0) + p) / (Complex::new(2.0 * fs, 0.0) - p));
        }
    }
    if poles
        .iter()
        .any(|p| !p.re.is_finite() || !p.im.is_finite() || p.norm() >= 1.0)
    {
        return Err(MathError::Parameter(
            "filter poles are numerically unstable; choose less extreme cutoffs".into(),
        ));
    }
    let mut rows = Vec::new();
    while let Some(p) = poles.pop() {
        let partner = if p.im.abs() > 1e-8 {
            poles.iter().position(|q| (*q - p.conj()).norm() < 1e-7)
        } else {
            poles.iter().position(|q| q.im.abs() <= 1e-8)
        };
        let q = partner.map(|i| poles.remove(i));
        if p.im.abs() > 1e-8 && q.is_none() {
            return Err(MathError::Parameter("cannot pair filter poles".into()));
        }
        let (a1, a2) = q.map_or((-p.re, 0.0), |q| (-(p + q).re, (p * q).re));
        let b = match kind {
            FilterKind::Lowpass => {
                if q.is_some() {
                    [1.0, 2.0, 1.0]
                } else {
                    [1.0, 1.0, 0.0]
                }
            }
            FilterKind::Highpass => {
                if q.is_some() {
                    [1.0, -2.0, 1.0]
                } else {
                    [1.0, -1.0, 0.0]
                }
            }
            FilterKind::Bandpass => [1.0, 0.0, -1.0],
        };
        rows.extend([b[0], b[1], b[2], 1.0, a1, a2]);
    }
    let mut sos = Matrix::new(rows.len() / 6, 6, rows)?;
    let frequency = match kind {
        FilterKind::Lowpass => 0.0,
        FilterKind::Highpass => fs / 2.0,
        FilterKind::Bandpass => fs / PI * ((w * wh).sqrt() / (2.0 * fs)).atan(),
    };
    let gain = sos_response(&sos, frequency, fs)?.norm();
    if !gain.is_finite() || gain <= 0.0 {
        return Err(MathError::Parameter("filter normalization failed".into()));
    }
    for b in &mut sos.data[..3] {
        *b /= gain;
    }
    Ok(sos)
}
pub fn sos_response(sos: &Matrix, frequency: f64, fs: f64) -> Result<Complex, MathError> {
    if sos.cols != 6
        || sos.rows == 0
        || sos.rows.checked_mul(6) != Some(sos.data.len())
        || !fs.is_finite()
        || fs <= 0.0
        || !frequency.is_finite()
        || !sos.data.iter().all(|x| x.is_finite())
    {
        return Err(MathError::Parameter(
            "valid SOS rows and positive sample rate required".into(),
        ));
    }
    let z = Complex::from_polar(1.0, -2.0 * PI * frequency / fs);
    let mut result = Complex::new(1.0, 0.0);
    for s in sos.data.as_chunks::<6>().0.iter() {
        result *= (s[0] + s[1] * z + s[2] * z * z) / (s[3] + s[4] * z + s[5] * z * z);
    }
    if !result.re.is_finite() || !result.im.is_finite() {
        return Err(MathError::Parameter(
            "singular or overflowing SOS response".into(),
        ));
    }
    Ok(result)
}
pub fn filter_sos(input: &[f64], sos: &Matrix) -> Result<Vec<f64>, MathError> {
    sos_response(sos, 0.0, 1.0)?;
    if input.len() > 1_000_000
        || !input.iter().all(|x| x.is_finite())
        || sos.rows > 32
        || sos.data.as_chunks::<6>().0.iter().any(|s| s[3] == 0.0)
    {
        return Err(MathError::Parameter(
            "invalid signal or SOS coefficients".into(),
        ));
    }
    let mut out = input.to_vec();
    for s in sos.data.as_chunks::<6>().0.iter() {
        let mut z = [0.0; 2];
        for x in &mut out {
            let y = s[0] / s[3] * *x + z[0];
            z[0] = s[1] / s[3] * *x - s[4] / s[3] * y + z[1];
            z[1] = s[2] / s[3] * *x - s[5] / s[3] * y;
            *x = y;
        }
    }
    if !out.iter().all(|x| x.is_finite()) {
        return Err(MathError::Parameter("filter output overflow".into()));
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn statistics_and_psd_conserve_power() {
        let s = statistics(&[1.0, 2.0, 3.0, 4.0]).unwrap();
        assert_eq!(s.median, 2.5);
        assert_eq!(s.peak_to_peak, 3.0);
        assert!((s.std_dev - (5.0_f64 / 3.0).sqrt()).abs() < 1e-12);
        let x: Vec<_> = (0..8192)
            .map(|i| (2.0 * PI * 128.0 * i as f64 / 2048.0).sin())
            .collect();
        let p = welch_psd(&x, 2048.0, 1024).unwrap();
        let integral: f64 = p.data.as_chunks::<2>().0.iter().map(|r| r[1] * 2.0).sum();
        assert!((integral - 0.5).abs() < 1e-4);
        assert!(welch_psd(&x, 0.0, 1024).is_err());
    }
    #[test]
    fn butterworth_cutoffs_and_stability() {
        for n in 1..=8 {
            for kind in [
                FilterKind::Lowpass,
                FilterKind::Highpass,
                FilterKind::Bandpass,
            ] {
                let sos = butterworth(n, kind, 1000.0, 3000.0, 16000.0).unwrap();
                assert!(
                    (sos_response(&sos, 1000.0, 16000.0).unwrap().norm() - 2.0_f64.sqrt().recip())
                        .abs()
                        < 1e-7
                );
                if matches!(kind, FilterKind::Bandpass) {
                    assert!(
                        (sos_response(&sos, 3000.0, 16000.0).unwrap().norm()
                            - 2.0_f64.sqrt().recip())
                        .abs()
                            < 1e-7
                    );
                }
                let y = filter_sos(&vec![1.0; 10000], &sos).unwrap();
                assert!(y.iter().all(|v| v.abs() < 3.0));
            }
        }
        assert!(butterworth(0, FilterKind::Lowpass, 1000.0, 0.0, 16000.0).is_err());
    }
}
