use pace_ty::Ty;

use super::CGenerator;

impl CGenerator {
    pub(super) fn is_boxed_collection_ty(ty: &Ty) -> bool {
        matches!(ty, Ty::Struct(_) | Ty::Enum(_) | Ty::Float)
    }

    pub(super) fn emit_box_for_collection(&self, local: pace_mir::Local, ty: &Ty) -> String {
        if let Some(opt_name) = Self::is_primitive_opt(ty) {
            if opt_name == "PaceOptFloat" {
                return format!(
                    "(_{}.has_value ? memcpy(pace_alloc(sizeof(double), NULL), &_{}.value, sizeof(double)) : NULL)",
                    local.0, local.0
                );
            }
            return format!(
                "(_{}.has_value ? (void*)(intptr_t)_{}.value : NULL)",
                local.0, local.0
            );
        }
        let c_ty = self.emit_c_type(ty);
        if c_ty.contains('*') {
            format!("(void*)_{}", local.0)
        } else if Self::is_boxed_collection_ty(ty) {
            format!(
                "memcpy(pace_alloc(sizeof({}), NULL), &_{}, sizeof({}))",
                c_ty, local.0, c_ty
            )
        } else {
            format!("(void*)(intptr_t)_{}", local.0)
        }
    }

    pub(super) fn is_primitive_opt(ty: &Ty) -> Option<&'static str> {
        if let Ty::Optional(inner) = ty {
            match inner.as_ref() {
                Ty::Int => Some("PaceOptInt"),
                Ty::Float => Some("PaceOptFloat"),
                Ty::Bool => Some("PaceOptBool"),
                _ => None,
            }
        } else {
            None
        }
    }

    pub(super) fn is_refcounted_ptr(ty: &Ty) -> bool {
        matches!(
            ty,
            Ty::Class(_)
                | Ty::String
                | Ty::List(_)
                | Ty::Map(_, _)
                | Ty::Set(_)
                | Ty::StringBuilder
                | Ty::Closure(_, _)
                | Ty::Function(_, _)
        )
    }

    pub(super) fn emit_box_for_result(&self, local: pace_mir::Local, ty: &Ty) -> String {
        if Self::is_refcounted_ptr(ty) {
            format!("pace_retain_ptr((void*)_{})", local.0)
        } else {
            self.emit_box_for_ptr(local, ty)
        }
    }

    pub(super) fn emit_box_for_ptr(&self, local: pace_mir::Local, ty: &Ty) -> String {
        let c_ty = self.emit_c_type(ty);
        if c_ty == "void" {
            "NULL".to_string()
        } else if c_ty.contains('*') {
            format!("(void*)_{}", local.0)
        } else if Self::is_boxed_collection_ty(ty)
            || Self::is_primitive_opt(ty).is_some()
            || matches!(ty, Ty::Float | Ty::Result(_, _))
        {
            format!(
                "memcpy(pace_alloc(sizeof({}), NULL), &_{}, sizeof({}))",
                c_ty, local.0, c_ty
            )
        } else {
            format!("(void*)(intptr_t)_{}", local.0)
        }
    }

    pub(super) fn emit_c_type(&self, ty: &Ty) -> String {
        if let Some(opt_name) = Self::is_primitive_opt(ty) {
            return opt_name.to_string();
        }
        if let Ty::Optional(inner) = ty {
            if Self::is_boxed_collection_ty(inner) {
                return format!("{}*", self.emit_c_type(inner));
            }
        }
        match ty {
            Ty::Int | Ty::Bool => "long long".to_string(),
            Ty::Float => "double".to_string(),
            Ty::String => "struct PaceString*".to_string(),
            Ty::Struct(id) => format!("struct pace_{}", id.0),
            Ty::Class(id) | Ty::Actor(id) => format!("struct pace_{}*", id.0),
            Ty::Enum(id) => format!("struct pace_{}", id.0),
            Ty::Closure(_, _) => "PaceClosure*".to_string(),
            Ty::Function(_, _) => "void*".to_string(),
            Ty::Optional(inner) => {
                let inner_c = self.emit_c_type(inner);
                if inner_c == "void" {
                    "void*".to_string()
                } else {
                    inner_c
                }
            }
            Ty::Future(_) => "void*".to_string(),
            Ty::Result(_, _) => "PaceResult".to_string(),
            Ty::List(_) => "struct PaceList*".to_string(),
            Ty::Map(_, _) => "struct PaceMap*".to_string(),
            Ty::Set(_) => "struct PaceSet*".to_string(),
            Ty::StringBuilder => "struct PaceStringBuilder*".to_string(),
            Ty::Void | Ty::Never => "void".to_string(),
        }
    }

    pub(super) fn emit_c_default_val(&self, ty: &Ty) -> String {
        if Self::is_primitive_opt(ty).is_some() {
            return "{0, 0}".to_string();
        }
        if let Ty::Optional(inner) = ty {
            if Self::is_boxed_collection_ty(inner) {
                return "NULL".to_string();
            }
        }
        match ty {
            Ty::Int | Ty::Bool | Ty::Float => "0".to_string(),
            Ty::String | Ty::Class(_) | Ty::Actor(_) | Ty::Function(_, _) | Ty::Closure(_, _) | Ty::List(_) | Ty::Map(_, _) | Ty::Set(_) | Ty::StringBuilder => "NULL".to_string(),
            Ty::Struct(_) | Ty::Enum(_) => "{0}".to_string(),
            Ty::Optional(inner) => {
                let default_val = self.emit_c_default_val(inner);
                if default_val.is_empty() {
                    "NULL".to_string()
                } else {
                    default_val
                }
            }
            Ty::Future(inner) => {
                let default_val = self.emit_c_default_val(inner);
                if default_val.is_empty() {
                    "NULL".to_string()
                } else {
                    default_val
                }
            }
            Ty::Result(_, _) => "{0, NULL, NULL}".to_string(),
            Ty::Void | Ty::Never => "".to_string(),
        }
    }

}
