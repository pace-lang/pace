use pace_ast::BinaryOp;
use pace_ty::Type;
use ustr::Ustr;

#[derive(Debug, Clone)]
pub enum Statement {
    /// Write the rvalue to the given place.
    Assign(Place, Rvalue),
    
    /// Fake read for borrow checking / uninitialized checking.
    FakeRead(Place),
}

#[derive(Debug, Clone)]
pub enum Rvalue {
    /// Yields the operand unchanged.
    Use(Operand),
    
    /// Applies a binary operation.
    BinaryOp(BinaryOp, Operand, Operand),
    
    /// Applies a unary operation (e.g. Negation, Not).
    UnaryOp(UnaryOp, Operand),
    
    /// Casts an operand to a different type.
    Cast(Operand, Type),
    
    /// Creates a reference to a place (e.g. &mut x or &x).
    Ref(BorrowKind, Place),
    
    /// Creates an aggregate value (e.g., an array, tuple, or class instance).
    Aggregate(AggregateKind, Vec<Operand>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AggregateKind {
    Array,
    Tuple,
    Class(Ustr, usize),
    Closure(Ustr),
    EnumVariant(Ustr, Ustr, usize),
}

#[derive(Debug, Clone)]
pub enum Operand {
    /// Copies the value from a place (for Copy types).
    Copy(Place),
    
    /// Moves the value from a place (for non-Copy types).
    Move(Place),
    
    /// A constant value.
    Constant(Constant),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Place {
    pub local: Local,
    pub projection: Vec<ProjectionElem>,
}

impl Place {
    pub fn new(local: Local) -> Self {
        Self { local, projection: Vec::new() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Local(pub usize);

impl Local {
    pub fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProjectionElem {
    /// Dereference a pointer (*ptr)
    Deref,
    /// Field access (e.g. obj.field_name), stores (field_name, class_name, offset_bytes)
    Field(Ustr, Ustr, usize),
    /// Index into an array/slice (e.g. arr[i])
    Index(Local),
}

#[derive(Debug, Clone)]
pub enum Constant {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Null,
    /// Reference to a function by name for Call operations.
    Function(Ustr),
}

#[derive(Debug, Clone, Copy)]
pub enum BorrowKind {
    Shared,
    Mut,
}

#[derive(Debug, Clone, Copy)]
pub enum UnaryOp {
    Not,
    Neg,
}
