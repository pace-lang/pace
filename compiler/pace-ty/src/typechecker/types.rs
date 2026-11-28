use crate::ty::Ty;

use super::TypeChecker;

impl TypeChecker {
    pub(super) fn is_assignable(&self, target: &Ty, source: &Ty) -> bool {
        if target == source {
            return true;
        }
        if let Ty::Optional(inner) = target {
            if **inner == *source {
                return true;
            }
            if matches!(source, Ty::Optional(s_inner) if **s_inner == Ty::Void) {
                return true;
            }
        }
        if let (Ty::Class(target_id), Ty::Class(source_id)) = (target, source) {
            let mut curr = self.class_parents.get(source_id).copied();
            while let Some(pid) = curr {
                if pid == *target_id {
                    return true;
                }
                curr = self.class_parents.get(&pid).copied();
            }
        }
        false
    }

    pub(super) fn unify_types(&self, a: &Ty, b: &Ty) -> Option<Ty> {
        if a == b {
            return Some(a.clone());
        }
        if *a == Ty::Never {
            return Some(b.clone());
        }
        if *b == Ty::Never {
            return Some(a.clone());
        }
        if matches!(a, Ty::Optional(inner) if **inner == Ty::Void) {
            return Some(match b {
                Ty::Optional(_) => b.clone(),
                other => Ty::Optional(Box::new(other.clone())),
            });
        }
        if matches!(b, Ty::Optional(inner) if **inner == Ty::Void) {
            return Some(match a {
                Ty::Optional(_) => a.clone(),
                other => Ty::Optional(Box::new(other.clone())),
            });
        }
        if self.is_assignable(a, b) {
            return Some(a.clone());
        }
        if self.is_assignable(b, a) {
            return Some(b.clone());
        }
        None
    }

    pub fn get_type(&self, ast_ty: &pace_ast::Type) -> Result<Ty, String> {
        match ast_ty {
            pace_ast::Type::Named(id) => match id.name.as_str() {
                "Int" | "int" => Ok(Ty::Int),
                "Float" | "float" => Ok(Ty::Float),
                "String" | "string" => Ok(Ty::String),
                "StringBuilder" => Ok(Ty::StringBuilder),
                "Bool" | "bool" => Ok(Ty::Bool),
                "Unit" | "Void" | "void" => Ok(Ty::Void),
                "Never" => Ok(Ty::Never),
                other => {
                    if let Some(&(hir_id, kind)) = self.named_types.get(other) {
                        if kind == 1 {
                            return Ok(Ty::Class(hir_id));
                        } else if kind == 0 {
                            return Ok(Ty::Struct(hir_id));
                        } else {
                            return Ok(Ty::Enum(hir_id));
                        }
                    }
                    Err(format!("Unknown type: {}", other))
                }
            },
            pace_ast::Type::Generic(base, args, _) => {
                if let pace_ast::Type::Named(id) = &**base {
                    if id.name.as_str() == "Future" && args.len() == 1 {
                        let inner = self.get_type(&args[0])?;
                        return Ok(Ty::Future(Box::new(inner)));
                    }
                    if id.name.as_str() == "Option" && args.len() == 1 {
                        let inner = self.get_type(&args[0])?;
                        return Ok(Ty::Optional(Box::new(inner)));
                    }
                    if id.name.as_str() == "Result" && args.len() == 2 {
                        let ok = self.get_type(&args[0])?;
                        let err = self.get_type(&args[1])?;
                        return Ok(Ty::Result(Box::new(ok), Box::new(err)));
                    }
                    if id.name.as_str() == "List" && args.len() == 1 {
                        let inner = self.get_type(&args[0])?;
                        return Ok(Ty::List(Box::new(inner)));
                    }
                    if id.name.as_str() == "Map" && args.len() == 2 {
                        let k = self.get_type(&args[0])?;
                        let v = self.get_type(&args[1])?;
                        return Ok(Ty::Map(Box::new(k), Box::new(v)));
                    }
                    if id.name.as_str() == "Set" && args.len() == 1 {
                        let inner = self.get_type(&args[0])?;
                        return Ok(Ty::Set(Box::new(inner)));
                    }
                    let mut final_args = Vec::new();
                    if let Some(template) = self.generic_templates.get(&id.name.to_string()) {
                        let generic_params = match template {
                            pace_hir::Decl::Struct { generic_params, .. } => {
                                generic_params.clone().unwrap_or_default()
                            }
                            pace_hir::Decl::Class { generic_params, .. }
                            | pace_hir::Decl::Actor { generic_params, .. } => {
                                generic_params.clone().unwrap_or_default()
                            }
                            pace_hir::Decl::Enum { generic_params, .. } => {
                                generic_params.clone().unwrap_or_default()
                            }
                            pace_hir::Decl::Function { generic_params, .. } => {
                                generic_params.clone().unwrap_or_default()
                            }
                            _ => vec![],
                        };
                        for (i, (_, _, default_ty)) in generic_params.iter().enumerate() {
                            if i < args.len() {
                                final_args.push(args[i].clone());
                            } else if let Some(def_ty) = default_ty {
                                final_args.push(def_ty.clone());
                            }
                        }
                    } else {
                        final_args = args.clone();
                    }

                    let mut mono_name = id.name.to_string();
                    for arg in final_args {
                        let arg_ty = self.get_type(&arg)?;
                        mono_name.push_str(
                            &format!("_{:?}", arg_ty)
                                .replace(" ", "")
                                .replace("(", "_")
                                .replace(")", "_")
                                .replace(":", "_"),
                        );
                    }
                    if let Some(&(hir_id, kind)) = self.named_types.get(&mono_name) {
                        if kind == 1 {
                            return Ok(Ty::Class(hir_id));
                        } else if kind == 0 {
                            return Ok(Ty::Struct(hir_id));
                        } else {
                            return Ok(Ty::Enum(hir_id));
                        }
                    }
                    Err(format!("Generic instantiation not found: {}", mono_name))
                } else {
                    Err("Complex generic base types not supported".to_string())
                }
            }
            pace_ast::Type::Optional(inner_ty, _) => {
                let inner = self.get_type(inner_ty)?;
                Ok(Ty::Optional(Box::new(inner)))
            }
            pace_ast::Type::Closure(params, ret, _) => {
                let mut param_tys = Vec::new();
                for p in params {
                    param_tys.push(self.get_type(p)?);
                }
                let ret_ty = self.get_type(ret)?;
                Ok(Ty::Closure(param_tys, Box::new(ret_ty)))
            }
        }
    }

