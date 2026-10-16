//! Native feasibility probe for Tellegen's opt-in POUNCE boundary.
//!
//! This is intentionally not an AC OPF model. It exercises the API and
//! derivative path selected for that future model: an in-memory expression
//! DAG, `NlTnlp`, exact sparse Jacobian/Hessian evaluation, and POUNCE's
//! interior-point driver. Run with:
//!
//! ```text
//! cargo run -p tellegen --example acopf_hs071_probe --features acopf
//! ```

use pounce_nl::nl_reader::{BinOp, Expr, NlProblem, NlProblemParts, NlTnlp};
use pounce_rs::{ApplicationReturnStatus, IpoptApplication, TNLP};
use std::cell::RefCell;
use std::rc::Rc;

const INF: f64 = 1.0e19;

fn constant(value: f64) -> Expr {
    Expr::Const(value)
}

fn variable(index: usize) -> Expr {
    Expr::Var(index)
}

fn binary(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
    Expr::Binary(op, Box::new(lhs), Box::new(rhs))
}

fn add(lhs: Expr, rhs: Expr) -> Expr {
    binary(BinOp::Add, lhs, rhs)
}

fn multiply(lhs: Expr, rhs: Expr) -> Expr {
    binary(BinOp::Mul, lhs, rhs)
}

fn square(value: Expr) -> Expr {
    binary(BinOp::Pow, value, constant(2.0))
}

fn hs071() -> Result<NlTnlp, String> {
    let x = [variable(0), variable(1), variable(2), variable(3)];
    let objective = add(
        multiply(
            multiply(x[0].clone(), x[3].clone()),
            Expr::Sum(vec![x[0].clone(), x[1].clone(), x[2].clone()]),
        ),
        x[2].clone(),
    );
    let product = multiply(
        multiply(x[0].clone(), x[1].clone()),
        multiply(x[2].clone(), x[3].clone()),
    );
    let sum_of_squares = Expr::Sum(x.into_iter().map(square).collect());

    let problem = NlProblem::from_expressions(NlProblemParts {
        minimize: true,
        objective,
        obj_constant: 0.0,
        constraints: vec![product, sum_of_squares],
        x_l: vec![1.0; 4],
        x_u: vec![5.0; 4],
        x0: vec![1.0, 5.0, 5.0, 1.0],
        g_l: vec![25.0, 40.0],
        g_u: vec![INF, 40.0],
        var_names: (1..=4).map(|i| format!("x{i}")).collect(),
        con_names: vec!["product".into(), "sum_of_squares".into()],
    })?;

    NlTnlp::try_new(problem)
}

fn main() -> Result<(), String> {
    let mut tnlp = hs071()?;
    let info = tnlp
        .get_nlp_info()
        .ok_or("POUNCE did not return problem dimensions")?;
    if (info.n, info.m, info.nnz_jac_g, info.nnz_h_lag) != (4, 2, 8, 10) {
        return Err(format!(
            "unexpected sparse derivative structure: n={}, m={}, nnz(J)={}, nnz(H)={}",
            info.n, info.m, info.nnz_jac_g, info.nnz_h_lag
        ));
    }

    let tnlp = Rc::new(RefCell::new(tnlp));
    let mut app = IpoptApplication::new();
    app.initialize_with_options_str(
        "linear_solver feral\n\
         hessian_approximation exact\n\
         nlp_scaling_method none\n\
         linear_system_scaling none\n\
         tol 1e-9\n\
         constr_viol_tol 1e-9\n\
         print_level 0\n",
    )
    .map_err(|error| format!("could not configure POUNCE: {error}"))?;
    app.initialize()
        .map_err(|error| format!("could not initialize POUNCE: {error}"))?;

    let status = app.optimize_tnlp(Rc::clone(&tnlp) as Rc<RefCell<dyn TNLP>>);
    if status != ApplicationReturnStatus::SolveSucceeded {
        return Err(format!("POUNCE returned {status:?}"));
    }

    let solved = tnlp.borrow();
    let x = solved
        .final_x()
        .ok_or("POUNCE reported success without a primal solution")?;
    let objective = solved.final_obj();
    let product: f64 = x.iter().product();
    let sum_of_squares: f64 = x.iter().map(|value| value * value).sum();
    let max_constraint_violation = (25.0 - product).max(0.0).max((sum_of_squares - 40.0).abs());

    if (objective - 17.014_017_3).abs() > 1.0e-5 || max_constraint_violation > 1.0e-7 {
        return Err(format!(
            "independent HS071 check failed: objective={objective:.12}, violation={max_constraint_violation:.3e}, x={x:?}"
        ));
    }

    println!(
        "HS071: {status:?}; objective={objective:.9}; max_violation={max_constraint_violation:.3e}; nnz(J)={}; nnz(H)={}",
        info.nnz_jac_g, info.nnz_h_lag
    );
    Ok(())
}
