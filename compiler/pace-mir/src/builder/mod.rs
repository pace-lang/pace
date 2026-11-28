mod expr;
mod program;
mod stmt;

use crate::mir::*;
use pace_hir::HirId;
use pace_ty::{Ty, TypeChecker};
use std::collections::HashMap;

pub(super) fn tc_get_type(tc: &TypeChecker, ty: &pace_ast::Type) -> Option<Ty> {
    if let Ok(t) = tc.get_type(ty) {
        return Some(t);
    }
    match ty {
        pace_ast::Type::Named(id) => {
            let suffix = format!("_{}", id.name);
            for (mangled, &(hir_id, kind)) in &tc.named_types {
                if mangled == id.name || mangled.ends_with(&suffix) {
                    return Some(match kind {
                        1 => Ty::Class(hir_id),
                        0 => Ty::Struct(hir_id),
                        3 => Ty::Actor(hir_id),
                        _ => Ty::Enum(hir_id),
                    });
                }
            }
            None
        }
        pace_ast::Type::Optional(inner, _) => {
            Some(Ty::Optional(Box::new(tc_get_type(tc, inner)?)))
        }
        pace_ast::Type::Generic(base, args, _) => {
            if let pace_ast::Type::Named(id) = base.as_ref() {
                match (id.name.as_str(), args.as_slice()) {
                    ("Option", [a]) => Some(Ty::Optional(Box::new(tc_get_type(tc, a)?))),
                    ("Result", [ok, err]) => Some(Ty::Result(
                        Box::new(tc_get_type(tc, ok)?),
                        Box::new(tc_get_type(tc, err)?),
                    )),
                    ("List", [elem]) => Some(Ty::List(Box::new(tc_get_type(tc, elem)?))),
                    ("Map", [k, v]) => Some(Ty::Map(
                        Box::new(tc_get_type(tc, k)?),
                        Box::new(tc_get_type(tc, v)?),
                    )),
                    ("Set", [elem]) => Some(Ty::Set(Box::new(tc_get_type(tc, elem)?))),
                    _ => None,
                }
            } else {
                None
            }
        }
        pace_ast::Type::Closure(params, ret, _) => {
            let p_tys: Option<Vec<Ty>> = params.iter().map(|p| tc_get_type(tc, p)).collect();
            let r_ty = tc_get_type(tc, ret)?;
            Some(Ty::Function(p_tys?, Box::new(r_ty)))
        }
    }
}


pub struct MirBuilder<'a> {
    pub blocks: Vec<BasicBlock>,
    pub current_block: BasicBlockId,
    pub locals: Vec<Ty>,
    pub hir_to_local: HashMap<HirId, Local>,
    pub global_fns: HashMap<String, String>,
    pub struct_defs: &'a HashMap<HirId, Vec<pace_ty::ResolvedField>>,
    pub class_defs: &'a HashMap<HirId, Vec<pace_ty::ResolvedField>>,
    pub class_vtables: &'a HashMap<HirId, Vec<pace_ty::VTableEntry>>,
    pub enum_defs: &'a HashMap<HirId, Vec<pace_hir::EnumVariant>>,
    pub global_env: &'a HashMap<pace_hir::HirId, Ty>,
    pub local_types: &'a HashMap<pace_hir::HirId, Ty>,
    pub named_types: &'a HashMap<String, (pace_hir::HirId, u8)>,
    pub static_fields_env: &'a HashMap<String, Ty>,
    pub methods_env: &'a HashMap<String, Ty>,
    pub current_expected_ty: Option<Ty>,
    pub current_self_local: Option<Local>,
    pub class_parents: &'a HashMap<HirId, HirId>,
    pub global_functions_env: &'a HashMap<String, Ty>,
    pub global_vars_env: &'a HashMap<String, Ty>,
    pub resolved_global_names: &'a HashMap<pace_hir::HirId, String>,
    pub closure_functions: Vec<MirFunction>,
    pub closure_counter: usize,
    pub loop_stack: Vec<(BasicBlockId, BasicBlockId)>,
    pub scope_stack: Vec<Vec<Local>>,
    pub scope_managed_locals: std::collections::HashSet<Local>,
    pub captured_vars_in_scope: std::collections::HashSet<HirId>,
    pub boxed_var_cells: HashMap<HirId, (Local, Ty)>,
}

