use std::collections::{HashMap, HashSet};

use pace_hir::HirId;
use pace_ty::Ty;

use crate::mir::{Local, Lvalue, MirBody, MirProgram, Rvalue, Statement, Terminator};

pub struct Devirtualizer;

impl Devirtualizer {
    pub fn new() -> Self {
        Self
    }

    pub fn optimize_program(&self, program: &mut MirProgram) {
        // Count distinct mangled implementations per method name across all vtables.
        // If a method name has only 1 implementation across the entire program, any VirtualCall
        // to that method name can be safely devirtualized to a direct GlobalCall.
        let mut impls_by_method: HashMap<String, HashSet<String>> = HashMap::new();
        for vt in program.class_vtables.values() {
            for entry in vt {
                impls_by_method
                    .entry(entry.name.clone())
                    .or_default()
                    .insert(entry.mangled_name.clone());
            }
        }

        let vtables = &program.class_vtables;
        for func in &mut program.functions {
            Self::devirt_body(&mut func.body, vtables, &impls_by_method);
        }
        Self::devirt_body(&mut program.main_body, vtables, &impls_by_method);
    }

    fn devirt_body(
        body: &mut MirBody,
        vtables: &HashMap<HirId, Vec<pace_ty::VTableEntry>>,
        impls_by_method: &HashMap<String, HashSet<String>>,
    ) {
        // Also track locals whose exact runtime class is statically known from Instantiate/StackInstantiate
        let mut exact_class: HashMap<Local, HirId> = HashMap::new();
        let mut def_counts: HashMap<Local, usize> = HashMap::new();
        for block in &body.blocks {
            for stmt in &block.statements {
                if let Statement::Assign(Lvalue::Local(dst), rval) = stmt {
                    *def_counts.entry(*dst).or_insert(0) += 1;
                    if let Rvalue::Instantiate(Ty::Class(cid), _)
                    | Rvalue::StackInstantiate(Ty::Class(cid), _) = rval
                    {
                        exact_class.insert(*dst, *cid);
                    }
                }
            }
        }
        // Propagate exact_class through single-def Use and *_init calls
        loop {
            let mut changed = false;
            for block in &body.blocks {
                for stmt in &block.statements {
                    match stmt {
                        Statement::Assign(Lvalue::Local(dst), Rvalue::Use(src))
                            if def_counts.get(dst).copied().unwrap_or(0) == 1 =>
                        {
                            if let Some(&cid) = exact_class.get(src)
                                && !exact_class.contains_key(dst)
                            {
                                exact_class.insert(*dst, cid);
                                changed = true;
                            }
                        }
                        Statement::Assign(Lvalue::Local(dst), Rvalue::GlobalCall(name, args))
                            if name.ends_with("_init")
                                && def_counts.get(dst).copied().unwrap_or(0) == 1 =>
                        {
                            if let Some(first) = args.first()
                                && let Some(&cid) = exact_class.get(first)
                                && !exact_class.contains_key(dst)
                            {
                                exact_class.insert(*dst, cid);
                                changed = true;
                            }
                        }
                        _ => {}
                    }
                }
            }
            if !changed {
                break;
            }
        }

        for block in &mut body.blocks {
            for stmt in &mut block.statements {
                if let Statement::Assign(_, rval) = stmt
                    && let Rvalue::VirtualCall(vidx, obj, args) = rval
                {
                    let target_cid = exact_class.get(obj).copied().or_else(|| {
                        if let Ty::Class(cid) = body.locals.get(obj.0 as usize)? {
                            Some(*cid)
                        } else {
                            None
                        }
                    });
                    let Some(cid) = target_cid else { continue };
                    let Some(vt) = vtables.get(&cid) else { continue };
                    let Some(entry) = vt.get(*vidx) else { continue };

                    let is_exact = exact_class.contains_key(obj);
                    let is_monomorphic = impls_by_method
                        .get(&entry.name)
                        .is_some_and(|s| s.len() == 1);

                    if is_exact || is_monomorphic {
                        let mut call_args = Vec::with_capacity(args.len() + 1);
                        call_args.push(*obj);
                        call_args.extend(args.iter().copied());
                        *rval = Rvalue::GlobalCall(entry.mangled_name.clone(), call_args);
                    }
                }
            }
        }
    }
}

pub struct DceOptimizer;

impl DceOptimizer {
    pub fn new() -> Self {
        Self
    }