    pub fn resolve_type(&mut self, ast_ty: &pace_ast::Type) -> Result<Ty, String> {
        match ast_ty {
            pace_ast::Type::Named(id) => {
                match id.name.as_str() {
                    "Int" | "int" => Ok(Ty::Int),
                    "Float" | "float" => Ok(Ty::Float),
                    "String" | "string" => Ok(Ty::String),
                "StringBuilder" => Ok(Ty::StringBuilder),
                    "Bool" | "bool" => Ok(Ty::Bool),
                    "Unit" | "Void" | "void" => Ok(Ty::Void),
                    "Never" => Ok(Ty::Never),
                    other => {
                        let mangled_name = if let Some(curr) = &self.current_module {
                            if let Some(scope) = self.module_scopes.get(curr) {
                                scope
                                    .visible_symbols
                                    .get(other)
                                    .cloned()
                                    .unwrap_or_else(|| other.to_string())
                            } else {
                                other.to_string()
                            }
                        } else {
                            other.to_string()
                        };

                        if let Some(&(hir_id, kind)) = self.named_types.get(&mangled_name) {
                            if kind == 1 {
                                return Ok(Ty::Class(hir_id));
                            } else if kind == 0 {
                                return Ok(Ty::Struct(hir_id));
                            } else if kind == 3 {
                                return Ok(Ty::Actor(hir_id));
                            } else {
                                return Ok(Ty::Enum(hir_id));
                            }
                        }
                        // Check if it's a generic template being used without arguments
                        if self.generic_templates.contains_key(&mangled_name) {
                            return Err(format!("Type {} requires generic arguments", other));
                        }
                        Err(format!("Unknown type: {}", other))
                    }
                }
            }
            pace_ast::Type::Generic(base, args, _) => {
                if let pace_ast::Type::Named(id) = &**base {
                    if id.name.as_str() == "Future" && args.len() == 1 {
                        let inner = self.resolve_type(&args[0])?;
                        return Ok(Ty::Future(Box::new(inner)));
                    }
                    if id.name.as_str() == "Option" && args.len() == 1 {
                        let inner = self.resolve_type(&args[0])?;
                        return Ok(Ty::Optional(Box::new(inner)));
                    }
                    if id.name.as_str() == "Result" && args.len() == 2 {
                        let ok = self.resolve_type(&args[0])?;
                        let err = self.resolve_type(&args[1])?;
                        return Ok(Ty::Result(Box::new(ok), Box::new(err)));
                    }
                    if id.name.as_str() == "List" && args.len() == 1 {
                        let inner = self.resolve_type(&args[0])?;
                        return Ok(Ty::List(Box::new(inner)));
                    }
                    if id.name.as_str() == "Map" && args.len() == 2 {
                        let k = self.resolve_type(&args[0])?;
                        let v = self.resolve_type(&args[1])?;
                        return Ok(Ty::Map(Box::new(k), Box::new(v)));
                    }
                    if id.name.as_str() == "Set" && args.len() == 1 {
                        let inner = self.resolve_type(&args[0])?;
                        return Ok(Ty::Set(Box::new(inner)));
                    }
                    let mangled_name = if let Some(curr) = &self.current_module {
                        if let Some(scope) = self.module_scopes.get(curr) {
                            scope
                                .visible_symbols
                                .get(id.name.as_str())
                                .cloned()
                                .unwrap_or_else(|| id.name.to_string())
                        } else {
                            id.name.to_string()
                        }
                    } else {
                        id.name.to_string()
                    };
                    let gname = if self.generic_templates.contains_key(&mangled_name) {
                        mangled_name
                    } else {
                        id.name.to_string()
                    };
                    self.instantiate_generic(&gname, args)
                } else {
                    Err("Complex generic base types not supported".to_string())
                }
            }
            pace_ast::Type::Optional(inner_ty, _) => {
                let inner = self.resolve_type(inner_ty)?;
                Ok(Ty::Optional(Box::new(inner)))
            }
            pace_ast::Type::Closure(params, ret, _) => {
                let mut param_tys = Vec::new();
                for p in params {
                    param_tys.push(self.resolve_type(p)?);
                }
                let ret_ty = self.resolve_type(ret)?;
                Ok(Ty::Closure(param_tys, Box::new(ret_ty)))
            }
        }
    }

