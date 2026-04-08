use crate::frontend::ast::{self, TypeStateSet, TypeParam, BoundKind};
use crate::ir::cfg::{CFG, Statement, Terminator};
use std::collections::{HashMap, HashSet};
use std::fmt;

// Σ : Peripheral → State
pub type StateEnv = HashMap<String, TypeStateSet>;
pub type AliasMap = HashMap<String, Vec<TypeStateSet>>;

#[derive(Debug)]
pub enum TypestateError {
    InvalidTransition {
        func_name: String,
        called_from: String,
        peripheral: String,
        candidate_states: Vec<TypeStateSet>,
        actual_state: TypeStateSet,
    },

    BoundViolation {
        func_name: String,
        called_from: String,
        param_name: String,
        bound_name: String,
        actual_state: TypeStateSet,
    },

    BranchStateMismatch {
        func_name: String,
        peripheral: String,
        then_state: TypeStateSet,
        else_state: TypeStateSet,
    },

    LoopChangesState {
        func_name: String,
        peripheral: String,
        before: TypeStateSet,
        after: TypeStateSet,
    },

    WrongExitState {
        func_name: String,
        peripheral: String,
        expected: TypeStateSet,
        actual: TypeStateSet,
    },

    UnknownPeripheral {
        func_name: String,
        name: String,
    },
}

fn fmt_typestate_set(s: &TypeStateSet) -> String {
    s.iter().cloned().collect::<Vec<_>>().join(" & ")
}

