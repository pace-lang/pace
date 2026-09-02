import os
import re

def process_file(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    # Replacements
    content = content.replace("pace_ast::arena::ExprId", "pace_hir::ExprId")
    content = content.replace("pace_ast::arena::StmtId", "pace_hir::StmtId")
    content = content.replace("pace_ast::Expr", "pace_hir::Expr")
    content = content.replace("pace_ast::Stmt", "pace_hir::Stmt")
    content = content.replace("Expr::Identifier(name, _)", "Expr::Identifier(name)")
    content = content.replace("Expr::Identifier(func_name, _)", "Expr::Identifier(func_name)")
    
    # In stmt.rs and decl.rs we need to fix VarDecl and FuncDecl spans
    content = re.sub(
        r'Stmt::VarDecl\s*\{\s*name,\s*is_mutable,\s*type_annotation,\s*is_static,\s*visibility,\s*initializer,\s*span,?\s*\}',
        r'Stmt::VarDecl { name, is_mutable, type_annotation, is_static, visibility, initializer }',
        content
    )
    content = re.sub(
        r'Stmt::FuncDecl\s*\{\s*name,\s*generic_params,\s*params,\s*return_type,\s*body,\s*is_async,\s*is_static,\s*is_extern,\s*visibility,\s*span,?\s*\}',
        r'Stmt::FuncDecl { name, generic_params, params, return_type, body, is_async, is_static, is_extern, visibility }',
        content
    )

    with open(filepath, 'w') as f:
        f.write(content)

for root, dirs, files in os.walk("crates/pace-ty/src"):
    for file in files:
        if file.endswith(".rs"):
            process_file(os.path.join(root, file))

