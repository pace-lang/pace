use super::TypeChecker;
use super::is_camel_case;
use crate::env::{ClassSignature, EnumSignature, FunctionSignature, Type};
use pace_hir::Stmt;
use std::collections::HashMap;

impl<'a> TypeChecker<'a> {
    pub(crate) fn register_types(&mut self, stmts: &[pace_hir::StmtId]) {
        for stmt_id in stmts {
            let stmt = self.arena.get_stmt(*stmt_id);
            match stmt {
                Stmt::Module { name, body } => {
                    let old_module = self.current_module;
                    self.current_module = *name;
                    self.register_types(body);
                    self.current_module = old_module;
                }
                Stmt::ClassDecl { name, visibility, .. } => {
                    self.env.register_class(
                        *name,
                        ClassSignature {
                            generic_params: None,
                            implements: None,
                            fields: HashMap::new(),
                            static_fields: HashMap::new(),
                            methods: HashMap::new(),
                            visibility: visibility.clone(),
                            module: self.current_module,
                            span: pace_span::Span::default(),
                        },
                    );
                }
                Stmt::InterfaceDecl { name, visibility, .. } => {
                    self.env.register_interface(
                        *name,
                        crate::env::InterfaceSignature {
                            generic_params: None,
                            methods: HashMap::new(),
                            visibility: visibility.clone(),
                            module: self.current_module,
                            span: pace_span::Span::default(),
                        },
                    );
                }
                Stmt::ActorDecl { name, visibility, .. } => {
                    self.env.register_actor(
                        *name,
                        crate::env::ActorSignature {
                            generic_params: None,
                            implements: None,
                            fields: HashMap::new(),
                            static_fields: HashMap::new(),
                            methods: HashMap::new(),
                            visibility: visibility.clone(),
                            module: self.current_module,
                            span: pace_span::Span::default(),
                        },
                    );
                }
                Stmt::StructDecl { name, visibility, .. } => {
                    self.env.register_struct(
                        *name,
                        ClassSignature {
                            generic_params: None,
                            implements: None,
                            fields: HashMap::new(),
                            static_fields: HashMap::new(),
                            methods: HashMap::new(),
                            visibility: visibility.clone(),
                            module: self.current_module,
                            span: pace_span::Span::default(),
                        },
                    );
                }
                Stmt::EnumDecl { name, visibility, .. } => {
                    self.env.register_enum(
                        *name,
                        EnumSignature {
                            generic_params: None,
                            variants: HashMap::new(),
                            visibility: visibility.clone(),
                            module: self.current_module,
                            span: pace_span::Span::default(),
                        },
                    );
                }
                _ => {}
            }
        }
    }

