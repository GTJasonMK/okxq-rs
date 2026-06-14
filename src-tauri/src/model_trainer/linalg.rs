/// 通过正规方程求解线性回归权重。
/// X 的形状 [N, D]（第0列已包含 intercept），y 的长度 N。
/// 返回权重向量 w [D]。
pub(super) fn solve_normal_equation(
    x: &[Vec<f64>],
    y: &[f64],
    n_cols: usize,
) -> Result<Vec<f64>, String> {
    let n = x.len();
    let mut xtx = vec![vec![0.0; n_cols]; n_cols];
    let mut xty = vec![0.0; n_cols];

    for i in 0..n {
        for (j, xtx_row) in xtx.iter_mut().enumerate().take(n_cols) {
            let x_ij = x[i][j];
            xty[j] += x_ij * y[i];
            for (k, xtx_cell) in xtx_row.iter_mut().enumerate().take(n_cols) {
                *xtx_cell += x_ij * x[i][k];
            }
        }
    }

    let lambda = 0.001;
    for (j, xtx_row) in xtx.iter_mut().enumerate().take(n_cols) {
        xtx_row[j] += lambda;
    }

    solve_linear_system(&xtx, &xty, n_cols)
        .ok_or_else(|| "矩阵求解失败：特征矩阵可能奇异".to_string())
}

/// 简单的高斯消元 + 回代求解 Ax = b。
fn solve_linear_system(a: &[Vec<f64>], b: &[f64], n: usize) -> Option<Vec<f64>> {
    let mut m: Vec<Vec<f64>> = a
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut r = row.clone();
            r.push(b[i]);
            r
        })
        .collect();

    for col in 0..n {
        let mut max_row = col;
        let mut max_val = m[col][col].abs();
        for (row, m_row) in m.iter().enumerate().take(n).skip(col + 1) {
            if m_row[col].abs() > max_val {
                max_val = m_row[col].abs();
                max_row = row;
            }
        }
        if max_val < 1e-15 {
            return None;
        }
        m.swap(col, max_row);

        let pivot_row = m[col].clone();
        for m_row in m.iter_mut().take(n).skip(col + 1) {
            let factor = m_row[col] / pivot_row[col];
            for (cell, pivot_cell) in m_row.iter_mut().zip(pivot_row.iter()).take(n + 1).skip(col) {
                *cell -= factor * pivot_cell;
            }
        }
    }

    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut sum = m[i][n];
        for j in (i + 1)..n {
            sum -= m[i][j] * x[j];
        }
        x[i] = sum / m[i][i];
    }
    Some(x)
}
