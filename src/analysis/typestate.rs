use crate::frontend::ast;
use crate::ir::cfg::{CFG, Statement, Expr, Terminator};
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;

// Σ : Peripheral → State
pub type StateEnv = HashMap<String, String>;

#[derive(Debug)]
pub enum TypestateError {
    // Failed premise in typing derivation
    InvalidTransition {
        func_name: String,
        called_from: String,
        peripheral: String,
        expected_state: String,
        actual_state: String,
    },
    
    // Violates the Branch typing rule: Σ ⊢ then : Σ₁ and Σ ⊢ else : Σ₂ requires Σ₁ = Σ₂
    BranchStateMismatch {
        func_name: String,
        peripheral: String,
        then_state: String,
        else_state: String,
    },
    
    // Violates the While rule: Σ ⊢ body : Σ' requires Σ = Σ'
    LoopChangesState {
        func_name: String,
        peripheral: String,
        before: String,
        after: String,
    },
    
    // Peripheral driver's derived effect != declared signature
    WrongExitState {
        func_name: String,
        peripheral: String,
        expected: String,
        actual: String,
    },
    
    // Unknown peripheral referenced
    UnknownPeripheral {
        func_name: String,
        name: String,
    },
}

impl fmt::Display for TypestateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypestateError::InvalidTransition { func_name, called_from, peripheral, expected_state, actual_state } => {
                write!(
                    f, "Call to '{}' requires '{}' in state '{}', but found '{}', called from '{}'",
                    func_name, peripheral, expected_state, actual_state, called_from
                )
            }

            TypestateError::BranchStateMismatch { func_name, peripheral, then_state, else_state } => {
                write!(
                    f, "Branch in {} leaves '{}' in different states: then = '{}', else = '{}'",
                    func_name, peripheral, then_state, else_state
                )
            }

            TypestateError::LoopChangesState { func_name, peripheral, before, after } => {
                write!(
                    f, "Loop in {} changes state of '{}' from '{}' to '{}'",
                    func_name, peripheral, before, after
                )
            }

            TypestateError::WrongExitState { func_name, peripheral, expected, actual } => {
                write!(
                    f, "Function '{}' declares output state '{}' for '{}', but body produces '{}'",
                    func_name, expected, peripheral, actual
                )
            }

            TypestateError::UnknownPeripheral { func_name,name } => {
                write!(
                    f, "Unknown peripheral '{}' in function '{}'", 
                    name, func_name)
            }
        }
    }
}

#[derive(Debug, PartialEq)]
enum FunctionType {
    // Has a typestate signature but calls no other driver functions
    // Axioms in our type system
    LeafDriver,
    
    // Has a typestate signature and calls other driver functions
    // Derived in our type system by verifying body matches their signature
    CompositeDriver,
    
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
    let mut env = StateEnv::new();
    for p in peripherals {
        env.insert(p.name.clone(), p.initial.clone());
    }
    env
}

