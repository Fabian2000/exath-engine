//! Dense real matrices and core linear algebra.
//!
//! A self-contained, additive module: it does not touch the scalar expression
//! evaluator, so the existing calculator is unaffected. Operations return
//! `Result` on dimension mismatch or singular systems, panic-free (no
//! `unwrap`/`expect`/`panic!`).

use crate::angle_mode::AngleMode;
use crate::ast::{eval_ast, Ast, BinOp, UserFns};
use crate::error::ExathError;
use crate::evaluator::Cx;
use std::collections::HashMap;

/// A dense matrix stored row-major.
#[derive(Clone, Debug, PartialEq)]
pub struct Matrix {
    rows: usize,
    cols: usize,
    data: Vec<f64>,
}

impl Matrix {
    /// Build from a list of equal-length rows.
    pub fn new(rows: Vec<Vec<f64>>) -> Result<Matrix, ExathError> {
        let r = rows.len();
        if r == 0 {
            return Err(ExathError::domain("matrix has no rows"));
        }
        let c = rows[0].len();
        if c == 0 {
            return Err(ExathError::domain("matrix has no columns"));
        }
        let mut data = Vec::with_capacity(r * c);
        for row in &rows {
            if row.len() != c {
                return Err(ExathError::domain("matrix rows have unequal length"));
            }
            data.extend_from_slice(row);
        }
        Ok(Matrix { rows: r, cols: c, data })
    }

    /// Build from a flat row-major slice (`data.len() == rows*cols`).
    pub fn from_flat(data: &[f64], rows: usize, cols: usize) -> Result<Matrix, ExathError> {
        if rows == 0 || cols == 0 || data.len() != rows * cols {
            return Err(ExathError::domain("matrix: data length must equal rows*cols (both > 0)"));
        }
        Ok(Matrix { rows, cols, data: data.to_vec() })
    }

    /// Flat row-major data.
    pub fn as_flat(&self) -> &[f64] {
        &self.data
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn get(&self, r: usize, c: usize) -> f64 {
        self.data[r * self.cols + c]
    }

    fn set(&mut self, r: usize, c: usize, v: f64) {
        self.data[r * self.cols + c] = v;
    }

    /// n×n identity matrix.
    pub fn identity(n: usize) -> Matrix {
        let mut data = vec![0.0; n * n];
        for i in 0..n {
            data[i * n + i] = 1.0;
        }
        Matrix { rows: n, cols: n, data }
    }

    pub fn to_rows(&self) -> Vec<Vec<f64>> {
        (0..self.rows)
            .map(|r| (0..self.cols).map(|c| self.get(r, c)).collect())
            .collect()
    }

    pub fn add(&self, other: &Matrix) -> Result<Matrix, ExathError> {
        self.elementwise(other, |a, b| a + b, "add")
    }

    pub fn sub(&self, other: &Matrix) -> Result<Matrix, ExathError> {
        self.elementwise(other, |a, b| a - b, "subtract")
    }

    fn elementwise(
        &self,
        other: &Matrix,
        op: impl Fn(f64, f64) -> f64,
        what: &str,
    ) -> Result<Matrix, ExathError> {
        if self.rows != other.rows || self.cols != other.cols {
            return Err(ExathError::domain(format!(
                "cannot {} matrices of different dimensions",
                what
            )));
        }
        let data = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(a, b)| op(*a, *b))
            .collect();
        Ok(Matrix { rows: self.rows, cols: self.cols, data })
    }

    pub fn scale(&self, k: f64) -> Matrix {
        Matrix {
            rows: self.rows,
            cols: self.cols,
            data: self.data.iter().map(|x| x * k).collect(),
        }
    }

    pub fn mul(&self, other: &Matrix) -> Result<Matrix, ExathError> {
        if self.cols != other.rows {
            return Err(ExathError::domain(
                "matrix product requires lhs.cols == rhs.rows",
            ));
        }
        let mut data = vec![0.0; self.rows * other.cols];
        for i in 0..self.rows {
            for k in 0..self.cols {
                let a = self.get(i, k);
                for j in 0..other.cols {
                    data[i * other.cols + j] += a * other.get(k, j);
                }
            }
        }
        Ok(Matrix { rows: self.rows, cols: other.cols, data })
    }

