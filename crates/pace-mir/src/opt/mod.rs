pub mod constant_fold;
pub mod dce;

use crate::MirProgram;

/// Run optimization passes on the MIR program.
pub fn optimize(program: &mut MirProgram) {
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
            
            // 2. Dead Code Elimination
            changed |= dce::optimize(body);
            
            iterations += 1;
        }
    }
}