impl<'a> MirBuilder<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        global_fns: HashMap<String, String>,
        struct_defs: &'a HashMap<HirId, Vec<pace_ty::ResolvedField>>,
        class_defs: &'a HashMap<pace_hir::HirId, Vec<pace_ty::ResolvedField>>,
        class_vtables: &'a HashMap<HirId, Vec<pace_ty::VTableEntry>>,
        enum_defs: &'a HashMap<pace_hir::HirId, Vec<pace_hir::EnumVariant>>,
        global_env: &'a HashMap<pace_hir::HirId, Ty>,
        local_types: &'a HashMap<pace_hir::HirId, Ty>,
        named_types: &'a HashMap<String, (pace_hir::HirId, u8)>,
        static_fields_env: &'a HashMap<String, Ty>,
        methods_env: &'a HashMap<String, Ty>,
        class_parents: &'a HashMap<HirId, HirId>,
        global_functions_env: &'a HashMap<String, Ty>,
        global_vars_env: &'a HashMap<String, Ty>,
        resolved_global_names: &'a HashMap<pace_hir::HirId, String>,
    ) -> Self {
        let initial_block = BasicBlock {
            statements: Vec::new(),
            terminator: None,
        };
        MirBuilder {
            blocks: vec![initial_block],
            current_block: BasicBlockId(0),
            locals: Vec::new(),
            hir_to_local: HashMap::new(),
            current_expected_ty: None,
            current_self_local: None,
            global_fns,
            struct_defs,
            class_defs,
            class_vtables,
            enum_defs,
            global_env,
            local_types,
            named_types,
            static_fields_env,
            methods_env,
            class_parents,
            global_functions_env,
            global_vars_env,
            resolved_global_names,
            closure_functions: Vec::new(),
            closure_counter: 0,
            loop_stack: Vec::new(),
            scope_stack: Vec::new(),
            scope_managed_locals: std::collections::HashSet::new(),
            captured_vars_in_scope: std::collections::HashSet::new(),
            boxed_var_cells: HashMap::new(),
        }
    }

    pub fn new_local(&mut self, ty: Ty) -> Local {
        let l = Local(self.locals.len() as u32);
        let is_rc = matches!(ty, Ty::Class(_) | Ty::List(_) | Ty::Map(_, _) | Ty::Set(_) | Ty::StringBuilder);
        self.locals.push(ty);
        if is_rc && self.scope_stack.len() > 1 {
            self.scope_stack.last_mut().unwrap().push(l);
            self.scope_managed_locals.insert(l);
        }
        l
    }

    pub fn new_block(&mut self) -> BasicBlockId {
        let id = BasicBlockId(self.blocks.len() as u32);
        self.blocks.push(BasicBlock {
            statements: Vec::new(),
            terminator: None,
        });
        id
    }

    pub(super) fn push_stmt(&mut self, stmt: Statement) {
        let idx = self.current_block.0 as usize;
        self.blocks[idx].statements.push(stmt);
    }

}

impl<'a> MirBuilder<'a> {
    pub fn finish(mut self, params: &[Local], ret_ty: &Ty) -> MirBody {
        let current_bb = self.current_block.0 as usize;

        if self.blocks[current_bb].terminator.is_none() {
            let temp = self.new_local(ret_ty.clone());
            self.blocks[current_bb].statements.push(Statement::Assign(
                Lvalue::Local(temp),
                Rvalue::Constant(Constant::Int("0".to_string())),
            ));
            self.blocks[current_bb].terminator = Some(Terminator::Return(temp));
        }

        // ARC: Release all Class, List, and Map locals at return points, except the returned local and params
        for block in &mut self.blocks {
            if let Some(Terminator::Return(ret_local)) = block.terminator {
                for (i, ty) in self.locals.iter().enumerate() {
                    let local = Local(i as u32);
                    if matches!(ty, Ty::Class(_) | Ty::List(_) | Ty::Map(_, _) | Ty::Set(_) | Ty::StringBuilder)
                        && local != ret_local
                        && !params.contains(&local)
                        && !self.scope_managed_locals.contains(&local)
                    {
                        block.statements.push(Statement::Release(Lvalue::Local(local)));
                    }
                }
            }
        }

        MirBody {
            blocks: self.blocks,
            locals: self.locals,
        }
    }
}
