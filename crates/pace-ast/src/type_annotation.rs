#[derive(Debug, Clone, PartialEq)]
pub struct TypeAnnotation {
    pub module_prefix: Option<ustr::Ustr>,
    pub name: ustr::Ustr,
    pub args: Vec<TypeAnnotation>,
    pub is_nullable: bool,
    // Function type support
    pub is_function: bool,
    pub function_params: Option<Vec<TypeAnnotation>>,
    pub function_return: Option<Box<TypeAnnotation>>,
}
