import os
import re

def process_file(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    # Fix imports
    content = re.sub(r'use pace_ast::\{\s*([^}]*?)Expr([^}]*?)\s*\};', r'use pace_ast::{\1 \2};\nuse pace_hir::Expr;', content)
    content = re.sub(r'use pace_ast::\{\s*([^}]*?)Stmt([^}]*?)\s*\};', r'use pace_ast::{\1 \2};\nuse pace_hir::Stmt;', content)
    content = re.sub(r'use pace_ast::Expr;', r'use pace_hir::Expr;', content)
    content = re.sub(r'use pace_ast::Stmt;', r'use pace_hir::Stmt;', content)
    
    # Fix the weird comma left behind by regex
    content = content.replace("use pace_ast::{BinaryOp,  , Visibility};", "use pace_ast::{BinaryOp, Visibility};")
    content = content.replace("use pace_ast::{BinaryOp,   Visibility};", "use pace_ast::{BinaryOp, Visibility};")

    with open(filepath, 'w') as f:
        f.write(content)

for root, dirs, files in os.walk("crates/pace-ty/src"):
    for file in files:
        if file.endswith(".rs"):
            process_file(os.path.join(root, file))

