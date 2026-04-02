use crate::frontend::ast::{self, TypeStateSet};
use crate::ir::cfg::{CFG, Statement, Expr, Terminator};
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;

// Σ : Peripheral → State
pub type StateEnv = HashMap<String, TypeStateSet>;

#[derive(Debug)]
pub enum TypestateError {
    // Failed premise in typing derivation
    InvalidTransition {
        func_name: String,
        called_from: String,
        peripheral: String,
        candidate_states: Vec<TypeStateSet>,
        actual_state: TypeStateSet,
    },

    // Violates the Branch typing rule: Σ ⊢ then : Σ₁ and Σ ⊢ else : Σ₂ requires Σ₁ = Σ₂
    BranchStateMismatch {
        func_name: String,
        peripheral: String,
        then_state: TypeStateSet,
        else_state: TypeStateSet,
    },

    // Violates the While rule: Σ ⊢ body : Σ' requires Σ = Σ'
    LoopChangesState {
        func_name: String,
        peripheral: String,
        before: TypeStateSet,
        after: TypeStateSet,
    },

    // Peripheral driver's derived effect != declared signature
    WrongExitState {
        func_name: String,
        peripheral: String,
        expected: TypeStateSet,
        actual: TypeStateSet,
    },

    // Unknown peripheral referenced
    UnknownPeripheral {
        func_name: String,
        name: String,
    },
}

fn fmt_typestate_set(s: &TypeStateSet) -> String {
    s.iter().cloned().collect::<Vec<_>>().join(" & ")
}

fn fmt_typestate_set_vec(v: &Vec<TypeStateSet>) -> String {
    v.iter()
        .map(|s| {
            if s.len() > 1 {
                format!("({})", fmt_typestate_set(s))
            } else {
                fmt_typestate_set(s)
            }
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

impl fmt::Display for TypestateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypestateError::InvalidTransition { func_name, called_from, peripheral, candidate_states, actual_state } => {
                write!(
                    f,
                    "Call to '{}' requires '{}' in state '{}', but found '{}' (called from '{}')",
                    func_name,
                    peripheral,
                    fmt_typestate_set_vec(candidate_states),
                    fmt_typestate_set(actual_state),
                    called_from,
                )
            }

            TypestateError::BranchStateMismatch { func_name, peripheral, then_state, else_state } => {
                write!(
                    f,
                    "Branch in '{}' leaves '{}' in different states: then = '{}', else = '{}'",
                    func_name, peripheral,
                    fmt_typestate_set(then_state),
                    fmt_typestate_set(else_state),
                )
            }

            TypestateError::LoopChangesState { func_name, peripheral, before, after } => {
                write!(
                    f,
                    "Loop in '{}' changes state of '{}': was '{}', now '{}'",
                    func_name, peripheral,
                    fmt_typestate_set(before),
                    fmt_typestate_set(after),
                )
            }

            TypestateError::WrongExitState { func_name, peripheral, expected, actual } => {
                write!(
                    f,
                    "Function '{}' declares output '{}' for '{}', but body produces '{}'",
                    func_name,
                    fmt_typestate_set(expected),
                    peripheral,
                    fmt_typestate_set(actual),
                )
            }

            TypestateError::UnknownPeripheral { func_name, name } => {
                write!(f, "Unknown peripheral '{}' in function '{}'", name, func_name)
            }
        }
    }
}

#[derive(Debug, PartialEq)]
enum FunctionType {
    // Has a typestate signature but calls no other driver functions
    // Axioms in our type system
    Leaf,

    // Has a typestate signature and calls other driver functions
    // Derived in our type system by verifying body matches their signature
    Composite,

    // No typestate signature, calls driver functions
    // Verified by checking all driver calls chain correctly
    Orchestration,
}

pub fn check(
    program: &ast::Program,
    ir: &[(String, CFG)],
) -> Result<(), String> {
    let signatures = build_signature_map(program);

    for (i, (_, cfg)) in ir.iter().enumerate() {
        let func = &program.functions[i];
        if let Err(err) = verify_function(func, cfg, &program.peripherals, &signatures) {
            return Err(format!("{}", err));
        }
    }

    Ok(())
}

fn build_signature_map(program: &ast::Program) -> HashMap<String, ast::TypeState> {
    let mut signatures = HashMap::new();
    for func in &program.functions {
        if let Some(sig) = &func.signature {
            signatures.insert(func.name.clone(), sig.clone());
        }
    }
    signatures
}

