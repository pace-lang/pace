use ast::TypeExpr;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct TypeSubstitution {
    map: HashMap<session::Symbol, TypeExpr>,
}

impl TypeSubstitution {
    pub fn new(type_params: &[session::Symbol], type_args: &[TypeExpr]) -> Self {
        let mut map = HashMap::new();
        for (param, arg) in type_params.iter().zip(type_args.iter()) {
            map.insert(param.clone(), arg.clone());
        }
        Self { map }
    }

    pub fn substitute(&self, te: &TypeExpr) -> TypeExpr {
        match te {
            TypeExpr::Named(name) => {
                if let Some(concrete_ty) = self.map.get(name) {
                    concrete_ty.clone()
                } else {
                    te.clone()
                }
            }
            TypeExpr::GenericInstance(name, args) => {
                let sub_args = args.iter().map(|a| self.substitute(a)).collect();
                TypeExpr::GenericInstance(name.clone(), sub_args)
            }
            TypeExpr::Optional(inner) => {
                TypeExpr::Optional(Box::new(self.substitute(inner)))
            }
            TypeExpr::Array(inner) => {
                TypeExpr::Array(Box::new(self.substitute(inner)))
            }
        }
    }
}