    pub fn display_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Int => "int".to_string(),
            Ty::Float => "float".to_string(),
            Ty::String => "string".to_string(),
            Ty::Bool => "bool".to_string(),
            Ty::Void => "Unit".to_string(),
            Ty::Never => "Never".to_string(),
            Ty::Optional(inner) => format!("{}?", self.display_ty(inner)),
            Ty::Future(inner) => format!("Future<{}>", self.display_ty(inner)),
            Ty::Result(ok, err) => format!("Result<{}, {}>", self.display_ty(ok), self.display_ty(err)),
            Ty::List(inner) => format!("List<{}>", self.display_ty(inner)),
            Ty::Map(k, v) => format!("Map<{}, {}>", self.display_ty(k), self.display_ty(v)),
            Ty::Set(inner) => format!("Set<{}>", self.display_ty(inner)),
            Ty::StringBuilder => "StringBuilder".to_string(),
            Ty::Function(args, ret) => {
                let args_str = args
                    .iter()
                    .map(|a| self.display_ty(a))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("fn({}) -> {}", args_str, self.display_ty(ret))
            }
            Ty::Closure(args, ret) => {
                let args_str = args
                    .iter()
                    .map(|a| self.display_ty(a))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({}) => {}", args_str, self.display_ty(ret))
            }
            Ty::Struct(id) | Ty::Class(id) | Ty::Enum(id) | Ty::Actor(id) => {
                // Try to find the name in named_types (reverse lookup)
                for (name, (nid, _)) in &self.named_types {
                    if nid == id {
                        if let Some(clean) = self.display_names.get(name) {
                            return clean.clone();
                        }
                        if let Some((_, base)) = name.split_once('_') {
                            return base.to_string();
                        }
                        return name.to_string();
                    }
                }
                format!("{:?}", ty)
            }
        }
    }
}