fn classify_function(
    func: &ast::Function,
    cfg: &CFG,
    signatures: &HashMap<String, ast::TypeState>,
) -> FunctionType {
    let has_signature = func.signature.is_some();
    let calls_drivers = cfg_calls_drivers(cfg, signatures);
    
    match (has_signature, calls_drivers) {
        (true, false)  => FunctionType::LeafDriver,
        (true, true)   => FunctionType::CompositeDriver,
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
 *   LeafDriver:       Axiom: signature is trusted, no verification needed
 *   CompositeDriver:  Derive: compose called driver signatures, check result matches declaration
 *   Orchestration:    Derive: compose called driver signatures, check all transitions are valid
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
        FunctionType::LeafDriver => Ok(()),
        
        /* Derive: verify body composes correctly, then check against declared signature
         *
         *   Σ₀ ⊢ s₁ : Σ₁    Σ₁ ⊢ s₂ : Σ₂    ...    Σₙ₋₁ ⊢ sₙ : Σₙ
         *   ──────────────────────────────────────────────────────────── (seq)
         *                    Σ₀ ⊢ body : Σₙ
         *
         *   Then check: Σₙ(P) = declared output state
         */
        FunctionType::CompositeDriver => {
            let sig = func.signature.as_ref().unwrap();
            
            // Start with the declared input state
            let mut state_env = init_state_env(peripherals);
            state_env.insert(sig.peripheral.clone(), sig.input_state.clone());
            
            // Derive the output state by composing driver calls in the body
            verify_cfg(cfg, &mut state_env, signatures, func_name)?;
            
            // Check derived output matches declared output
            let actual = state_env.get(&sig.peripheral)
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
    verify_block_recursive(cfg, cfg.entry, state_env, signatures, &mut visited, func_name)
}

fn verify_block_recursive(
    cfg: &CFG,
    block_id: usize,
    state_env: &mut StateEnv,
    signatures: &HashMap<String, ast::TypeState>,
    visited: &mut HashSet<usize>,
    func_name: &str,
) -> Result<(), TypestateError> {
    if visited.contains(&block_id) {
        return Ok(());
    }
    visited.insert(block_id);
    
    let block = cfg.block(block_id);
    
    for stmt in &block.statements {
        verify_statement(stmt, state_env, signatures, func_name)?;
    }
    
    match &block.terminator {
        Terminator::Jump(target) => {
            verify_block_recursive(cfg, *target, state_env, signatures, visited, func_name)?;
        }
        
        Terminator::Branch { cond: _, then_block, else_block } => {
            let mut then_env = state_env.clone();
            let mut else_env = state_env.clone();
            
            verify_block_recursive(cfg, *then_block, &mut then_env, signatures, &mut visited.clone(), func_name)?;
            verify_block_recursive(cfg, *else_block, &mut else_env, signatures, &mut visited.clone(), func_name)?;
            
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
        
        Terminator::Fallthrough(target) => {
            verify_block_recursive(cfg, *target, state_env, signatures, visited, func_name)?;
        }
        
        Terminator::Return(_) | Terminator::None => {
            // End of control flow path
        }
    }
    
    Ok(())
}

/* Verify a single statement's effect on the state environment
 *
 * Typing rule for driver calls:
 *
 *   Σ(P) = S₁     sig(f) = P<S₁> → P<S₂>
 *   ──────────────────────────────────────── (driver-call)
 *           Σ ⊢ f() : Σ[P ↦ S₂]
 */
fn verify_statement(
    stmt: &Statement,
    state_env: &mut StateEnv,
    signatures: &HashMap<String, ast::TypeState>,
    func_name: &str,
) -> Result<(), TypestateError> {
    match stmt {
        Statement::PeripheralDriverCall { function, peripheral, from_state, to_state } => {
            let current = state_env.get(peripheral)
                .ok_or_else(|| TypestateError::UnknownPeripheral {
                    func_name: func_name.to_string(),
                    name: peripheral.clone(),
                })?;
            
            if current != from_state {
                return Err(TypestateError::InvalidTransition {
                    func_name: function.clone(),
                    called_from: func_name.to_string(),
                    peripheral: peripheral.clone(),
                    expected_state: from_state.clone(),
                    actual_state: current.clone(),
                });
            }
            
            state_env.insert(peripheral.clone(), to_state.clone());
        }
        
        Statement::Expr { expr } => {
            if let Expr::FnCall { name, .. } = expr {
                if let Some(sig) = signatures.get(name) {
                    let current = state_env.get(&sig.peripheral)
                        .ok_or_else(|| TypestateError::UnknownPeripheral {
                            func_name: func_name.to_string(),
                            name: sig.peripheral.clone(),
                        })?;
                    
                    if current != &sig.input_state {
                        return Err(TypestateError::InvalidTransition {
                            func_name: name.clone(),
                            called_from: func_name.to_string(),
                            peripheral: sig.peripheral.clone(),
                            expected_state: sig.input_state.clone(),
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