    pub fn optimize_program(&self, program: &mut MirProgram) {
        for func in &mut program.functions {
            Self::optimize_body(&mut func.body);
        }
        Self::optimize_body(&mut program.main_body);
    }

    fn optimize_body(body: &mut MirBody) {
        // Iterate DCE until convergence
        loop {
            let used = Self::collect_used_locals(body);
            let mut removed = false;
            for block in &mut body.blocks {
                let before = block.statements.len();
                block.statements.retain(|stmt| {
                    if let Statement::Assign(Lvalue::Local(dst), rval) = stmt
                        && !used.contains(dst)
                        && Self::is_pure_rvalue(rval)
                    {
                        return false;
                    }
                    true
                });
                if block.statements.len() != before {
                    removed = true;
                }
            }
            if !removed {
                break;
            }
        }
    }

    fn is_pure_rvalue(rval: &Rvalue) -> bool {
        matches!(
            rval,
            Rvalue::Use(_)
                | Rvalue::Constant(_)
                | Rvalue::BinaryOp(_, _, _)
                | Rvalue::UnaryOp(_, _)
                | Rvalue::FieldAccess(_, _)
                | Rvalue::OptionalFieldAccess(_, _)
                | Rvalue::EnumFieldAccess(_, _, _)
                | Rvalue::EnumTag(_)
                | Rvalue::ResultIsOk(_)
                | Rvalue::ResultOk(_)
                | Rvalue::OptionIsSome(_)
                | Rvalue::GlobalRead(_)
                | Rvalue::Instantiate(_, _)
                | Rvalue::StackInstantiate(_, _)
                | Rvalue::InstantiateEnum(_, _, _)
                | Rvalue::MakeClosure(_, _)
        )
    }

    fn collect_used_locals(body: &MirBody) -> HashSet<Local> {
        let mut used = HashSet::new();
        for block in &body.blocks {
            for stmt in &block.statements {
                match stmt {
                    Statement::Assign(lval, rval) => {
                        match lval {
                            Lvalue::Local(_) => {}
                            Lvalue::FieldAccess(l, _) | Lvalue::EnumFieldAccess(l, _, _) => {
                                used.insert(*l);
                            }
                        }
                        Self::collect_rvalue_uses(rval, &mut used);
                    }
                    Statement::Retain(lval) | Statement::Release(lval) => match lval {
                        Lvalue::Local(l)
                        | Lvalue::FieldAccess(l, _)
                        | Lvalue::EnumFieldAccess(l, _, _) => {
                            used.insert(*l);
                        }
                    },
                    Statement::GlobalWrite(_, l) => {
                        used.insert(*l);
                    }
                    Statement::BoxSet(cell, val) => {
                        used.insert(*cell);
                        used.insert(*val);
                    }
                }
            }
            if let Some(term) = &block.terminator {
                match term {
                    Terminator::Return(l) => {
                        used.insert(*l);
                    }
                    Terminator::Branch { cond, .. } => {
                        used.insert(*cond);
                    }
                    Terminator::Goto(_) => {}
                }
            }
        }
        used
    }

    fn collect_rvalue_uses(rval: &Rvalue, used: &mut HashSet<Local>) {
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
            | Rvalue::ListNew(_, l)
            | Rvalue::BoxNew(l)
            | Rvalue::BoxGet(l, _) => {
                used.insert(*l);
            }
            Rvalue::BinaryOp(_, a, b) | Rvalue::ListGet(a, b) | Rvalue::MapGet(a, b) => {
                used.insert(*a);
                used.insert(*b);
            }
            Rvalue::ListSet(a, b, c) | Rvalue::MapSet(a, b, c) => {
                used.insert(*a);
                used.insert(*b);
                used.insert(*c);
            }
            Rvalue::Call(f, args) | Rvalue::VirtualCall(_, f, args) => {
                used.insert(*f);
                used.extend(args.iter().copied());
            }
            Rvalue::BuiltinCall(_, args)
            | Rvalue::GlobalCall(_, args)
            | Rvalue::Instantiate(_, args)
            | Rvalue::StackInstantiate(_, args)
            | Rvalue::InstantiateEnum(_, _, args)
            | Rvalue::MakeClosure(_, args) => {
                used.extend(args.iter().copied());
            }
            Rvalue::Constant(_)
            | Rvalue::GlobalRead(_)
            | Rvalue::MapNew(_, _)
            | Rvalue::SetNew(_) => {}
        }
    }
}

