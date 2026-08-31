use super::TypeChecker;
use super::is_camel_case;
use crate::env::{ClassSignature, EnumSignature, FunctionSignature, Type};
use pace_ast::Stmt;
use std::collections::HashMap;

impl<'a> TypeChecker<'a> {
    pub(crate) fn register_types(&mut self, stmts: &[pace_ast::arena::StmtId]) {
        for stmt_id in stmts {
            let stmt = self.arena.get_stmt(*stmt_id);
            match stmt {
                Stmt::Module { name, body } => {
                    let old_module = self.current_module;
                    self.current_module = *name;
                    self.register_types(body);
                    self.current_module = old_module;
                }
                Stmt::ClassDecl { name, .. } | Stmt::InterfaceDecl { name, .. } => {
                    self.env.register_class(
                        *name,
                        ClassSignature {
                            generic_params: None,
                            fields: HashMap::new(),
                            static_fields: HashMap::new(),
                            methods: HashMap::new(),
                        },
                    );
                }
                Stmt::ActorDecl { name, .. } => {
                    self.env.register_actor(
                        *name,
                        crate::env::ActorSignature {
                            generic_params: None,
                            fields: HashMap::new(),
                            static_fields: HashMap::new(),
                            methods: HashMap::new(),
                        },
                    );
                }
                Stmt::StructDecl { name, .. } => {
                    self.env.register_struct(
                        *name,
                        ClassSignature {
                            generic_params: None,
                            fields: HashMap::new(),
                            static_fields: HashMap::new(),
                            methods: HashMap::new(),
                        },
                    );
                }
                Stmt::EnumDecl { name, .. } => {
                    self.env.register_enum(
                        *name,
                        EnumSignature {
                            generic_params: None,
                            variants: HashMap::new(),
                        },
                    );
                }
                _ => {}
            }
        }
    }

