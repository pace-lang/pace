use crate::checker::TypeChecker;
use crate::env::Type;
use std::collections::HashSet;

impl<'a> TypeChecker<'a> {
    pub(crate) fn detect_cycles(&mut self) {
        let mut visited = HashSet::new();
        let mut in_path = HashSet::new();
        let mut cycle_found = HashSet::new();

        // Only checking Classes and Actors since they are heap-allocated reference types.
        let class_names: Vec<ustr::Ustr> = self.env.classes.keys().copied().collect();

        for class_name in class_names {
            if !visited.contains(&class_name) {
                self.dfs_detect_cycle(class_name, &mut visited, &mut in_path, &mut cycle_found);
            }
        }
    }

    fn dfs_detect_cycle(
        &mut self,
        current: ustr::Ustr,
        visited: &mut HashSet<ustr::Ustr>,
        in_path: &mut HashSet<ustr::Ustr>,
        cycle_found: &mut HashSet<ustr::Ustr>,
    ) {
        visited.insert(current);
        in_path.insert(current);

        let fields = if let Some(sig) = self.env.classes.get(&current) {
            sig.fields.clone()
        } else {
            std::collections::HashMap::new()
        };

        for (_, field_ty) in fields {
            // We consider direct references to other classes (or self). 
            // In a real scenario, this could also include Option<Class> or List<Class>.
            // For now, let's just do direct class references.
            let target_class_opt = match field_ty.ty {
                Type::Class(name) => Some(name),
                Type::Nullable(inner) => {
                    if let Type::Class(name) = *inner {
                        Some(name)
                    } else {
                        None
                    }
                }
                _ => None,
            };

            if let Some(target_class) = target_class_opt {
                // If it's a weak reference, we wouldn't follow it.
                // However, 'weak' modifier parsing is in Phase 2. 
                // Once we have weak fields, we can skip them here.

                if in_path.contains(&target_class) {
                    // Cycle detected!
                    if !cycle_found.contains(&target_class) {
                        let span = self.env.classes.get(&target_class).map(|s| s.span).unwrap_or_default();
                        self.warnings.push(pace_errors::SemanticWarning::CycleDetected {
                            class_name: target_class.to_string(),
                            src: self.get_source(),
                            span,
                        });
                        cycle_found.insert(target_class);
                    }
                } else if !visited.contains(&target_class) {
                    self.dfs_detect_cycle(target_class, visited, in_path, cycle_found);
                }
            }
        }

        in_path.remove(&current);
    }
}
