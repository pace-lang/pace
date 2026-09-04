pub mod constant_fold;
pub mod dce;
pub mod escape;
pub mod inline;

use crate::MirProgram;

/// Run optimization passes on the MIR program.
pub fn optimize(program: &mut MirProgram) {
    inline::optimize(program);
    
    let mut any_changed = true;
    let mut pass_iterations = 0;
    
    while any_changed && pass_iterations < 5 {
        any_changed = false;
        
        for (_, body) in program.functions.iter_mut() {
            if body.is_extern {
            continue;
        }
        
        let mut changed = true;
        let mut iterations = 0;
        let max_iterations = 10;
        
        while changed && iterations < max_iterations {
            changed = false;
            
            // 1. Constant Folding & Propagation
            changed |= constant_fold::optimize(body);
            
            // 2. Escape Analysis
            changed |= escape::optimize(body);
            
            // 3. Dead Code Elimination
            changed |= dce::optimize(body);
            
            iterations += 1;
        }
    }
    
    pass_iterations += 1;
    }
}
