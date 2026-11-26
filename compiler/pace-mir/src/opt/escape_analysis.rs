use std::collections::{HashMap, HashSet};

use pace_hir::HirId;
use pace_ty::Ty;

use crate::mir::{Local, Lvalue, MirBody, MirFunction, MirProgram, Rvalue, Statement, Terminator};

pub struct EscapeAnalyzer;

impl EscapeAnalyzer {
    pub fn new() -> Self {
        Self
    }

    pub fn optimize_program(&self, program: &mut MirProgram) {
        let trivial_classes = Self::find_trivial_classes(program);
        if trivial_classes.is_empty() {
            return;
        }

        let summaries = Self::compute_param_summaries(program);

        for func in &mut program.functions {
            Self::optimize_body(&mut func.body, &trivial_classes, &summaries, program.class_vtables.values());
        }
        Self::optimize_body(&mut program.main_body, &trivial_classes, &summaries, program.class_vtables.values());
    }

    fn find_trivial_classes(program: &MirProgram) -> HashSet<HirId> {
        let mut custom_deinits: HashSet<String> = HashSet::new();
        for f in &program.functions {
            if f.name.ends_with("_deinit") || f.name == "deinit" {
                custom_deinits.insert(f.name.clone());
            }
        }

        let mut trivial = HashSet::new();
        for (id, fields) in &program.class_defs {
            let has_ref_field = fields.iter().any(|f| !matches!(f.ty, Ty::Int | Ty::Float | Ty::Bool));
            if has_ref_field {
                continue;
            }
            let has_custom = program
                .class_vtables
                .get(id)
                .is_some_and(|vt| vt.iter().any(|e| custom_deinits.contains(&e.mangled_name)));
            if !has_custom {
                trivial.insert(*id);
            }
        }
        trivial
    }