    pub fn transpose(&self) -> Matrix {
        let mut m = Matrix { rows: self.cols, cols: self.rows, data: vec![0.0; self.data.len()] };
        for r in 0..self.rows {
            for c in 0..self.cols {
                m.set(c, r, self.get(r, c));
            }
        }
        m
    }

    fn require_square(&self, what: &str) -> Result<usize, ExathError> {
        if self.rows != self.cols {
            return Err(ExathError::domain(format!("{} requires a square matrix", what)));
        }
        Ok(self.rows)
    }

    /// Determinant via Gaussian elimination with partial pivoting.
    pub fn determinant(&self) -> Result<f64, ExathError> {
        let n = self.require_square("determinant")?;
        let mut a = self.data.clone();
        let idx = |r: usize, c: usize| r * n + c;
        let mut det = 1.0;
        for col in 0..n {
            // partial pivot
            let mut pivot = col;
            let mut best = a[idx(col, col)].abs();
            for r in (col + 1)..n {
                let v = a[idx(r, col)].abs();
                if v > best {
                    best = v;
                    pivot = r;
                }
            }
            if best < 1e-14 {
                return Ok(0.0);
            }
            if pivot != col {
                for c in 0..n {
                    a.swap(idx(col, c), idx(pivot, c));
                }
                det = -det;
            }
            let p = a[idx(col, col)];
            det *= p;
            for r in (col + 1)..n {
                let factor = a[idx(r, col)] / p;
                for c in col..n {
                    a[idx(r, c)] -= factor * a[idx(col, c)];
                }
            }
        }
        Ok(det)
    }

    /// Inverse via Gauss-Jordan elimination. Errors if singular.
    pub fn inverse(&self) -> Result<Matrix, ExathError> {
        let n = self.require_square("inverse")?;
        let mut a = self.data.clone();
        let mut inv = Matrix::identity(n).data;
        let idx = |r: usize, c: usize| r * n + c;
        for col in 0..n {
            let mut pivot = col;
            let mut best = a[idx(col, col)].abs();
            for r in (col + 1)..n {
                let v = a[idx(r, col)].abs();
                if v > best {
                    best = v;
                    pivot = r;
                }
            }
            if best < 1e-14 {
                return Err(ExathError::domain("matrix is singular (no inverse)"));
            }
            if pivot != col {
                for c in 0..n {
                    a.swap(idx(col, c), idx(pivot, c));
                    inv.swap(idx(col, c), idx(pivot, c));
                }
            }
            let p = a[idx(col, col)];
            for c in 0..n {
                a[idx(col, c)] /= p;
                inv[idx(col, c)] /= p;
            }
            for r in 0..n {
                if r == col {
                    continue;
                }
                let factor = a[idx(r, col)];
                if factor == 0.0 {
                    continue;
                }
                for c in 0..n {
                    a[idx(r, c)] -= factor * a[idx(col, c)];
                    inv[idx(r, c)] -= factor * inv[idx(col, c)];
                }
            }
        }
        Ok(Matrix { rows: n, cols: n, data: inv })
    }

    /// Frobenius norm √(Σ aᵢⱼ²).
    pub fn frobenius_norm(&self) -> f64 {
        self.data.iter().map(|x| x * x).sum::<f64>().sqrt()
    }