fn init_state_env(peripherals: &[ast::Peripheral]) -> StateEnv {
    peripherals
        .iter()
        .map(|p| {
            let mut set = TypeStateSet::new();
            set.insert(p.initial.clone());
            (p.name.clone(), set)
        })
        .collect()
}

fn classify_function(
    func: &ast::Function,
    cfg: &CFG,
    signatures: &HashMap<String, ast::TypeState>,
) -> FunctionType {
    let has_signature = func.signature.is_some();
    let calls_drivers = cfg_calls_drivers(cfg, signatures);

    match (has_signature, calls_drivers) {
        (true, false)  => FunctionType::Leaf,
        (true, true)   => FunctionType::Composite,
        (false, _)     => FunctionType::Orchestration,
    }
}

fn cfg_calls_drivers(cfg: &CFG, signatures: &HashMap<String, ast::TypeState>) -> bool {
    for block in &cfg.blocks {
        for stmt in &block.statements {
            match stmt {
                Statement::PeripheralDriverCall { .. } => return true,
                Statement::Expr { expr: Expr::FnCall { name, .. } } => {
                    if signatures.contains_key(name) {
                        return true;
                    }
                }
                _ => {}
            }
        }
    }
    false
}

/* Verify a single function's typestate correctness
 *
 *   Leaf:          Axiom: signature is trusted, no verification needed
 *   Composite:     Derive: compose called driver signatures, check result matches declaration
 *   Orchestration: Derive: compose called driver signatures, check all transitions are valid
 */
fn verify_function(
    func: &ast::Function,
    cfg: &CFG,
    peripherals: &[ast::Peripheral],
    signatures: &HashMap<String, ast::TypeState>,
) -> Result<(), TypestateError> {
    let kind = classify_function(func, cfg, signatures);
    let func_name = &func.name;

    match kind {
        /* Axiom: trusted, no verification needed
         *
         *   ────────────────────────────────── (axiom)
         *   Σ ⊢ leaf_driver() : Σ[P ↦ S_out]
         */
        FunctionType::Leaf => Ok(()),

        /* Derive: verify body composes correctly, then check against declared signature
         *
         *   Σ₀ ⊢ s₁ : Σ₁    Σ₁ ⊢ s₂ : Σ₂    ...    Σₙ₋₁ ⊢ sₙ : Σₙ
         *   ──────────────────────────────────────────────────────────── (seq)
         *                    Σ₀ ⊢ body : Σₙ
         *
         *   Then check: Σₙ(P) = declared output state
         */
        FunctionType::Composite => {
            let sig = func.signature.as_ref().unwrap();

            for input_set in &sig.input_states {
                let mut state_env = init_state_env(peripherals);
                state_env.insert(sig.peripheral.clone(), input_set.clone());

                verify_cfg(cfg, &mut state_env, signatures, func_name)?;

                let actual = state_env
                    .get(&sig.peripheral)
                    .ok_or_else(|| TypestateError::UnknownPeripheral {
                        func_name: func_name.clone(),
                        name: sig.peripheral.clone(),
                    })?;

                if actual != &sig.output_state {
                    return Err(TypestateError::WrongExitState {
                        func_name: func_name.clone(),
                        peripheral: sig.peripheral.clone(),
                        expected: sig.output_state.clone(),
                        actual: actual.clone(),
                    });
                }
            }

            Ok(())
        }

        // Orchestration: no declared signature, just verify all transitions are valid
        FunctionType::Orchestration => {
            let mut state_env = init_state_env(peripherals);
            verify_cfg(cfg, &mut state_env, signatures, func_name)?;
            Ok(())
        }
    }
}

fn verify_cfg(
    cfg: &CFG,
    state_env: &mut StateEnv,
    signatures: &HashMap<String, ast::TypeState>,
    func_name: &str,
) -> Result<(), TypestateError> {
    let mut visited = HashSet::new();
    let mut state_snapshots: HashMap<usize, StateEnv> = HashMap::new();
    verify_block_recursive(cfg, cfg.entry, state_env, signatures, &mut visited, func_name, &mut state_snapshots)
}