    fn compute_param_summaries(program: &MirProgram) -> HashMap<String, Vec<bool>> {
        // true = parameter does NOT escape
        let mut summaries: HashMap<String, Vec<bool>> = HashMap::new();
        let fn_map: HashMap<String, &MirFunction> = program
            .functions
            .iter()
            .map(|f| (f.name.clone(), f))
            .collect();

        for f in &program.functions {
            summaries.insert(f.name.clone(), vec![!f.is_async; f.params.len()]);
        }

        // Fixed-point iteration
        loop {
            let mut changed = false;
            for f in &program.functions {
                if f.is_async {
                    continue;
                }
                let mut new_safe = Vec::with_capacity(f.params.len());
                for &param_local in &f.params {
                    let safe = !Self::local_escapes_in_body(
                        &f.body,
                        param_local,
                        f.name.ends_with("_init") && param_local == f.params[0],
                        &summaries,
                        &fn_map,
                        program.class_vtables.values(),
                    );
                    new_safe.push(safe);
                }
                if summaries.get(&f.name) != Some(&new_safe) {
                    summaries.insert(f.name.clone(), new_safe);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        summaries
    }

    fn local_escapes_in_body<'a>(
        body: &MirBody,
        root: Local,
        allow_self_return: bool,
        summaries: &HashMap<String, Vec<bool>>,
        fn_map: &HashMap<String, &MirFunction>,
        vtables: impl Iterator<Item = &'a Vec<pace_ty::VTableEntry>> + Clone,
    ) -> bool {
        let mut aliases = HashSet::new();
        aliases.insert(root);

        // Collect transitive aliases via Use
        loop {
            let mut added = false;
            for block in &body.blocks {
                for stmt in &block.statements {
                    if let Statement::Assign(Lvalue::Local(dst), Rvalue::Use(src)) = stmt
                        && aliases.contains(src)
                        && aliases.insert(*dst)
                    {
                        added = true;
                    }
                }
            }
            if !added {
                break;
            }
        }

        for block in &body.blocks {
            for stmt in &block.statements {
                match stmt {
                    Statement::GlobalWrite(_, l) => {
                        if aliases.contains(l) {
                            return true;
                        }
                    }
                    Statement::Assign(lval, rval) => {
                        if matches!(lval, Lvalue::FieldAccess(_, _) | Lvalue::EnumFieldAccess(_, _, _)) {
                            if Self::rvalue_uses_any(rval, &aliases) {
                                return true;
                            }
                        }
                        match rval {
                            Rvalue::Use(_) | Rvalue::FieldAccess(_, _) | Rvalue::OptionalFieldAccess(_, _) => {}
                            Rvalue::GlobalCall(name, args) => {
                                if fn_map.get(name).is_none_or(|f| f.is_async) {
                                    if args.iter().any(|a| aliases.contains(a)) {
                                        return true;
                                    }
                                } else if let Some(param_safe) = summaries.get(name) {
                                    for (idx, a) in args.iter().enumerate() {
                                        if aliases.contains(a) && !param_safe.get(idx).copied().unwrap_or(false) {
                                            return true;
                                        }
                                    }
                                } else if args.iter().any(|a| aliases.contains(a)) {
                                    return true;
                                }
                            }
                            Rvalue::VirtualCall(vidx, obj, args) => {
                                // Check all vtable entries at vidx across classes
                                let mut obj_safe = true;
                                let mut arg_safe = vec![true; args.len()];
                                let mut found_any = false;
                                for vt in vtables.clone() {
                                    if let Some(entry) = vt.get(*vidx) {
                                        found_any = true;
                                        if fn_map.get(&entry.mangled_name).is_none_or(|f| f.is_async) {
                                            obj_safe = false;
                                            arg_safe.fill(false);
                                            break;
                                        }
                                        if let Some(ps) = summaries.get(&entry.mangled_name) {
                                            if !ps.first().copied().unwrap_or(false) {
                                                obj_safe = false;
                                            }
                                            for (i, s) in arg_safe.iter_mut().enumerate() {
                                                if !ps.get(i + 1).copied().unwrap_or(false) {
                                                    *s = false;
                                                }
                                            }
                                        } else {
                                            obj_safe = false;
                                            arg_safe.fill(false);
                                        }
                                    }
                                }
                                if !found_any {
                                    obj_safe = false;
                                    arg_safe.fill(false);
                                }
                                if aliases.contains(obj) && !obj_safe {
                                    return true;
                                }
                                for (i, a) in args.iter().enumerate() {
                                    if aliases.contains(a) && !arg_safe[i] {
                                        return true;
                                    }
                                }
                            }
                            other => {
                                if Self::rvalue_uses_any(other, &aliases) {
                                    return true;
                                }
                            }
                        }
                    }
                    Statement::Retain(_) | Statement::Release(_) => {}
                }
            }
            if !allow_self_return
                && let Some(Terminator::Return(ret_l)) = &block.terminator
                && aliases.contains(ret_l)
            {
                return true;
            }
        }

        false
    }

    fn rvalue_uses_any(rval: &Rvalue, set: &HashSet<Local>) -> bool {
        match rval {
            Rvalue::Use(l)
            | Rvalue::FieldAccess(l, _)
            | Rvalue::OptionalFieldAccess(l, _)
            | Rvalue::EnumFieldAccess(l, _, _)
            | Rvalue::EnumTag(l)
            | Rvalue::ResultIsOk(l)
            | Rvalue::ResultOk(l)
            | Rvalue::ResultPropagateErr(l)
            | Rvalue::OptionIsSome(l)
            | Rvalue::UnaryOp(_, l)
            | Rvalue::ListNew(_, l) => set.contains(l),
            Rvalue::BinaryOp(_, a, b)
            | Rvalue::ListGet(a, b)
            | Rvalue::MapGet(a, b) => set.contains(a) || set.contains(b),
            Rvalue::ListSet(a, b, c) | Rvalue::MapSet(a, b, c) => {
                set.contains(a) || set.contains(b) || set.contains(c)
            }
            Rvalue::Call(f, args) | Rvalue::VirtualCall(_, f, args) => {
                set.contains(f) || args.iter().any(|a| set.contains(a))
            }
            Rvalue::BuiltinCall(_, args)
            | Rvalue::GlobalCall(_, args)
            | Rvalue::Instantiate(_, args)
            | Rvalue::StackInstantiate(_, args)
            | Rvalue::InstantiateEnum(_, _, args)
            | Rvalue::MakeClosure(_, args) => args.iter().any(|a| set.contains(a)),
            Rvalue::Constant(_) | Rvalue::GlobalRead(_) | Rvalue::MapNew(_, _) | Rvalue::SetNew(_) => false,
        }
    }

    fn optimize_body<'a>(
        body: &mut MirBody,
        trivial_classes: &HashSet<HirId>,
        summaries: &HashMap<String, Vec<bool>>,
        vtables: impl Iterator<Item = &'a Vec<pace_ty::VTableEntry>> + Clone,
    ) {
        let mut def_counts: HashMap<Local, usize> = HashMap::new();
        let mut candidates: Vec<(usize, usize, Local, HirId)> = Vec::new();

        for (bi, block) in body.blocks.iter().enumerate() {
            for (si, stmt) in block.statements.iter().enumerate() {
                if let Statement::Assign(Lvalue::Local(dst), rval) = stmt {
                    *def_counts.entry(*dst).or_insert(0) += 1;
                    if let Rvalue::Instantiate(Ty::Class(cid), _) = rval
                        && trivial_classes.contains(cid)
                    {
                        candidates.push((bi, si, *dst, *cid));
                    }
                }
            }
        }

        let mut stack_locals: HashSet<Local> = HashSet::new();
        let mut stack_inst_sites: HashSet<(usize, usize)> = HashSet::new();
        let fn_map: HashMap<String, &MirFunction> = HashMap::new();

        for (bi, si, cand, _cid) in candidates {
            if def_counts.get(&cand).copied().unwrap_or(0) != 1 {
                continue;
            }
            // Collect aliases in the same basic block only (ensures single-block / loop-safe stack lifetime)
            let mut aliases = HashSet::new();
            aliases.insert(cand);
            loop {
                let mut added = false;
                for stmt in &body.blocks[bi].statements {
                    match stmt {
                        Statement::Assign(Lvalue::Local(dst), Rvalue::Use(src))
                            if aliases.contains(src) && def_counts.get(dst).copied().unwrap_or(0) == 1 =>
                        {
                            if aliases.insert(*dst) {
                                added = true;
                            }
                        }
                        Statement::Assign(Lvalue::Local(dst), Rvalue::GlobalCall(name, args))
                            if name.ends_with("_init")
                                && args.first().is_some_and(|a| aliases.contains(a))
                                && def_counts.get(dst).copied().unwrap_or(0) == 1 =>
                        {
                            if aliases.insert(*dst) {
                                added = true;
                            }
                        }
                        _ => {}
                    }
                }
                if !added {
                    break;
                }
            }

            // Ensure no alias is used outside block `bi`
            let mut used_outside = false;
            for (obi, block) in body.blocks.iter().enumerate() {
                if obi == bi {
                    continue;
                }
                for stmt in &block.statements {
                    let hits = match stmt {
                        Statement::Assign(lval, rval) => {
                            let lhits = match lval {
                                Lvalue::Local(l) | Lvalue::FieldAccess(l, _) | Lvalue::EnumFieldAccess(l, _, _) => {
                                    aliases.contains(l)
                                }
                            };
                            lhits || Self::rvalue_uses_any(rval, &aliases)
                        }
                        Statement::GlobalWrite(_, l) => aliases.contains(l),
                        Statement::Retain(_) | Statement::Release(_) => false,
                    };
                    if hits {
                        used_outside = true;
                        break;
                    }
                }
                if let Some(term) = &block.terminator {
                    let thits = match term {
                        Terminator::Return(l) => aliases.contains(l),
                        Terminator::Branch { cond, .. } => aliases.contains(cond),
                        Terminator::Goto(_) => false,
                    };
                    if thits {
                        used_outside = true;
                    }
                }
                if used_outside {
                    break;
                }
            }
            if used_outside {
                continue;
            }

            // Check escape in body using summaries (note: for GlobalCall, check summaries directly)
            let mut escapes = false;
            for stmt in &body.blocks[bi].statements {
                match stmt {
                    Statement::GlobalWrite(_, l) if aliases.contains(l) => {
                        escapes = true;
                        break;
                    }
                    Statement::Assign(lval, rval) => {
                        if matches!(lval, Lvalue::FieldAccess(_, _) | Lvalue::EnumFieldAccess(_, _, _))
                            && Self::rvalue_uses_any(rval, &aliases)
                        {
                            escapes = true;
                            break;
                        }
                        match rval {
                            Rvalue::Use(_) | Rvalue::FieldAccess(_, _) | Rvalue::OptionalFieldAccess(_, _) => {}
                            Rvalue::Instantiate(Ty::Class(_), _) => {}
                            Rvalue::GlobalCall(name, args) => {
                                if let Some(ps) = summaries.get(name) {
                                    for (idx, a) in args.iter().enumerate() {
                                        if aliases.contains(a) && !ps.get(idx).copied().unwrap_or(false) {
                                            escapes = true;
                                            break;
                                        }
                                    }
                                } else if args.iter().any(|a| aliases.contains(a)) {
                                    escapes = true;
                                }
                            }
                            Rvalue::VirtualCall(vidx, obj, args) => {
                                let mut obj_safe = true;
                                let mut arg_safe = vec![true; args.len()];
                                let mut found_any = false;
                                for vt in vtables.clone() {
                                    if let Some(entry) = vt.get(*vidx) {
                                        found_any = true;
                                        if let Some(ps) = summaries.get(&entry.mangled_name) {
                                            if !ps.first().copied().unwrap_or(false) {
                                                obj_safe = false;
                                            }
                                            for (i, s) in arg_safe.iter_mut().enumerate() {
                                                if !ps.get(i + 1).copied().unwrap_or(false) {
                                                    *s = false;
                                                }
                                            }
                                        } else {
                                            obj_safe = false;
                                            arg_safe.fill(false);
                                        }
                                    }
                                }
                                if !found_any {
                                    obj_safe = false;
                                    arg_safe.fill(false);
                                }
                                if aliases.contains(obj) && !obj_safe {
                                    escapes = true;
                                }
                                for (i, a) in args.iter().enumerate() {
                                    if aliases.contains(a) && !arg_safe[i] {
                                        escapes = true;
                                    }
                                }
                            }
                            other => {
                                if Self::rvalue_uses_any(other, &aliases) {
                                    escapes = true;
                                }
                            }
                        }
                        if escapes {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(Terminator::Return(l)) = &body.blocks[bi].terminator
                && aliases.contains(l)
            {
                escapes = true;
            }
            let _ = &fn_map;

            if !escapes {
                stack_inst_sites.insert((bi, si));
                stack_locals.extend(aliases);
            }
        }

        // Apply StackInstantiate and elide Retain/Release on stack_locals
        for (bi, si) in stack_inst_sites {
            if let Statement::Assign(_, rval) = &mut body.blocks[bi].statements[si] {
                if let Rvalue::Instantiate(ty, fields) = std::mem::replace(
                    rval,
                    Rvalue::Constant(crate::mir::Constant::Int("0".to_string())),
                ) {
                    *rval = Rvalue::StackInstantiate(ty, fields);
                }
            }
        }

        if !stack_locals.is_empty() {
            for block in &mut body.blocks {
                block.statements.retain(|stmt| match stmt {
                    Statement::Retain(Lvalue::Local(l)) | Statement::Release(Lvalue::Local(l)) => {
                        !stack_locals.contains(l)
                    }
                    _ => true,
                });
            }
        }
    }
}