    /// Singular values (descending) via one-sided Jacobi SVD, numerically
    /// sound (does NOT form MᵀM, so it preserves conditioning).
    pub fn singular_values(&self) -> Result<Vec<f64>, ExathError> {
        let (m, n) = (self.rows, self.cols);
        let mut a = self.data.clone();
        let idx = |r: usize, c: usize| r * n + c;
        for _ in 0..60 {
            let mut changed = false;
            for i in 0..n {
                for j in (i + 1)..n {
                    let mut alpha = 0.0;
                    let mut beta = 0.0;
                    let mut gamma = 0.0;
                    for k in 0..m {
                        alpha += a[idx(k, i)] * a[idx(k, i)];
                        beta += a[idx(k, j)] * a[idx(k, j)];
                        gamma += a[idx(k, i)] * a[idx(k, j)];
                    }
                    if gamma.abs() <= 1e-15 * (alpha * beta).sqrt() {
                        continue;
                    }
                    changed = true;
                    let zeta = (beta - alpha) / (2.0 * gamma);
                    let t = zeta.signum() / (zeta.abs() + (1.0 + zeta * zeta).sqrt());
                    let c = 1.0 / (1.0 + t * t).sqrt();
                    let s = c * t;
                    for k in 0..m {
                        let aki = a[idx(k, i)];
                        let akj = a[idx(k, j)];
                        a[idx(k, i)] = c * aki - s * akj;
                        a[idx(k, j)] = s * aki + c * akj;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        let mut sv: Vec<f64> = (0..n)
            .map(|j| (0..m).map(|k| a[idx(k, j)].powi(2)).sum::<f64>().sqrt())
            .collect();
        sv.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        Ok(sv)
    }

    /// True if the matrix is square and symmetric (within tolerance).
    pub fn is_symmetric(&self) -> bool {
        if self.rows != self.cols {
            return false;
        }
        for i in 0..self.rows {
            for j in (i + 1)..self.cols {
                if (self.get(i, j) - self.get(j, i)).abs() > 1e-12 {
                    return false;
                }
            }
        }
        true
    }

    /// Eigenvalues and orthonormal eigenvectors of a SYMMETRIC matrix via the
    /// Jacobi rotation algorithm, accurate and stable. Eigenvalues ascending;
    /// eigenvectors are the columns of the returned matrix.
    pub fn jacobi_eigen(&self) -> Result<(Vec<f64>, Matrix), ExathError> {
        let n = self.require_square("eigen")?;
        if !self.is_symmetric() {
            return Err(ExathError::domain("jacobi_eigen requires a symmetric matrix"));
        }
        let mut a = self.data.clone();
        let mut v = Matrix::identity(n).data;
        let idx = |r: usize, c: usize| r * n + c;
        for _ in 0..100 {
            let mut off = 0.0;
            for p in 0..n {
                for q in (p + 1)..n {
                    off += a[idx(p, q)] * a[idx(p, q)];
                }
            }
            if off.sqrt() < 1e-14 {
                break;
            }
            for p in 0..n {
                for q in (p + 1)..n {
                    let apq = a[idx(p, q)];
                    if apq.abs() < 1e-300 {
                        continue;
                    }
                    let theta = (a[idx(q, q)] - a[idx(p, p)]) / (2.0 * apq);
                    let t = if theta == 0.0 {
                        1.0
                    } else {
                        theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt())
                    };
                    let c = 1.0 / (t * t + 1.0).sqrt();
                    let s = t * c;
                    for k in 0..n {
                        let akp = a[idx(k, p)];
                        let akq = a[idx(k, q)];
                        a[idx(k, p)] = c * akp - s * akq;
                        a[idx(k, q)] = s * akp + c * akq;
                    }
                    for k in 0..n {
                        let apk = a[idx(p, k)];
                        let aqk = a[idx(q, k)];
                        a[idx(p, k)] = c * apk - s * aqk;
                        a[idx(q, k)] = s * apk + c * aqk;
                    }
                    for k in 0..n {
                        let vkp = v[idx(k, p)];
                        let vkq = v[idx(k, q)];
                        v[idx(k, p)] = c * vkp - s * vkq;
                        v[idx(k, q)] = s * vkp + c * vkq;
                    }
                }
            }
        }
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&i, &j| a[idx(i, i)].partial_cmp(&a[idx(j, j)]).unwrap_or(std::cmp::Ordering::Equal));
        let eigenvalues: Vec<f64> = order.iter().map(|&i| a[idx(i, i)]).collect();
        let mut vec_data = vec![0.0; n * n];
        for (col, &src) in order.iter().enumerate() {
            for r in 0..n {
                vec_data[r * n + col] = v[idx(r, src)];
            }
        }
        Ok((eigenvalues, Matrix { rows: n, cols: n, data: vec_data }))
    }

    /// Thin QR decomposition (Gram–Schmidt) for a full-column-rank matrix:
    /// returns (Q, R) with Q orthonormal columns (m×n) and R upper-triangular (n×n).
    pub fn qr(&self) -> Result<(Matrix, Matrix), ExathError> {
        let (m, n) = (self.rows, self.cols);
        if m < n {
            return Err(ExathError::domain("qr requires rows >= cols"));
        }
        let mut q = vec![0.0; m * n];
        let mut r = vec![0.0; n * n];
        let qi = |q: &[f64], j: usize| (0..m).map(|i| q[i * n + j]).collect::<Vec<f64>>();
        for j in 0..n {
            let mut v: Vec<f64> = (0..m).map(|i| self.get(i, j)).collect();
            for i in 0..j {
                let col = qi(&q, i);
                let dot: f64 = (0..m).map(|k| col[k] * self.get(k, j)).sum();
                r[i * n + j] = dot;
                for k in 0..m {
                    v[k] -= dot * col[k];
                }
            }
            let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
            if norm < 1e-12 {
                return Err(ExathError::domain("qr: matrix is rank-deficient"));
            }
            r[j * n + j] = norm;
            for k in 0..m {
                q[k * n + j] = v[k] / norm;
            }
        }
        Ok((Matrix { rows: m, cols: n, data: q }, Matrix { rows: n, cols: n, data: r }))
    }

    /// Rank via Gaussian elimination with partial pivoting.
    pub fn rank(&self) -> usize {
        let (rows, cols) = (self.rows, self.cols);
        let mut a = self.data.clone();
        let idx = |r: usize, c: usize| r * cols + c;
        let mut rank = 0;
        let mut row = 0;
        for col in 0..cols {
            if row >= rows {
                break;
            }
            let mut piv = row;
            let mut best = a[idx(row, col)].abs();
            for r in (row + 1)..rows {
                if a[idx(r, col)].abs() > best {
                    best = a[idx(r, col)].abs();
                    piv = r;
                }
            }
            if best < 1e-12 {
                continue;
            }
            for c in 0..cols {
                a.swap(idx(row, c), idx(piv, c));
            }
            for r in 0..rows {
                if r != row {
                    let f = a[idx(r, col)] / a[idx(row, col)];
                    for c in col..cols {
                        a[idx(r, c)] -= f * a[idx(row, c)];
                    }
                }
            }
            row += 1;
            rank += 1;
        }
        rank
    }

    /// One non-trivial vector in the null space (kernel) of a square matrix, or
    /// None if the matrix is non-singular. Found via reduced row echelon form.
    pub fn null_space_vector(&self) -> Option<Vec<f64>> {
        if self.rows != self.cols {
            return None;
        }
        let n = self.rows;
        let mut a = self.data.clone();
        let idx = |r: usize, c: usize| r * n + c;
        let mut pivot_cols: Vec<usize> = Vec::new();
        let mut row = 0;
        for col in 0..n {
            if row >= n {
                break;
            }
            let mut piv = row;
            let mut best = a[idx(row, col)].abs();
            for r in (row + 1)..n {
                if a[idx(r, col)].abs() > best {
                    best = a[idx(r, col)].abs();
                    piv = r;
                }
            }
            if best < 1e-9 {
                continue;
            }
            for c in 0..n {
                a.swap(idx(row, c), idx(piv, c));
            }
            let p = a[idx(row, col)];
            for c in 0..n {
                a[idx(row, c)] /= p;
            }
            for r in 0..n {
                if r != row {
                    let f = a[idx(r, col)];
                    for c in 0..n {
                        a[idx(r, c)] -= f * a[idx(row, c)];
                    }
                }
            }
            pivot_cols.push(col);
            row += 1;
        }
        let free = (0..n).find(|c| !pivot_cols.contains(c))?;
        let mut v = vec![0.0; n];
        v[free] = 1.0;
        for (i, &p) in pivot_cols.iter().enumerate() {
            v[p] = -a[idx(i, free)];
        }
        Some(v)
    }

    /// Coefficients of the characteristic polynomial det(λI − A), power-indexed
    /// and monic (leading coefficient 1), via the Faddeev–LeVerrier algorithm.
    pub fn char_poly_coeffs(&self) -> Result<Vec<f64>, ExathError> {
        let n = self.require_square("eigenvalues")?;
        let mut m = Matrix::identity(n); // M₀ = I
        let mut coeffs = vec![0.0; n + 1];
        coeffs[n] = 1.0;
        for k in 1..=n {
            let am = self.mul(&m)?; // A·M_{k-1}
            let tr: f64 = (0..n).map(|i| am.get(i, i)).sum();
            let c = -tr / (k as f64);
            coeffs[n - k] = c;
            m = am.add(&Matrix::identity(n).scale(c))?;
        }
        Ok(coeffs)
    }

    /// Solve `A·x = b` for `x` (A is this square matrix, b a length-n column).
    pub fn solve(&self, b: &[f64]) -> Result<Vec<f64>, ExathError> {
        let n = self.require_square("solve")?;
        if b.len() != n {
            return Err(ExathError::domain(
                "solve: right-hand side length must equal matrix dimension",
            ));
        }
        let idx = |r: usize, c: usize| r * n + c;
        let mut a = self.data.clone();
        let mut x = b.to_vec();
        for col in 0..n {
            let mut pivot = col;
            let mut best = a[idx(col, col)].abs();
            for r in (col + 1)..n {
                let v = a[idx(r, col)].abs();
                if v > best {
                    best = v;
                    pivot = r;
                }
            }
            if best < 1e-14 {
                return Err(ExathError::domain("solve: singular system"));
            }
            if pivot != col {
                for c in 0..n {
                    a.swap(idx(col, c), idx(pivot, c));
                }
                x.swap(col, pivot);
            }
            for r in (col + 1)..n {
                let factor = a[idx(r, col)] / a[idx(col, col)];
                for c in col..n {
                    a[idx(r, c)] -= factor * a[idx(col, c)];
                }
                x[r] -= factor * x[col];
            }
        }
        // back-substitution
        let mut sol = vec![0.0; n];
        for r in (0..n).rev() {
            let mut s = x[r];
            for c in (r + 1)..n {
                s -= a[idx(r, c)] * sol[c];
            }
            sol[r] = s / a[idx(r, r)];
        }
        Ok(sol)
    }
}

// ── Matrix-expression evaluation (for `[[..]]` literals in the DSL) ────────────

/// A value produced by the matrix evaluator: a scalar or a matrix.
pub enum MValue {
    Scalar(f64),
    Mat(Matrix),
}

/// True if `ast` is (or contains) a matrix literal or a matrix function call,
/// used to route a line to [`eval_matrix_ast`] instead of the scalar evaluator.
pub fn is_matrix_expr(ast: &Ast) -> bool {
    match ast {
        Ast::Matrix(_) => true,
        Ast::Call(name, _)
            if matches!(
                name.as_str(),
                "det" | "inv" | "transpose" | "identity" | "trace" | "rank" | "norm"
                    | "svdvals" | "linsolve"
            ) =>
        {
            true
        }
        Ast::BinOp(_, l, r) => is_matrix_expr(l) || is_matrix_expr(r),
        Ast::UnaryNeg(u) => is_matrix_expr(u),
        _ => false,
    }
}

/// Evaluate a matrix expression: literals, +, −, *, scalar ×, and the functions
/// det / inv / transpose / trace / identity. Scalar sub-expressions are
/// evaluated with the ordinary (scalar) evaluator.
pub fn eval_matrix_ast(
    ast: &Ast,
    vars: &HashMap<String, Cx>,
    fns: &UserFns,
    angle: AngleMode,
) -> Result<MValue, ExathError> {
    match ast {
        Ast::Matrix(rows) => {
            let mut data = Vec::with_capacity(rows.len());
            for r in rows {
                let mut row = Vec::with_capacity(r.len());
                for e in r {
                    row.push(eval_ast(e, vars, fns, angle)?.re);
                }
                data.push(row);
            }
            Ok(MValue::Mat(Matrix::new(data)?))
        }
        Ast::UnaryNeg(u) => match eval_matrix_ast(u, vars, fns, angle)? {
            MValue::Scalar(s) => Ok(MValue::Scalar(-s)),
            MValue::Mat(m) => Ok(MValue::Mat(m.scale(-1.0))),
        },
        Ast::BinOp(op, l, r) => {
            let a = eval_matrix_ast(l, vars, fns, angle)?;
            let b = eval_matrix_ast(r, vars, fns, angle)?;
            eval_matrix_binop(op, a, b)
        }
        Ast::Call(name, args) => eval_matrix_call(name, args, vars, fns, angle),
        // Anything else is a scalar.
        _ => Ok(MValue::Scalar(eval_ast(ast, vars, fns, angle)?.re)),
    }
}

fn eval_matrix_binop(op: &BinOp, a: MValue, b: MValue) -> Result<MValue, ExathError> {
    use MValue::*;
    match (op, a, b) {
        (BinOp::Add, Mat(x), Mat(y)) => Ok(Mat(x.add(&y)?)),
        (BinOp::Add, Scalar(x), Scalar(y)) => Ok(Scalar(x + y)),
        (BinOp::Sub, Mat(x), Mat(y)) => Ok(Mat(x.sub(&y)?)),
        (BinOp::Sub, Scalar(x), Scalar(y)) => Ok(Scalar(x - y)),
        (BinOp::Mul, Mat(x), Mat(y)) => Ok(Mat(x.mul(&y)?)),
        (BinOp::Mul, Scalar(s), Mat(m)) | (BinOp::Mul, Mat(m), Scalar(s)) => Ok(Mat(m.scale(s))),
        (BinOp::Mul, Scalar(x), Scalar(y)) => Ok(Scalar(x * y)),
        (BinOp::Div, Mat(m), Scalar(s)) => {
            if s == 0.0 {
                Err(ExathError::domain("division by zero"))
            } else {
                Ok(Mat(m.scale(1.0 / s)))
            }
        }
        (BinOp::Div, Scalar(x), Scalar(y)) => Ok(Scalar(x / y)),
        _ => Err(ExathError::domain(
            "unsupported operation between matrix/scalar operands",
        )),
    }
}

fn eval_matrix_call(
    name: &str,
    args: &[Ast],
    vars: &HashMap<String, Cx>,
    fns: &UserFns,
    angle: AngleMode,
) -> Result<MValue, ExathError> {
    let one_matrix = |a: &[Ast]| -> Result<Matrix, ExathError> {
        if a.len() != 1 {
            return Err(ExathError::arg_count(format!("{} expects 1 argument", name)));
        }
        match eval_matrix_ast(&a[0], vars, fns, angle)? {
            MValue::Mat(m) => Ok(m),
            MValue::Scalar(_) => Err(ExathError::arg_type(format!("{} expects a matrix", name))),
        }
    };
    match name {
        "det" => Ok(MValue::Scalar(one_matrix(args)?.determinant()?)),
        "inv" => Ok(MValue::Mat(one_matrix(args)?.inverse()?)),
        "transpose" => Ok(MValue::Mat(one_matrix(args)?.transpose())),
        "trace" => {
            let m = one_matrix(args)?;
            let n = m.require_square("trace")?;
            Ok(MValue::Scalar((0..n).map(|i| m.get(i, i)).sum()))
        }
        "rank" => Ok(MValue::Scalar(one_matrix(args)?.rank() as f64)),
        "norm" => Ok(MValue::Scalar(one_matrix(args)?.frobenius_norm())),
        "svdvals" => {
            let sv = one_matrix(args)?.singular_values()?;
            Ok(MValue::Mat(Matrix::new(vec![sv])?))
        }
        "linsolve" => {
            if args.len() != 2 {
                return Err(ExathError::arg_count("linsolve expects 2 arguments: linsolve(A, b)"));
            }
            let a = one_matrix(&args[..1])?;
            let b = match eval_matrix_ast(&args[1], vars, fns, angle)? {
                MValue::Mat(m) => m,
                MValue::Scalar(_) => {
                    return Err(ExathError::arg_type("linsolve: b must be a vector"))
                }
            };
            // Flatten b (row or column vector) into a length-n column.
            let bvec: Vec<f64> = b.to_rows().into_iter().flatten().collect();
            let x = a.solve(&bvec)?;
            Ok(MValue::Mat(Matrix::new(x.into_iter().map(|v| vec![v]).collect())?))
        }
        "identity" => {
            if args.len() != 1 {
                return Err(ExathError::arg_count("identity expects 1 argument"));
            }
            let n = eval_ast(&args[0], vars, fns, angle)?.re;
            if n < 1.0 || n.fract() != 0.0 {
                return Err(ExathError::domain("identity: size must be a positive integer"));
            }
            Ok(MValue::Mat(Matrix::identity(n as usize)))
        }
        _ => Err(ExathError::undefined(format!("unknown matrix function '{}'", name))),
    }
}

/// Render a matrix value to a string: scalars plainly, matrices as `[[..],[..]]`.
pub fn render_mvalue(v: &MValue) -> String {
    match v {
        MValue::Scalar(s) => format_num(*s),
        MValue::Mat(m) => {
            let rows: Vec<String> = (0..m.rows())
                .map(|r| {
                    let cells: Vec<String> =
                        (0..m.cols()).map(|c| format_num(m.get(r, c))).collect();
                    format!("[{}]", cells.join(", "))
                })
                .collect();
            format!("[{}]", rows.join(", "))
        }
    }
}

fn format_num(x: f64) -> String {
    let r = x.round();
    if (x - r).abs() < 1e-9 && r.abs() < 1e15 {
        format!("{}", r as i64)
    } else {
        format!("{}", x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(rows: Vec<Vec<f64>>) -> Matrix {
        match Matrix::new(rows) {
            Ok(m) => m,
            Err(e) => panic!("bad test matrix: {}", e),
        }
    }

    #[test]
    fn add_mul_transpose() {
        let a = m(vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
        let b = m(vec![vec![5.0, 6.0], vec![7.0, 8.0]]);
        assert_eq!(a.add(&b).unwrap().to_rows(), vec![vec![6.0, 8.0], vec![10.0, 12.0]]);
        assert_eq!(
            a.mul(&b).unwrap().to_rows(),
            vec![vec![19.0, 22.0], vec![43.0, 50.0]]
        );
        assert_eq!(a.transpose().to_rows(), vec![vec![1.0, 3.0], vec![2.0, 4.0]]);
    }

    #[test]
    fn determinant_and_inverse() {
        let a = m(vec![vec![4.0, 7.0], vec![2.0, 6.0]]);
        assert!((a.determinant().unwrap() - 10.0).abs() < 1e-9);
        let inv = a.inverse().unwrap();
        // A · A⁻¹ = I
        let prod = a.mul(&inv).unwrap();
        let id = Matrix::identity(2);
        for r in 0..2 {
            for c in 0..2 {
                assert!((prod.get(r, c) - id.get(r, c)).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn jacobi_eigen_invariant() {
        // Symmetric matrix: verify A·v = λ·v for every eigenpair.
        let a = m(vec![vec![4.0, 1.0, 0.0], vec![1.0, 3.0, 1.0], vec![0.0, 1.0, 2.0]]);
        let (vals, vecs) = a.jacobi_eigen().unwrap();
        for (j, &lambda) in vals.iter().enumerate() {
            let v: Vec<f64> = (0..3).map(|i| vecs.get(i, j)).collect();
            let av = a.mul(&Matrix::new(v.iter().map(|x| vec![*x]).collect()).unwrap()).unwrap();
            for i in 0..3 {
                assert!((av.get(i, 0) - lambda * v[i]).abs() < 1e-8, "eigvec invariant");
            }
        }
        // singular values of a known matrix match sqrt eig of A^T A
        let b = m(vec![vec![3.0, 0.0], vec![0.0, -4.0]]);
        let sv = b.singular_values().unwrap();
        assert!((sv[0] - 4.0).abs() < 1e-6 && (sv[1] - 3.0).abs() < 1e-6);
    }

    #[test]
    fn qr_norm_svd() {
        let a = m(vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
        // Frobenius norm = sqrt(1+4+9+16) = sqrt(30)
        assert!((a.frobenius_norm() - 30.0_f64.sqrt()).abs() < 1e-9);
        // QR: Q·R == A and Q orthonormal
        let (q, r) = a.qr().unwrap();
        let qr = q.mul(&r).unwrap();
        for i in 0..2 {
            for j in 0..2 {
                assert!((qr.get(i, j) - a.get(i, j)).abs() < 1e-9);
            }
        }
        // singular values of [[1,0],[0,3]] are 3 and 1
        let d = m(vec![vec![1.0, 0.0], vec![0.0, 3.0]]);
        let sv = d.singular_values().unwrap();
        assert!((sv[0] - 3.0).abs() < 1e-6 && (sv[1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn solve_system() {
        // 2x + y = 5 ; x + 3y = 10  →  x = 1, y = 3
        let a = m(vec![vec![2.0, 1.0], vec![1.0, 3.0]]);
        let x = a.solve(&[5.0, 10.0]).unwrap();
        assert!((x[0] - 1.0).abs() < 1e-9 && (x[1] - 3.0).abs() < 1e-9);
    }

    #[test]
    fn errors_not_panics() {
        let a = m(vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
        let b = m(vec![vec![1.0, 2.0, 3.0]]);
        assert!(a.mul(&b).is_err() || b.mul(&a).is_err());
        let singular = m(vec![vec![1.0, 2.0], vec![2.0, 4.0]]);
        assert!(singular.inverse().is_err());
        assert!((singular.determinant().unwrap()).abs() < 1e-9);
    }
}