    pub(crate) fn resolve_signatures(&mut self, stmts: &[pace_ast::arena::StmtId]) {
        for stmt_id in stmts {
            let stmt = self.arena.get_stmt(*stmt_id);
            match stmt {
                Stmt::Module { name, body } => {
                    let old_module = self.current_module;
                    self.current_module = *name;
                    self.resolve_signatures(body);
                    self.current_module = old_module;
                }
                Stmt::FuncDecl { name, params, return_type, span, visibility, generic_params, is_static, .. } => {
                    if let Some(gp) = generic_params {
                        self.generic_params_in_scope.extend(gp.clone());
                    }
                    let mut param_types = Vec::new();
                    for param in params {
                        param_types.push(self.resolve_type_name(&param.type_annotation));
                    }
                    let ret_ty = if let Some(rt) = return_type {
                        self.resolve_type_name(rt)
                    } else {
                        Type::Void
                    };
                    if let Some(gp) = generic_params {
                        for _ in gp {
                            self.generic_params_in_scope.pop();
                        }
                    }
                    if !is_camel_case(name) && name != "main" && !name.contains("__") {
                        self.warnings.push(pace_errors::SemanticWarning::NamingConvention {
                            name: name.to_string(),
                            src: self.get_source(),
                            span: (*span),
                        });
                    }
                    let sig = FunctionSignature {
                        params: param_types,
                        return_type: ret_ty,
                        span: (*span),
                        is_used: false,
                        visibility: visibility.clone(),
                        module: self.current_module,
                        generic_params: generic_params.clone(),
                        is_static: *is_static,
                    };
                    self.env.register_function(*name, sig);
                }
                Stmt::ClassDecl { name, fields, methods, generic_params, .. } => {
                    if let Some(gp) = generic_params {
                        self.generic_params_in_scope.extend(gp.clone());
                    }
                    self.current_class = Some(*name);
                    let mut field_map = HashMap::new();
                    let mut static_field_map = HashMap::new();
                    for f in fields {
                        if let Stmt::VarDecl { name: f_name, type_annotation, is_static, .. } = self.arena.get_stmt(*f) {
                            let f_ty = if let Some(ty_str) = type_annotation {
                                self.resolve_type_name(ty_str)
                            } else {
                                Type::Unknown
                            };
                            if *is_static {
                                static_field_map.insert(*f_name, f_ty);
                            } else {
                                field_map.insert(*f_name, f_ty);
                            }
                        }
                    }

                    let mut method_map = HashMap::new();
                    for m in methods {
                        if let Stmt::FuncDecl { name: m_name, params, return_type, visibility, is_static, .. } = self.arena.get_stmt(*m) {
                            let mut param_types = Vec::new();
                            for param in params {
                                param_types.push(self.resolve_type_name(&param.type_annotation));
                            }
                            let ret_ty = if let Some(rt) = return_type {
                                self.resolve_type_name(rt)
                            } else {
                                Type::Void
                            };
                            let sig = FunctionSignature {
                                params: param_types,
                                return_type: ret_ty,
                                span: pace_ast::Span::default(),
                                is_used: true,
                                visibility: visibility.clone(),
                                module: self.current_module,
                                generic_params: generic_params.clone(),
                                is_static: *is_static,
                            };
                            method_map.insert(*m_name, sig);
                        }
                    }

                    let sig = ClassSignature {
                        generic_params: generic_params.clone(),
                        fields: field_map,
                        static_fields: static_field_map,
                        methods: method_map,
                    };
                    if let Some(gp) = generic_params {
                        for _ in gp {
                            self.generic_params_in_scope.pop();
                        }
                    }
                    self.env.register_class(*name, sig);
                    self.current_class = None;
                }
                Stmt::ActorDecl { name, fields, methods, generic_params, .. } => {
                    if let Some(gp) = generic_params {
                        self.generic_params_in_scope.extend(gp.clone());
                    }
                    self.current_class = Some(*name);
                    let mut field_map = HashMap::new();
                    let mut static_field_map = HashMap::new();
                    for f in fields {
                        if let Stmt::VarDecl { name: f_name, type_annotation, is_static, .. } = self.arena.get_stmt(*f) {
                            let f_ty = if let Some(ty_str) = type_annotation {
                                self.resolve_type_name(ty_str)
                            } else {
                                Type::Unknown
                            };
                            if *is_static {
                                static_field_map.insert(*f_name, f_ty);
                            } else {
                                field_map.insert(*f_name, f_ty);
                            }
                        }
                    }

                    let mut method_map = HashMap::new();
                    for m in methods {
                        if let Stmt::FuncDecl { name: m_name, params, return_type, visibility, is_static, .. } = self.arena.get_stmt(*m) {
                            let mut param_types = Vec::new();
                            for param in params {
                                param_types.push(self.resolve_type_name(&param.type_annotation));
                            }
                            let ret_ty = if let Some(rt) = return_type {
                                self.resolve_type_name(rt)
                            } else {
                                Type::Void
                            };
                            let sig = FunctionSignature {
                                params: param_types,
                                return_type: ret_ty,
                                span: pace_ast::Span::default(),
                                is_used: true,
                                visibility: visibility.clone(),
                                module: self.current_module,
                                generic_params: generic_params.clone(),
                                is_static: *is_static,
                            };
                            method_map.insert(*m_name, sig);
                        }
                    }

                    let sig = crate::env::ActorSignature {
                        generic_params: generic_params.clone(),
                        fields: field_map,
                        static_fields: static_field_map,
                        methods: method_map,
                    };
                    if let Some(gp) = generic_params {
                        for _ in gp {
                            self.generic_params_in_scope.pop();
                        }
                    }
                    self.env.register_actor(*name, sig);
                    self.current_class = None;
                }
                Stmt::StructDecl { name, fields, generic_params, .. } => {
                    if let Some(gp) = generic_params {
                        self.generic_params_in_scope.extend(gp.clone());
                    }
                    let mut field_map = HashMap::new();
                    let mut static_field_map = HashMap::new();
                    for f in fields {
                        if let Stmt::VarDecl { name: f_name, type_annotation, is_static, .. } = self.arena.get_stmt(*f) {
                            let f_ty = if let Some(ty_str) = type_annotation {
                                self.resolve_type_name(ty_str)
                            } else {
                                Type::Unknown
                            };
                            if *is_static {
                                static_field_map.insert(*f_name, f_ty);
                            } else {
                                field_map.insert(*f_name, f_ty);
                            }
                        }
                    }
                    let sig = ClassSignature {
                        generic_params: generic_params.clone(),
                        fields: field_map,
                        static_fields: static_field_map,
                        methods: HashMap::new(),
                    };
                    if let Some(gp) = generic_params {
                        for _ in gp {
                            self.generic_params_in_scope.pop();
                        }
                    }
                    self.env.register_struct(*name, sig);
                }
                Stmt::EnumDecl { name, variants, generic_params, .. } => {
                    let mut variant_map = HashMap::new();
                    self.current_class = Some(*name);

                    if let Some(params) = generic_params {
                        self.generic_params_in_scope.extend(params.clone());
                    }

                    for v in variants {
                        let fields = if let Some(fs) = &v.fields {
                            let mut resolved = Vec::new();
                            for f in fs {
                                resolved.push(self.resolve_type_name(f));
                            }
                            Some(resolved)
                        } else {
                            None
                        };
                        variant_map.insert(v.name, fields);
                    }

                    if let Some(gp) = generic_params {
                        for _ in gp {
                            self.generic_params_in_scope.pop();
                        }
                    }

                    self.current_class = None;

                    let sig = EnumSignature {
                        generic_params: generic_params.clone(),
                        variants: variant_map,
                    };
                    self.env.register_enum(*name, sig);
                }
                Stmt::InterfaceDecl { name, methods, generic_params, .. } => {
                    if let Some(gp) = generic_params {
                        self.generic_params_in_scope.extend(gp.clone());
                    }
                    self.current_class = Some(*name);
                    let mut method_map = HashMap::new();
                    for m in methods {
                        if let Stmt::FuncDecl { name: m_name, params, return_type, visibility, is_static, .. } = self.arena.get_stmt(*m) {
                            let mut param_types = Vec::new();
                            for param in params {
                                param_types.push(self.resolve_type_name(&param.type_annotation));
                            }
                            let ret_ty = if let Some(rt) = return_type {
                                self.resolve_type_name(rt)
                            } else {
                                Type::Void
                            };
                            let sig = FunctionSignature {
                                params: param_types,
                                return_type: ret_ty,
                                span: pace_ast::Span::default(),
                                is_used: true,
                                visibility: visibility.clone(),
                                module: self.current_module,
                                generic_params: None,
                                is_static: *is_static,
                            };
                            method_map.insert(*m_name, sig);
                        }
                    }
                    if let Some(gp) = generic_params {
                        for _ in gp {
                            self.generic_params_in_scope.pop();
                        }
                    }
                    let sig = ClassSignature {
                        generic_params: generic_params.clone(),
                        fields: HashMap::new(), static_fields: HashMap::new(),
                        methods: method_map,
                    };
                    self.env.register_class(*name, sig);
                    self.current_class = None;
                }
                Stmt::Import { path, .. }
                    // Basic placeholder for module resolution.
                    // For now, if we import "std/collection", we mock registering `List` and `Set`
                    if path == "std/collection" => {
                        self.env.register_class("List".into(), ClassSignature {
                            generic_params: Some(vec!["T".into()]),
                            fields: HashMap::new(), static_fields: HashMap::new(),
                            methods: HashMap::new(),
                        });
                        self.env.register_class("Set".into(), ClassSignature {
                            generic_params: Some(vec!["T".into()]),
                            fields: HashMap::new(), static_fields: HashMap::new(),
                            methods: HashMap::new(),
                        });
                    }
                Stmt::VarDecl { name, type_annotation, is_mutable, visibility, span, .. } => {
                    let ty = if let Some(annot) = type_annotation {
                        self.resolve_type_name(annot)
                    } else {
                        Type::Unknown
                    };
                    self.env.register_global_var(*name, crate::env::GlobalVariableSignature {
                        ty,
                        is_mutable: *is_mutable,
                        visibility: visibility.clone(),
                        module: self.current_module,
                        span: (*span),
                    });
                }
                _ => {}
            }
        }
    }
}
