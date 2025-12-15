use eyre::Result;
use rustpython_parser::{Parse, ast};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Parsed Python file with source for line number computation
pub struct ParsedFile {
    pub module: ast::ModModule,
    pub source: String,
}

impl ParsedFile {
    /// Convert a byte offset to a 1-based line number
    pub fn offset_to_line(&self, offset: u32) -> usize {
        let offset = offset as usize;
        self.source[..offset.min(self.source.len())].matches('\n').count() + 1
    }
}

/// Parse a Python file and return the AST module with source
pub fn parse_file(path: &Path) -> Result<ParsedFile> {
    let source = fs::read_to_string(path)?;
    let module = ast::ModModule::parse(&source, path.to_string_lossy().as_ref())?;
    Ok(ParsedFile { module, source })
}

/// Extract parameters as a map of name -> type
pub fn extract_params(args: &ast::Arguments) -> BTreeMap<String, String> {
    let mut params = BTreeMap::new();

    // Regular positional-or-keyword args
    for arg_with_default in args.args.iter() {
        let arg = &arg_with_default.def;
        let name = arg.arg.to_string();
        let type_str = arg.annotation.as_ref().map(|a| expr_to_string(a)).unwrap_or_default();
        if !type_str.is_empty() {
            params.insert(name, type_str);
        } else {
            params.insert(name, "...".to_string());
        }
    }

    // *args
    if let Some(vararg) = &args.vararg {
        let name = format!("*{}", vararg.arg);
        let type_str = vararg
            .annotation
            .as_ref()
            .map(|a| expr_to_string(a))
            .unwrap_or_default();
        if !type_str.is_empty() {
            params.insert(name, type_str);
        } else {
            params.insert(name, "...".to_string());
        }
    }

    // Keyword-only args
    for arg_with_default in args.kwonlyargs.iter() {
        let arg = &arg_with_default.def;
        let name = arg.arg.to_string();
        let type_str = arg.annotation.as_ref().map(|a| expr_to_string(a)).unwrap_or_default();
        if !type_str.is_empty() {
            params.insert(name, type_str);
        } else {
            params.insert(name, "...".to_string());
        }
    }

    // **kwargs
    if let Some(kwarg) = &args.kwarg {
        let name = format!("**{}", kwarg.arg);
        let type_str = kwarg.annotation.as_ref().map(|a| expr_to_string(a)).unwrap_or_default();
        if !type_str.is_empty() {
            params.insert(name, type_str);
        } else {
            params.insert(name, "...".to_string());
        }
    }

    params
}

/// Extract return type as a string
pub fn extract_returns(returns: Option<&ast::Expr>) -> Option<String> {
    returns.map(expr_to_string)
}

/// Convert an expression to a string representation
pub fn expr_to_string(expr: &ast::Expr) -> String {
    match expr {
        ast::Expr::Name(name) => name.id.to_string(),

        ast::Expr::Constant(c) => match &c.value {
            ast::Constant::None => "None".to_string(),
            ast::Constant::Bool(b) => if *b { "True" } else { "False" }.to_string(),
            ast::Constant::Str(s) => format!("\"{}\"", s),
            ast::Constant::Int(i) => i.to_string(),
            ast::Constant::Float(f) => f.to_string(),
            ast::Constant::Ellipsis => "...".to_string(),
            _ => "...".to_string(),
        },

        ast::Expr::Attribute(attr) => {
            format!("{}.{}", expr_to_string(&attr.value), attr.attr)
        }

        ast::Expr::Subscript(sub) => {
            format!("{}[{}]", expr_to_string(&sub.value), expr_to_string(&sub.slice))
        }

        ast::Expr::Tuple(tuple) => {
            let elts: Vec<_> = tuple.elts.iter().map(expr_to_string).collect();
            elts.join(", ")
        }

        ast::Expr::List(list) => {
            let elts: Vec<_> = list.elts.iter().map(expr_to_string).collect();
            format!("[{}]", elts.join(", "))
        }

        ast::Expr::BinOp(binop) => {
            let op = match binop.op {
                ast::Operator::BitOr => " | ",
                _ => " ? ",
            };
            format!("{}{}{}", expr_to_string(&binop.left), op, expr_to_string(&binop.right))
        }

        ast::Expr::Call(call) => {
            let func = expr_to_string(&call.func);
            let args: Vec<_> = call.args.iter().map(expr_to_string).collect();
            format!("{}({})", func, args.join(", "))
        }

        _ => "...".to_string(),
    }
}
