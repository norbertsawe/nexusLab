//! Bounded decompositions for interactive numerical work.
use super::{Complex, DMatrix, MathError, Matrix};
use nalgebra::linalg::{Schur, SVD};

impl Matrix {
    fn decomposition_scale(&self) -> Result<f64, MathError> {
        if self.rows == 0
            || self.cols == 0
            || self.rows > 256
            || self.cols > 256
            || self.rows.checked_mul(self.cols) != Some(self.data.len())
            || !self.data.iter().all(|x| x.is_finite())
        {
            return Err(MathError::Parameter(
                "decomposition requires a finite, nonempty matrix with dimensions <= 256".into(),
            ));
        }
        Ok(self.data.iter().fold(0.0_f64, |m, x| m.max(x.abs())))
    }

    /// Eigenvalues of a real square matrix, including complex conjugate pairs.
    pub fn eigenvalues(&self) -> Result<Vec<Complex>, MathError> {
        let scale = self.decomposition_scale()?;
        if self.rows != self.cols {
            return Err(MathError::Shape("eig requires a square matrix".into()));
        }
        if scale == 0.0 {
            return Ok(vec![Complex::new(0.0, 0.0); self.rows]);
        }
        let schur = Schur::try_new(self.native() / scale, f64::EPSILON, 100_000)
            .ok_or_else(|| MathError::Parameter("eigenvalue iteration limit reached".into()))?;
        let values: Vec<_> = schur
            .complex_eigenvalues()
            .iter()
            .map(|v| *v * scale)
            .collect();
        if !values.iter().all(|v| v.re.is_finite() && v.im.is_finite()) {
            return Err(MathError::Parameter(
                "eigenvalues exceed floating-point range".into(),
            ));
        }
        Ok(values)
    }

    /// Moore–Penrose pseudoinverse. `tolerance` is an absolute singular-value cutoff.
    pub fn pseudoinverse(&self, tolerance: Option<f64>) -> Result<Self, MathError> {
        let scale = self.decomposition_scale()?;
        if tolerance.is_some_and(|x| !x.is_finite() || x < 0.0) {
            return Err(MathError::Parameter(
                "pinv tolerance must be finite and nonnegative".into(),
            ));
        }
        if scale == 0.0 {
            return Self::new(self.cols, self.rows, vec![0.0; self.data.len()]);
        }
        let svd = SVD::try_new(self.native() / scale, true, true, f64::EPSILON, 100_000)
            .ok_or_else(|| MathError::Parameter("singular-value iteration limit reached".into()))?;
        let cutoff = tolerance.map(|t| t / scale).unwrap_or_else(|| {
            self.rows.max(self.cols) as f64 * f64::EPSILON * svd.singular_values[0]
        });
        // A tolerance larger than the scaled floating-point range discards every value.
        let result = if cutoff.is_infinite() {
            DMatrix::zeros(self.cols, self.rows)
        } else {
            svd.pseudo_inverse(cutoff)
                .map_err(|e| MathError::Parameter(e.into()))?
                / scale
        };
        if !result.iter().all(|x| x.is_finite()) {
            return Err(MathError::Parameter(
                "pseudoinverse exceeds floating-point range".into(),
            ));
        }
        Ok(Self::from_native(result))
    }