fn verify_block_recursive(
    cfg: &CFG,
    block_id: usize,
    state_env: &mut StateEnv,
    signatures: &HashMap<String, ast::TypeState>,
    visited: &mut HashSet<usize>,
    func_name: &str,
    state_snapshots: &mut HashMap<usize, StateEnv>,
) -> Result<(), TypestateError> {
    if visited.contains(&block_id) {
        return Ok(());
    }
    visited.insert(block_id);
    state_snapshots.insert(block_id, state_env.clone());

    let block = cfg.block(block_id);

    for stmt in &block.statements {
        verify_statement(stmt, state_env, signatures, func_name)?;
    }

    match &block.terminator {
        Terminator::Jump(target) => {
            if visited.contains(target) {
                if let Some(entry_state) = state_snapshots.get(target) {
                    for (peripheral, before) in entry_state {
                        if let Some(after) = state_env.get(peripheral) {
                            if before != after {
                                return Err(TypestateError::LoopChangesState {
                                    func_name: func_name.to_string(),
                                    peripheral: peripheral.clone(),
                                    before: before.clone(),
                                    after: after.clone(),
                                });
                            }
                        }
                    }
                }
            } else {
                verify_block_recursive(cfg, *target, state_env, signatures, visited, func_name, state_snapshots)?;
            }
        }

        Terminator::Branch { cond: _, then_block, else_block } |
        Terminator::CondBranch { then_block, else_block, .. } => {
            let mut then_env = state_env.clone();
            let mut else_env = state_env.clone();

            verify_block_recursive(cfg, *then_block, &mut then_env, signatures, &mut visited.clone(), func_name, &mut state_snapshots.clone())?;
            verify_block_recursive(cfg, *else_block, &mut else_env, signatures, &mut visited.clone(), func_name, &mut state_snapshots.clone())?;

            for (peripheral, then_state) in &then_env {
                if let Some(else_state) = else_env.get(peripheral) {
                    if then_state != else_state {
                        return Err(TypestateError::BranchStateMismatch {
                            func_name: func_name.to_string(),
                            peripheral: peripheral.clone(),
                            then_state: then_state.clone(),
                            else_state: else_state.clone(),
                        });
                    }
                }
            }

            *state_env = then_env;
        }

        Terminator::Return(_) | Terminator::None => {
            // End of control flow path
        }
    }

    Ok(())
}

fn state_satisfies(current: &TypeStateSet, candidates: &Vec<TypeStateSet>) -> bool {
    candidates.iter().any(|set| set.is_subset(current))
}

/* Verify a single statement's effect on the state environment
 *
 * Typing rule for driver calls:
 *
 *   ∃ s ∈ sig(f).input_states : s ⊆ Σ(P)    sig(f).output_state = S_out
 *   ──────────────────────────────────────────────────────────────────────── (driver-call)
 *                    Σ ⊢ f() : Σ[P ↦ S_out]
 */
fn verify_statement(
    stmt: &Statement,
    state_env: &mut StateEnv,
    signatures: &HashMap<String, ast::TypeState>,
    func_name: &str,
) -> Result<(), TypestateError> {
    match stmt {
        Statement::PeripheralDriverCall { function, peripheral, from_states, to_state } => {
            let current = state_env
                .get(peripheral)
                .ok_or_else(|| TypestateError::UnknownPeripheral {
                    func_name: func_name.to_string(),
                    name: peripheral.clone(),
                })?;

            if !state_satisfies(current, from_states) {
                return Err(TypestateError::InvalidTransition {
                    func_name: function.clone(),
                    called_from: func_name.to_string(),
                    peripheral: peripheral.clone(),
                    candidate_states: from_states.clone(),
                    actual_state: current.clone(),
                });
            }

            state_env.insert(peripheral.clone(), to_state.clone());
        }

        Statement::Expr { expr } => {
            if let Expr::FnCall { name, .. } = expr {
                if let Some(sig) = signatures.get(name) {
                    let current = state_env
                        .get(&sig.peripheral)
                        .ok_or_else(|| TypestateError::UnknownPeripheral {
                            func_name: func_name.to_string(),
                            name: sig.peripheral.clone(),
                        })?;

                    if !state_satisfies(current, &sig.input_states) {
                        return Err(TypestateError::InvalidTransition {
                            func_name: name.clone(),
                            called_from: func_name.to_string(),
                            peripheral: sig.peripheral.clone(),
                            candidate_states: sig.input_states.clone(),
                            actual_state: current.clone(),
                        });
                    }

                    state_env.insert(sig.peripheral.clone(), sig.output_state.clone());
                }
            }
        }

        Statement::Let { .. } | Statement::Assign { .. } | Statement::PeripheralWrite { .. } => {}
    }

    Ok(())
}