fn fmt_typestate_set_vec(e: &[TypeStateSet]) -> String {
    e.iter()
        .map(|s| if s.len() > 1 { format!("({})", fmt_typestate_set(s)) } else { fmt_typestate_set(s) })
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

            TypestateError::BoundViolation { func_name, called_from, param_name, bound_name, actual_state } => {
                write!(
                    f,
                    "Call to '{}': type parameter '{}' must satisfy bound '{}', but state is '{}' (called from '{}')",
                    func_name,
                    param_name,
                    bound_name,
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

fn build_alias_map(program: &ast::Program) -> AliasMap {
    let mut map = AliasMap::new();
    for p in &program.peripherals {
        for alias in &p.aliases {
            map.insert(alias.name.clone(), alias.definition.clone());
        }
    }
    map
}

fn is_type_var(label: &str, type_params: &[TypeParam]) -> bool {
    type_params.iter().any(|p| p.name == label)
}

fn check_alternative(
    current: &TypeStateSet,
    alt: &TypeStateSet,
    type_params: &[TypeParam],
    alias_map: &AliasMap,
) -> bool {
    for label in alt {
        if label.starts_with('!') {
            let negation = &label[1..];
            if current.contains(negation) {
                return false;
            }
        } else if is_type_var(label, type_params) {
            // Type variable is always satisfied
        } else if let Some(alias_def) = alias_map.get(label) {
            if !state_satisfies_inner(current, alias_def, type_params, alias_map) {
                return false;
            }
        } else {
            if !current.contains(label) {
                return false;
            }
        }
    }
    true
}

fn state_satisfies_inner(
    current: &TypeStateSet,
    candidates: &[TypeStateSet],
    type_params: &[TypeParam],
    alias_map: &AliasMap,
) -> bool {
    candidates.iter().any(|alt| check_alternative(current, alt, type_params, alias_map))
}

fn state_satisfies(
    current: &TypeStateSet,
    candidates: &[TypeStateSet],
    alias_map: &AliasMap,
) -> bool {
    state_satisfies_inner(current, candidates, &[], alias_map)
}

fn compute_parametric_output(
    current: &TypeStateSet,
    output_template: &TypeStateSet,
    type_params: &[TypeParam],
) -> TypeStateSet {
    let mut result = TypeStateSet::new();
    let mut removals = TypeStateSet::new();

    for label in output_template {
        if is_type_var(label, type_params) {
            result.extend(current.clone());
        } else if label.starts_with('!') {
            removals.insert(label[1..].to_string());
        } else {
            result.insert(label.clone());
        }
    }

    for r in &removals {
        result.remove(r);
    }
    result
}

fn check_parametric_input(
    current: &TypeStateSet,
    input_template: &TypeStateSet,
    type_params: &[TypeParam],
    alias_map: &AliasMap,
) -> bool {
    for label in input_template {
        if is_type_var(label, type_params) {
            continue;
        }
        if label.starts_with('!') {
            let negation = &label[1..];
            if current.contains(negation) {
                return false;
            }
        } else if let Some(alias_def) = alias_map.get(label) {
            if !state_satisfies_inner(current, alias_def, type_params, alias_map) {
                return false;
            }
        } else if !current.contains(label) {
            return false;
        }
    }
    true
}

fn check_as_bound(current: &TypeStateSet, bound_def: &[TypeStateSet]) -> bool {
    bound_def.iter().any(|alt| {
        let positives: TypeStateSet = alt.iter()
            .filter(|l| !l.starts_with('!'))
            .cloned()
            .collect();
        let negatives: Vec<&str> = alt.iter()
            .filter(|l| l.starts_with('!'))
            .map(|l| &l[1..])
            .collect();
        *current == positives
            && negatives.iter().all(|n| !current.contains(*n))
    })
}

fn check_includes_bound(current: &TypeStateSet, bound_def: &[TypeStateSet], alias_map: &AliasMap) -> bool {
    state_satisfies_inner(current, bound_def, &[], alias_map)
}

fn check_bounds(
    current: &TypeStateSet,
    type_params: &[TypeParam],
    alias_map: &AliasMap,
) -> Option<(String, String)> {
    for tp in type_params {
        if tp.bound.is_empty() {
            continue;
        }
        let resolved: Vec<TypeStateSet> = if let Some(def) = alias_map.get(&tp.bound) {
            def.clone()
        } else {
            vec![std::iter::once(tp.bound.clone()).collect()]
        };

        let satisfied = match tp.kind {
            BoundKind::As       => check_as_bound(current, &resolved),
            BoundKind::Includes => check_includes_bound(current, &resolved, alias_map),
        };
        if !satisfied {
            return Some((tp.name.clone(), tp.bound.clone()));
        }
    }
    None
}

pub fn check(program: &ast::Program, ir: &[(String, CFG)]) -> Result<(), String> {
    let signatures = build_signature_map(program);
    let alias_map = build_alias_map(program);

    for (i, (_, cfg)) in ir.iter().enumerate() {
        let func = &program.functions[i];
        if let Err(err) = verify_function(func, cfg, &program.peripherals, &signatures, &alias_map) {
            return Err(format!("{}", err));
        }
    }

    Ok(())
}

fn build_signature_map(program: &ast::Program) -> HashMap<String, ast::TypeState> {
    program.functions.iter()
        .filter_map(|f| f.signature.as_ref().map(|sig| (f.name.clone(), sig.clone())))
        .collect()
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

fn classify(func: &ast::Function, cfg: &CFG, _sigs: &HashMap<String, ast::TypeState>) -> FunctionType {
    let has_sig = func.signature.is_some();
    let calls_drivers = cfg.blocks.iter().any(|b| b.statements.iter().any(|s| {
        matches!(s, Statement::PeripheralDriverCall { .. })
    }));
    match (has_sig, calls_drivers) {
        (true, false) => FunctionType::Leaf,
        (true, true)  => FunctionType::Composite,
        (false, _)    => FunctionType::Orchestration,
    }
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
    alias_map: &AliasMap,
) -> Result<(), TypestateError> {
    let kind = classify(func, cfg, signatures);
    let fn_name = &func.name;

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
                let mut env = init_state_env(peripherals);
                env.insert(sig.peripheral.clone(), input_set.clone());
                verify_cfg(cfg, &mut env, signatures, alias_map, fn_name)?;

                let actual = env
                .get(&sig.peripheral)
                    .ok_or_else(|| TypestateError::UnknownPeripheral {
                        func_name: fn_name.clone(),
                        name: sig.peripheral.clone(),
                    })?;

                let expected = &sig.output_state;
                let output_ok = if expected.len() == 1 {
                    let label = expected.iter().next().unwrap();
                    if let Some(alias_def) = alias_map.get(label) {
                        check_as_bound(actual, alias_def)
                    } else {
                        actual == expected
                    }
                } else {
                    actual == expected
                };

                if !output_ok {
                    return Err(TypestateError::WrongExitState {
                        func_name: fn_name.clone(),
                        peripheral: sig.peripheral.clone(),
                        expected: expected.clone(),
                        actual: actual.clone(),
                    });
                }
            }

            Ok(())
        }

        // Orchestration: no declared signature, just verify all transitions are valid
        FunctionType::Orchestration => {
            let mut env = init_state_env(peripherals);
            verify_cfg(cfg, &mut env, signatures, alias_map, fn_name)?;
            Ok(())
        }
    }
}

fn verify_cfg(
    cfg: &CFG,
    state_env: &mut StateEnv,
    signatures: &HashMap<String, ast::TypeState>,
    alias_map: &AliasMap,
    func_name: &str,
) -> Result<(), TypestateError> {
    let mut visited = HashSet::new();
    let mut snapshots: HashMap<usize, StateEnv> = HashMap::new();
    verify_block(cfg, cfg.entry, state_env, signatures, alias_map, &mut visited, func_name, &mut snapshots)
}

fn verify_block(
    cfg: &CFG,
    block_id: usize,
    state_env: &mut StateEnv,
    signatures: &HashMap<String, ast::TypeState>,
    alias_map: &AliasMap,
    visited: &mut HashSet<usize>,
    func_name: &str,
    snapshots: &mut HashMap<usize, StateEnv>,
) -> Result<(), TypestateError> {
    if visited.contains(&block_id) {
        return Ok(());
    }
    visited.insert(block_id);
    snapshots.insert(block_id, state_env.clone());

    let block = cfg.block(block_id);

    for stmt in &block.statements {
        verify_stmt(stmt, state_env, alias_map, func_name)?;
    }

    match &block.terminator {
        Terminator::Jump(target) => {
            if visited.contains(target) {
                if let Some(entry) = snapshots.get(target) {
                    for (p, before) in entry {
                        if let Some(after) = state_env.get(p) {
                            if before != after {
                                return Err(TypestateError::LoopChangesState {
                                    func_name: func_name.to_string(),
                                    peripheral: p.clone(),
                                    before: before.clone(),
                                    after: after.clone(),
                                });
                            }
                        }
                    }
                }
            } else {
                verify_block(cfg, *target, state_env, signatures, alias_map, visited, func_name, snapshots)?;
            }
        }

        Terminator::Branch { then_block, else_block, .. } |
        Terminator::CondBranch { then_block, else_block, .. } => {
            let mut then_env = state_env.clone();
            let mut else_env = state_env.clone();
            verify_block(cfg, *then_block, &mut then_env, signatures, alias_map, &mut visited.clone(), func_name, &mut snapshots.clone())?;
            verify_block(cfg, *else_block, &mut else_env, signatures, alias_map, &mut visited.clone(), func_name, &mut snapshots.clone())?;

            for (p, then_state) in &then_env {
                if let Some(else_state) = else_env.get(p) {
                    if then_state != else_state {
                        return Err(TypestateError::BranchStateMismatch {
                            func_name: func_name.to_string(),
                            peripheral: p.clone(),
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

fn expand_output(output: &TypeStateSet, alias_map: &AliasMap) -> TypeStateSet {
    let mut result = TypeStateSet::new();
    let mut removals = TypeStateSet::new();
    for label in output {
        if let Some(def) = alias_map.get(label) {
            if let Some(alt) = def.first() {
                for l in alt {
                    if l.starts_with('!') {
                        removals.insert(l[1..].to_string());
                    } else {
                        result.insert(l.clone());
                    }
                }
            }
        } else {
            result.insert(label.clone());
        }
    }
    for r in &removals {
        result.remove(r);
    }
    result
}

/* Verify a single statement's effect on the state environment
 *
 * Typing rule for driver calls:
 *
 *   ∃ s ∈ sig(f).input_states : s ⊆ Σ(P)    sig(f).output_state = S_out
 *   ──────────────────────────────────────────────────────────────────────── (driver-call)
 *                    Σ ⊢ f() : Σ[P ↦ S_out]
 */
fn verify_stmt(
    stmt: &Statement,
    state_env: &mut StateEnv,
    alias_map: &AliasMap,
    func_name: &str,
) -> Result<(), TypestateError> {
    match stmt {
        Statement::PeripheralDriverCall { function, peripheral, type_params, from_states, to_state } => {
            let current = state_env
                .get(peripheral)
                .ok_or_else(|| TypestateError::UnknownPeripheral {
                    func_name: func_name.to_string(),
                    name: peripheral.clone(),
                })?;

            let is_parametric = !type_params.is_empty();

            if is_parametric {
                for alt in from_states {
                    if !check_parametric_input(&current, alt, type_params, alias_map) {
                        return Err(TypestateError::InvalidTransition {
                            func_name: function.clone(),
                            called_from: func_name.to_string(),
                            peripheral: peripheral.clone(),
                            candidate_states: from_states.clone(),
                            actual_state: current.clone(),
                        });
                    }
                }
                if let Some((param_name, bound_name)) = check_bounds(&current, type_params, alias_map) {
                    return Err(TypestateError::BoundViolation {
                        func_name: function.clone(),
                        called_from: func_name.to_string(),
                        param_name,
                        bound_name,
                        actual_state: current.clone(),
                    });
                }
                let new_state = compute_parametric_output(&current, to_state, type_params);
                state_env.insert(peripheral.clone(), new_state);
            } else {
                if !state_satisfies(&current, from_states, alias_map) {
                    return Err(TypestateError::InvalidTransition {
                        func_name: function.clone(),
                        called_from: func_name.to_string(),
                        peripheral: peripheral.clone(),
                        candidate_states: from_states.clone(),
                        actual_state: current.clone(),
                    });
                }
                state_env.insert(peripheral.clone(), expand_output(to_state, alias_map));
            }
        }

        Statement::Expr { expr } => {
            let _ = expr;
        }

        Statement::Let { .. } | Statement::Assign { .. } | Statement::PeripheralWrite { .. } => {}
    }

    Ok(())
}