    /// Integer matrix power, with negative powers computed through the inverse.
    pub fn power(&self, exponent: f64) -> Result<Self, MathError> {
        if self.data.len() == 1 {
            return Ok(Self::scalar(self.data[0].powf(exponent)));
        }
        self.decomposition_scale()?;
        if self.rows != self.cols
            || !exponent.is_finite()
            || exponent.fract() != 0.0
            || exponent.abs() > 1_000_000.0
        {
            return Err(MathError::Parameter("matrix power requires a square matrix and an integer exponent with magnitude <= 1000000; use .^ for element-wise powers".into()));
        }
        let mut base = if exponent < 0.0 {
            self.inverse()?
        } else {
            self.clone()
        };
        let mut result = Self::from_native(DMatrix::identity(self.rows, self.cols));
        let mut n = exponent.abs() as u32;
        let multiplications = n.count_ones() + (32 - n.leading_zeros()).saturating_sub(1);
        if self
            .rows
            .saturating_pow(3)
            .saturating_mul(multiplications as usize)
            > 50_000_000
        {
            return Err(MathError::Parameter(
                "matrix power work limit reached".into(),
            ));
        }
        while n > 0 {
            if n & 1 == 1 {
                result = result.matmul(&base)?;
            }
            n >>= 1;
            if n > 0 {
                base = base.matmul(&base)?;
            }
        }
        if !result.data.iter().all(|x| x.is_finite()) {
            return Err(MathError::Parameter(
                "matrix power exceeds floating-point range".into(),
            ));
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn near(a: &Matrix, b: &Matrix) {
        assert_eq!((a.rows, a.cols), (b.rows, b.cols));
        for (x, y) in a.data.iter().zip(&b.data) {
            assert!((x - y).abs() < 1e-9, "{x} != {y}");
        }
    }
    #[test]
    fn moore_penrose_identities_rectangular_singular_and_scaled() {
        for a in [
            Matrix::new(3, 2, vec![1., 2., 3., 4., 5., 6.]).unwrap(),
            Matrix::new(2, 3, vec![1., 2., 3., 4., 5., 6.]).unwrap(),
            Matrix::new(2, 2, vec![1., 2., 2., 4.]).unwrap(),
            Matrix::new(2, 3, vec![0.; 6]).unwrap(),
        ] {
            let p = a.pseudoinverse(None).unwrap();
            near(&a.matmul(&p).unwrap().matmul(&a).unwrap(), &a);
            near(&p.matmul(&a).unwrap().matmul(&p).unwrap(), &p);
            let ap = a.matmul(&p).unwrap();
            near(&ap, &ap.transpose());
            let pa = p.matmul(&a).unwrap();
            near(&pa, &pa.transpose());
            let scaled = a
                .map(|x| x * 1e100)
                .pseudoinverse(None)
                .unwrap()
                .map(|x| x * 1e100);
            near(&p, &scaled);
        }
    }
    #[test]
    fn eigenvalues_real_and_complex_preserve_trace_and_determinant() {
        for a in [
            Matrix::new(2, 2, vec![0., -1., 1., 0.]).unwrap(),
            Matrix::new(3, 3, vec![2., 1., 0., 0., 3., 1., 0., 0., 4.]).unwrap(),
        ] {
            let e = a.eigenvalues().unwrap();
            let sum: Complex = e.iter().sum();
            let product: Complex = e.iter().product();
            assert!(
                (sum.re - (0..a.rows).map(|i| a.data[i * a.cols + i]).sum::<f64>()).abs() < 1e-9
            );
            assert!((product.re - a.determinant().unwrap()).abs() < 1e-9);
            assert!(sum.im.abs() < 1e-9 && product.im.abs() < 1e-9);
        }
        assert!(Matrix::vector(vec![f64::NAN]).eigenvalues().is_err());
        assert!(Matrix::scalar(1.).pseudoinverse(Some(-1.)).is_err());
    }
    #[test]
    fn matrix_power_and_least_squares() {
        let a = Matrix::new(2, 2, vec![1., 2., 0., 1.]).unwrap();
        near(
            &a.power(3.).unwrap(),
            &Matrix::new(2, 2, vec![1., 6., 0., 1.]).unwrap(),
        );
        near(
            &a.power(-1.).unwrap().matmul(&a).unwrap(),
            &a.power(0.).unwrap(),
        );
        assert!(a.power(0.5).is_err());
        let design = Matrix::new(3, 2, vec![1., 0., 1., 1., 1., 2.]).unwrap();
        near(
            &design
                .solve(&Matrix::new(3, 1, vec![1., 3., 5.]).unwrap())
                .unwrap(),
            &Matrix::new(2, 1, vec![1., 2.]).unwrap(),
        );
    }
}
