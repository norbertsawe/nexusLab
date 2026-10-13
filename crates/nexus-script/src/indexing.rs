use super::{Expr, Matrix, ScriptError, Session, Value, MAX_ELEMENTS};

struct Selection {
    offsets: Vec<usize>,
    rows: usize,
    cols: usize,
}
impl Session {
    fn index_values(&mut self, expr: &Expr, limit: usize) -> Result<Matrix, ScriptError> {
        let previous = self.index_limit.replace(limit);
        let result = self.eval(expr);
        self.index_limit = previous;
        let matrix = result?.real()?.clone();
        if !matrix
            .data
            .iter()
            .all(|x| x.is_finite() && x.fract() == 0.0 && *x >= 1.0 && *x <= limit as f64)
        {
            return Err(ScriptError::Eval(format!(
                "indices must be integers in 1..{limit}"
            )));
        }
        Ok(matrix)
    }
    fn selection(&mut self, matrix: &Matrix, args: &[Expr]) -> Result<Selection, ScriptError> {
        match args {
            [expr] => {
                let index = self.index_values(expr, matrix.data.len())?;
                let (rows, cols) = if matches!(expr, Expr::AllIndex) {
                    (index.data.len(), 1)
                } else if (matrix.rows == 1 || matrix.cols == 1)
                    && (index.rows == 1 || index.cols == 1)
                {
                    if matrix.rows == 1 {
                        (1, index.data.len())
                    } else {
                        (index.data.len(), 1)
                    }
                } else {
                    (index.rows, index.cols)
                };
                Ok(Selection {
                    offsets: index
                        .data
                        .iter()
                        .map(|x| {
                            let i = *x as usize - 1;
                            (i % matrix.rows) * matrix.cols + i / matrix.rows
                        })
                        .collect(),
                    rows,
                    cols,
                })
            }
            [r, c] => {
                let rows = self.index_values(r, matrix.rows)?;
                let cols = self.index_values(c, matrix.cols)?;
                if rows.data.len().saturating_mul(cols.data.len()) > MAX_ELEMENTS {
                    return Err(ScriptError::Eval(
                        "selection exceeds one million elements".into(),
                    ));
                }
                // Subscript arrays are traversed in column-major order, regardless of their shape.
                let column_order = |m: &Matrix| -> Vec<usize> {
                    (0..m.data.len())
                        .map(|i| m.data[(i % m.rows) * m.cols + i / m.rows] as usize - 1)
                        .collect()
                };
                let r = column_order(&rows);
                let c = column_order(&cols);
                Ok(Selection {
                    offsets: r
                        .iter()
                        .flat_map(|r| c.iter().map(move |c| r * matrix.cols + c))
                        .collect(),
                    rows: r.len(),
                    cols: c.len(),
                })
            }
            _ => Err(ScriptError::Eval(
                "indexing expects one or two subscripts".into(),
            )),
        }
    }
    pub(super) fn read_indices(
        &mut self,
        matrix: &Matrix,
        args: &[Expr],
    ) -> Result<Value, ScriptError> {
        let selection = self.selection(matrix, args)?;
        Ok(Value::Real(Matrix::new(
            selection.rows,
            selection.cols,
            selection.offsets.iter().map(|i| matrix.data[*i]).collect(),
        )?))
    }
    pub(super) fn assign_indices(
        &mut self,
        name: &str,
        args: &[Expr],
        value: &Value,
    ) -> Result<(), ScriptError> {
        let mut matrix = self
            .variables
            .get(name)
            .ok_or_else(|| ScriptError::Eval(format!("unknown indexed variable '{name}'")))?
            .real()?
            .clone();
        let selection = self.selection(&matrix, args)?;
        let rhs = value.real()?;
        if rhs.data.len() != 1 && rhs.data.len() != selection.offsets.len() {
            return Err(ScriptError::Eval(
                "indexed assignment must preserve element count or use a scalar".into(),
            ));
        }
        if args.len() == 2
            && rhs.data.len() != 1
            && (rhs.rows, rhs.cols) != (selection.rows, selection.cols)
        {
            return Err(ScriptError::Eval(
                "two-subscript assignment requires matching rows and columns".into(),
            ));
        }
        // Linear assignments consume the RHS in column-major order. Repeated indices use the last value.
        if args.len() == 1 {
            let offsets: Vec<_> = (0..selection.offsets.len())
                .map(|i| {
                    selection.offsets[(i % selection.rows) * selection.cols + i / selection.rows]
                })
                .collect();
            for (i, offset) in offsets.iter().enumerate() {
                matrix.data[*offset] = if rhs.data.len() == 1 {
                    rhs.data[0]
                } else {
                    rhs.data[(i % rhs.rows) * rhs.cols + i / rhs.rows]
                };
            }
        } else {
            for (i, offset) in selection.offsets.iter().enumerate() {
                matrix.data[*offset] = if rhs.data.len() == 1 {
                    rhs.data[0]
                } else {
                    rhs.data[i]
                };
            }
        }
        self.insert_variable(name.to_owned(), Value::Real(matrix))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slicing_end_and_column_major_order() {
        let mut s = Session::new();
        s.evaluate("A=[1 2 3;4 5 6]; col=A(:,2); rows=A([2 1],1:2); flat=A(:); last=A(end); middle=A(2:end-1); reversed=A(end:-1:1)").unwrap();
        assert_eq!(
            s.variables["col"].real().unwrap(),
            &Matrix::new(2, 1, vec![2., 5.]).unwrap()
        );
        assert_eq!(
            s.variables["rows"].real().unwrap(),
            &Matrix::new(2, 2, vec![4., 5., 1., 2.]).unwrap()
        );
        assert_eq!(
            s.variables["flat"].real().unwrap(),
            &Matrix::new(6, 1, vec![1., 4., 2., 5., 3., 6.]).unwrap()
        );
        assert_eq!(
            s.variables["middle"].numeric_samples().unwrap(),
            vec![4., 2., 5., 3.]
        );
        assert_eq!(s.variables["last"].scalar().unwrap(), 6.);
        s.evaluate("v=[1;2;3]; w=v([3 1]); n=A(v(end),end)")
            .unwrap_err();
        assert_eq!(s.variables["w"].shape(), (2, 1));
        assert!(s.evaluate("end").is_err());
        assert!(s.evaluate("A(0)").is_err());
    }
    #[test]
    fn assignment_is_atomic_and_matches_linear_order() {
        let mut s = Session::new();
        s.evaluate("A=[1 2 3;4 5 6]; A(:,2)=[20;50]; A(1:2)=9; A([1 3;2 4])=[11 12;13 14]")
            .unwrap();
        assert_eq!(
            s.variables["A"].numeric_samples().unwrap(),
            vec![11., 12., 3., 13., 14., 6.]
        );
        let before = s.variables["A"].clone();
        assert!(s.evaluate("A([1 99])=[7 8]").is_err());
        assert_eq!(s.variables["A"], before);
        assert!(s.evaluate("A(:,2)=[1 2]").is_err());
        assert_eq!(s.variables["A"], before);
        s.evaluate("A([])=[]; A(end)=42; E=[]; empty=E(:)").unwrap();
        assert_eq!(s.variables["A"].real().unwrap().data[5], 42.);
        assert_eq!(s.variables["empty"].shape(), (0, 1));
    }
}
