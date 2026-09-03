use crate::basic_block::BasicBlockData;
use pace_ty::Type;
use ustr::Ustr;
use pace_span::Span;

#[derive(Debug, Clone)]
pub struct MirBody {
    /// List of all basic blocks in the function.
    /// The first block (index 0) is always the entry block.
    pub basic_blocks: Vec<BasicBlockData>,
    
    /// Declarations of all locals (variables, temporaries, return slot).
    /// The return slot is always `Local(0)`.
    pub local_decls: Vec<LocalDecl>,
    
    /// The number of arguments the function takes.
    /// Arguments are stored in `local_decls[1..arg_count+1]`.
    pub arg_count: usize,
    
    /// Name of the function.
    pub name: Ustr,
    
    /// Whether this function is an external FFI function.
    pub is_extern: bool,
}

impl MirBody {
    pub fn new(name: Ustr, arg_count: usize, is_extern: bool) -> Self {
        Self {
            basic_blocks: vec![BasicBlockData {
                statements: Vec::new(),
                terminator: None,
                is_cleanup: false,
            }],
            local_decls: Vec::new(),
            arg_count,
            name,
            is_extern,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LocalDecl {
    /// The type of the local.
    pub ty: Type,
    
    /// True if the user declared this variable as mutable.
    pub mutability: Mutability,
    
    /// Information about the local (user-defined vs temporary).
    pub kind: LocalKind,
    
    /// The span where this local was defined.
    pub source_info: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutability {
    Not,
    Mut,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalKind {
    /// The return slot `_0`.
    ReturnPointer,
    
    /// A user-defined variable or function parameter.
    User(Ustr),
    
    /// A compiler-generated temporary.
    Temp,
}
