use crate::checker::TypeChecker;

/// A dedicated pass that runs after the initial typechecking phase to
/// iteratively typecheck all generic instantiations that were discovered.
/// This separates instantiation logic from typechecking to avoid recursive stack overflows.
pub struct MonomorphizePass<'a, 'b> {
    typechecker: &'b mut TypeChecker<'a>,
}

impl<'a, 'b> MonomorphizePass<'a, 'b> {
    pub fn new(typechecker: &'b mut TypeChecker<'a>) -> Self {
        Self { typechecker }
    }

    pub fn run(&mut self) {
        // We use a fixed-point iteration loop because typechecking an instantiation
        // might trigger the discovery of MORE instantiations (e.g. MyList_Int might use MyBox_Int).
        loop {
            // Drain the queue to avoid borrow checker issues with `self.typechecker` inside the loop
            let queue: Vec<_> = self.typechecker.instantiation_queue.drain(..).collect();

            if queue.is_empty() {
                break;
            }

            for (_key, concrete_stmt, _substitution, _mangled_name) in queue {
                // Typecheck the concrete (monomorphized) statement
                let typed_stmt = self.typechecker.check_stmt(&concrete_stmt);

                // Add the typed statement to pending_instantiations so it gets emitted to MIR
                self.typechecker.pending_instantiations.push(typed_stmt);
            }
        }
    }
}
