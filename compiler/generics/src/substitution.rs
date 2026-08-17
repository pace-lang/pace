use ast::TypeExpr;
use std::collections::HashMap;

#[derive(Debug)]
pub struct TypeSubstitution<'a> {
    pub arena: &'a bumpalo::Bump,
    map: HashMap<session::Symbol, TypeExpr<'a>>,
}

impl<'a> TypeSubstitution<'a> {
    pub fn new(
        arena: &'a bumpalo::Bump,
        type_params: &[session::Symbol],
        type_args: &[TypeExpr<'a>],
        _interner: &session::Interner,
    ) -> Self {
        let mut map = HashMap::new();
        for (param, arg) in type_params.iter().zip(type_args.iter()) {
            map.insert(*param, arg.clone());
        }
        Self { map, arena }
    }

    pub fn substitute(&self, te: &TypeExpr<'a>) -> TypeExpr<'a> {
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
                TypeExpr::GenericInstance(*name, sub_args)
            }
            TypeExpr::Optional(inner) => {
                TypeExpr::Optional(self.arena.alloc(self.substitute(inner)))
            }
            TypeExpr::Array(inner) => TypeExpr::Array(self.arena.alloc(self.substitute(inner))),
            TypeExpr::Function(params, ret_ty) => {
                let sub_params = params.iter().map(|p| self.substitute(p)).collect();
                let sub_ret = ret_ty.map(|rt| self.arena.alloc(self.substitute(rt)) as &TypeExpr<'a>);
                TypeExpr::Function(sub_params, sub_ret)
            }
        }
    }
}