    pub(crate) fn resolve_signatures(&mut self, stmts: &[pace_hir::StmtId]) {
        for stmt_id in stmts {
            let stmt = self.arena.get_stmt(*stmt_id);
            match stmt {
                Stmt::Module { name, body } => {
                    let old_module = self.current_module;
                    self.current_module = *name;
                    self.resolve_signatures(body);
                    self.current_module = old_module;
                }
                Stmt::FuncDecl { name, params, return_type, visibility, generic_params, is_static, .. } => {
                    let span = self.arena.get_stmt_span(*stmt_id);
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
                            span,
                        });
                    }
                    let sig = FunctionSignature {
                        params: param_types,
                        return_type: ret_ty,
                        span,
                        is_used: false,
                        visibility: visibility.clone(),
                        module: self.current_module,
                        generic_params: generic_params.clone(),
                        is_static: *is_static,
                    };
                    self.env.register_function(*name, sig);
                }
                Stmt::ClassDecl { name, fields, methods, generic_params, implements, visibility, .. } => {
                    let implements_type = if let Some(i) = implements.as_ref() { Some(self.resolve_type_name(i)) } else { None };
                    if let Some(gp) = generic_params {
                        self.generic_params_in_scope.extend(gp.clone());
                    }
                    self.current_class = Some(*name);
                    let mut field_map = HashMap::new();
                    let mut static_field_map = HashMap::new();
                    for f in fields {
                        if let Stmt::VarDecl { name: f_name, type_annotation, is_static, visibility, is_mutable, is_weak, .. } = self.arena.get_stmt(*f) {
                            let span = self.arena.get_stmt_span(*f);
                            let f_ty = if let Some(ty_str) = type_annotation {
                                self.resolve_type_name(ty_str)
                            } else {
                                Type::Unknown
                            };
                            let field_sig = crate::env::FieldSignature { ty: f_ty, visibility: visibility.clone(), is_mutable: *is_mutable, is_weak: *is_weak, span };
                            if *is_static {
                                static_field_map.insert(*f_name, field_sig);
                            } else {
                                field_map.insert(*f_name, field_sig);
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
                                span: pace_span::Span::default(),
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
                        implements: implements_type,
                        fields: field_map,
                        static_fields: static_field_map,
                        methods: method_map,
                        visibility: visibility.clone(),
                        module: self.current_module,
                        span: pace_span::Span::default(),
                    };
                    if let Some(gp) = generic_params {
                        for _ in gp {
                            self.generic_params_in_scope.pop();
                        }
                    }
                    self.env.register_class(*name, sig);
                    self.current_class = None;
                }
                Stmt::ActorDecl { name, fields, methods, generic_params, implements, visibility, .. } => {
                    let implements_type = if let Some(i) = implements.as_ref() { Some(self.resolve_type_name(i)) } else { None };
                    if let Some(gp) = generic_params {
                        self.generic_params_in_scope.extend(gp.clone());
                    }
                    self.current_class = Some(*name);
                    let mut field_map = HashMap::new();
                    let mut static_field_map = HashMap::new();
                    for f in fields {
                        if let Stmt::VarDecl { name: f_name, type_annotation, is_static, visibility, is_mutable, is_weak, .. } = self.arena.get_stmt(*f) {
                            let span = self.arena.get_stmt_span(*f);
                            let f_ty = if let Some(ty_str) = type_annotation {
                                self.resolve_type_name(ty_str)
                            } else {
                                Type::Unknown
                            };
                            let field_sig = crate::env::FieldSignature { ty: f_ty, visibility: visibility.clone(), is_mutable: *is_mutable, is_weak: *is_weak, span };
                            if *is_static {
                                static_field_map.insert(*f_name, field_sig);
                            } else {
                                field_map.insert(*f_name, field_sig);
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
                                span: pace_span::Span::default(),
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
                        implements: implements_type,
                        fields: field_map,
                        static_fields: static_field_map,
                        methods: method_map,
                        visibility: visibility.clone(),
                        module: self.current_module,
                        span: pace_span::Span::default(),
                    };
                    if let Some(gp) = generic_params {
                        for _ in gp {
                            self.generic_params_in_scope.pop();
                        }
                    }
                    self.env.register_actor(*name, sig);
                    self.current_class = None;
                }
                Stmt::StructDecl { name, fields, generic_params, visibility, .. } => {
                    if let Some(gp) = generic_params {
                        self.generic_params_in_scope.extend(gp.clone());
                    }
                    let mut field_map = HashMap::new();
                    let mut static_field_map = HashMap::new();
                    for f in fields {
                        if let Stmt::VarDecl { name: f_name, type_annotation, is_static, visibility, is_mutable, is_weak, .. } = self.arena.get_stmt(*f) {
                            let span = self.arena.get_stmt_span(*f);
                            let f_ty = if let Some(ty_str) = type_annotation {
                                self.resolve_type_name(ty_str)
                            } else {
                                Type::Unknown
                            };
                            let field_sig = crate::env::FieldSignature { ty: f_ty, visibility: visibility.clone(), is_mutable: *is_mutable, is_weak: *is_weak, span };
                            if *is_static {
                                static_field_map.insert(*f_name, field_sig);
                            } else {
                                field_map.insert(*f_name, field_sig);
                            }
                        }
                    }
                    let sig = ClassSignature {
                        generic_params: generic_params.clone(),
                        implements: None,
                        fields: field_map,
                        static_fields: static_field_map,
                        methods: HashMap::new(),
                        visibility: visibility.clone(),
                        module: self.current_module,
                        span: pace_span::Span::default(),
                    };
                    if let Some(gp) = generic_params {
                        for _ in gp {
                            self.generic_params_in_scope.pop();
                        }
                    }
                    self.env.register_struct(*name, sig);
                }
                Stmt::EnumDecl { name, variants, generic_params, visibility, .. } => {
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
                        visibility: visibility.clone(),
                        module: self.current_module,
                        span: pace_span::Span::default(),
                    };
                    self.env.register_enum(*name, sig);
                }
                Stmt::InterfaceDecl { name, methods, generic_params, visibility, .. } => {
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
                                span: pace_span::Span::default(),
                                is_used: true,
                                visibility: visibility.clone(),
                                module: self.current_module,
                                generic_params: None,
                                is_static: *is_static,
                            };
                            method_map.insert(*m_name, sig);
                            
                            // Assign global vtable method index
                            let global_method_name = format!("{}_{}", name, m_name);
                            self.env.assign_global_interface_method_index(ustr::Ustr::from(global_method_name.as_str()));
                        }
                    }
                    if let Some(gp) = generic_params {
                        for _ in gp {
                            self.generic_params_in_scope.pop();
                        }
                    }
                    let sig = crate::env::InterfaceSignature {
                        generic_params: generic_params.clone(),
                        methods: method_map,
                        visibility: visibility.clone(),
                        module: self.current_module,
                        span: pace_span::Span::default(),
                    };
                    self.env.register_interface(*name, sig);
                    self.current_class = None;
                }
                Stmt::Import { path, .. }
                    // Basic placeholder for module resolution.
                    // For now, if we import "std/collection", we mock registering `List` and `Set`
                    if path == "std/collection" => {
                        self.env.register_class("List".into(), ClassSignature {
                            generic_params: Some(vec![pace_ast::GenericParam { name: "T".into(), bound: None }]),
                            implements: None,
                            fields: HashMap::new(), static_fields: HashMap::new(),
                            methods: HashMap::new(),
                            visibility: pace_ast::Visibility::Public,
                            module: "std/collection".into(),
                            span: pace_span::Span::default(),
                        });
                        self.env.register_class("Set".into(), ClassSignature {
                            generic_params: Some(vec![pace_ast::GenericParam { name: "T".into(), bound: None }]),
                            implements: None,
                            fields: HashMap::new(), static_fields: HashMap::new(),
                            methods: HashMap::new(),
                            visibility: pace_ast::Visibility::Public,
                            module: "std/collection".into(),
                            span: pace_span::Span::default(),
                        });
                    }
                Stmt::VarDecl { name, type_annotation, is_mutable, visibility, .. } => {
                    let span = self.arena.get_stmt_span(*stmt_id);
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
                        span,
                    });
                }
                _ => {}
            }
        }
    }
}
