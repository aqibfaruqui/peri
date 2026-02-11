use crate::frontend::ast;
use crate::ir::cfg::{CFG, Statement, Expr, Terminator};
use std::collections::HashMap;
use std::collections::HashSet;

// Σ : Peripheral → State
pub type StateEnv = HashMap<String, String>;

#[derive(Debug)]
pub enum TypeError {
    // Failed premise in typing derivation
    InvalidTransition {
        func_name: String,
        peripheral: String,
        expected_state: String,
        actual_state: String,
    },
    
    // Violates the Branch typing rule: Σ ⊢ then : Σ₁ and Σ ⊢ else : Σ₂ requires Σ₁ = Σ₂
    BranchStateMismatch {
        peripheral: String,
        then_state: String,
        else_state: String,
    },
    
    // Violates the While rule: Σ ⊢ body : Σ' requires Σ = Σ'
    LoopChangesState {
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
        name: String,
    },
}

#[derive(Debug, PartialEq)]
enum FunctionKind {
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

pub fn build_signature_map(program: &ast::Program) -> HashMap<String, ast::TypeState> {
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
) -> FunctionKind {
    let has_signature = func.signature.is_some();
    let calls_drivers = cfg_calls_drivers(cfg, signatures);
    
    match (has_signature, calls_drivers) {
        (true, false)  => FunctionKind::LeafDriver,
        (true, true)   => FunctionKind::CompositeDriver,
        (false, _)     => FunctionKind::Orchestration,
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
pub fn verify_function(
    func: &ast::Function,
    cfg: &CFG,
    peripherals: &[ast::Peripheral],
    signatures: &HashMap<String, ast::TypeState>,
) -> Result<(), TypeError> {
    let kind = classify_function(func, cfg, signatures);
    
    match kind {
        /* Axiom: trusted, no verification needed
         *
         *   ────────────────────────────────── (axiom)
         *   Σ ⊢ leaf_driver() : Σ[P ↦ S_out]
         */
        FunctionKind::LeafDriver => Ok(()),
        
        /* Derive: verify body composes correctly, then check against declared signature
         *
         *   Σ₀ ⊢ s₁ : Σ₁    Σ₁ ⊢ s₂ : Σ₂    ...    Σₙ₋₁ ⊢ sₙ : Σₙ
         *   ──────────────────────────────────────────────────────────── (seq)
         *                    Σ₀ ⊢ body : Σₙ
         *
         *   Then check: Σₙ(P) = declared output state
         */
        FunctionKind::CompositeDriver => {
            let sig = func.signature.as_ref().unwrap();
            
            // Start with the declared input state
            let mut state_env = init_state_env(peripherals);
            state_env.insert(sig.peripheral.clone(), sig.input_state.clone());
            
            // Derive the output state by composing driver calls in the body
            verify_cfg(cfg, &mut state_env, signatures)?;
            
            // Check derived output matches declared output
            let actual = state_env.get(&sig.peripheral)
                .ok_or_else(|| TypeError::UnknownPeripheral { name: sig.peripheral.clone() })?;
            
            if actual != &sig.output_state {
                return Err(TypeError::WrongExitState {
                    func_name: func.name.clone(),
                    peripheral: sig.peripheral.clone(),
                    expected: sig.output_state.clone(),
                    actual: actual.clone(),
                });
            }
            
            Ok(())
        }
        
        // Orchestration: no declared signature, just verify all transitions are valid
        FunctionKind::Orchestration => {
            let mut state_env = init_state_env(peripherals);
            verify_cfg(cfg, &mut state_env, signatures)?;
            Ok(())
        }
    }
}

fn verify_cfg(
    cfg: &CFG,
    state_env: &mut StateEnv,
    signatures: &HashMap<String, ast::TypeState>,
) -> Result<(), TypeError> {
    let mut visited = HashSet::new();
    verify_block_recursive(cfg, cfg.entry, state_env, signatures, &mut visited)
}

fn verify_block_recursive(
    cfg: &CFG,
    block_id: usize,
    state_env: &mut StateEnv,
    signatures: &HashMap<String, ast::TypeState>,
    visited: &mut HashSet<usize>,
) -> Result<(), TypeError> {
    if visited.contains(&block_id) {
        return Ok(());
    }
    visited.insert(block_id);
    
    let block = cfg.block(block_id);
    
    for stmt in &block.statements {
        verify_statement(stmt, state_env, signatures)?;
    }
    
    match &block.terminator {
        Terminator::Jump(target) => {
            verify_block_recursive(cfg, *target, state_env, signatures, visited)?;
        }
        
        Terminator::Branch { cond: _, then_block, else_block } => {
            let mut then_env = state_env.clone();
            let mut else_env = state_env.clone();
            
            verify_block_recursive(cfg, *then_block, &mut then_env, signatures, &mut visited.clone())?;
            verify_block_recursive(cfg, *else_block, &mut else_env, signatures, &mut visited.clone())?;
            
            for (peripheral, then_state) in &then_env {
                if let Some(else_state) = else_env.get(peripheral) {
                    if then_state != else_state {
                        return Err(TypeError::BranchStateMismatch {
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
            verify_block_recursive(cfg, *target, state_env, signatures, visited)?;
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
) -> Result<(), TypeError> {
    match stmt {
        Statement::PeripheralDriverCall { func_name, peripheral, from_state, to_state } => {
            let current = state_env.get(peripheral)
                .ok_or_else(|| TypeError::UnknownPeripheral { name: peripheral.clone() })?;
            
            if current != from_state {
                return Err(TypeError::InvalidTransition {
                    func_name: func_name.clone(),
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
                        .ok_or_else(|| TypeError::UnknownPeripheral { name: sig.peripheral.clone() })?;
                    
                    if current != &sig.input_state {
                        return Err(TypeError::InvalidTransition {
                            func_name: name.clone(),
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
