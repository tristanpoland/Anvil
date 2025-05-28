use crate::error::{AnvilError, AnvilResult};
use crate::objects::ShellObject;
use std::collections::HashMap;
use syn::{Expr, Lit, BinOp, UnOp};
use quote::ToTokens;

pub struct EvaluationEngine {
    variables: HashMap<String, ShellObject>,
    functions: HashMap<String, ShellObject>,
}

impl EvaluationEngine {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            functions: HashMap::new(),
        }
    }

    pub fn with_variables(variables: HashMap<String, ShellObject>) -> Self {
        Self {
            variables,
            functions: HashMap::new(),
        }
    }

    /// Evaluate a Rust expression string
    pub fn evaluate_expression(&self, expr_str: &str) -> AnvilResult<ShellObject> {
        // Parse the expression
        let expr: Expr = syn::parse_str(expr_str)
            .map_err(|e| AnvilError::parse(format!("Failed to parse expression: {}", e)))?;

        self.evaluate_expr(&expr)
    }

    /// Evaluate a parsed expression
    pub fn evaluate_expr(&self, expr: &Expr) -> AnvilResult<ShellObject> {
        match expr {
            Expr::Lit(lit) => self.evaluate_literal(&lit.lit),
            Expr::Path(path) => self.evaluate_path(path),
            Expr::Binary(binary) => self.evaluate_binary(binary),
            Expr::Unary(unary) => self.evaluate_unary(unary),
            Expr::Paren(paren) => self.evaluate_expr(&paren.expr),
            Expr::Array(array) => self.evaluate_array(array),
            Expr::Tuple(tuple) => self.evaluate_tuple(tuple),
            Expr::Call(call) => self.evaluate_call(call),
            Expr::MethodCall(method_call) => self.evaluate_method_call(method_call),
            Expr::Field(field) => self.evaluate_field_access(field),
            Expr::Index(index) => self.evaluate_index(index),
            Expr::Block(block) => self.evaluate_block(block),
            Expr::If(if_expr) => self.evaluate_if(if_expr),
            Expr::Match(match_expr) => self.evaluate_match(match_expr),
            _ => Err(AnvilError::eval(format!(
                "Unsupported expression type: {}",
                expr.to_token_stream()
            ))),
        }
    }

    fn evaluate_literal(&self, lit: &Lit) -> AnvilResult<ShellObject> {
        match lit {
            Lit::Str(s) => Ok(ShellObject::String(s.value())),
            Lit::Int(i) => {
                let value = i.base10_parse::<i64>()
                    .map_err(|e| AnvilError::eval(format!("Invalid integer: {}", e)))?;
                Ok(ShellObject::Integer(value))
            }
            Lit::Float(f) => {
                let value = f.base10_parse::<f64>()
                    .map_err(|e| AnvilError::eval(format!("Invalid float: {}", e)))?;
                Ok(ShellObject::Float(value))
            }
            Lit::Bool(b) => Ok(ShellObject::Boolean(b.value)),
            Lit::Char(c) => Ok(ShellObject::String(c.value().to_string())),
            _ => Err(AnvilError::eval("Unsupported literal type")),
        }
    }

    fn evaluate_path(&self, path: &syn::ExprPath) -> AnvilResult<ShellObject> {
        let path_str = path.path.segments.iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>()
            .join("::");

        // Check if it's a simple variable reference
        if path.path.segments.len() == 1 {
            let var_name = &path.path.segments[0].ident.to_string();
            if let Some(value) = self.variables.get(var_name) {
                return Ok(value.clone());
            }
        }

        // Handle built-in constants and types
        match path_str.as_str() {
            "true" => Ok(ShellObject::Boolean(true)),
            "false" => Ok(ShellObject::Boolean(false)),
            _ => Err(AnvilError::eval(format!("Unknown identifier: {}", path_str))),
        }
    }

    fn evaluate_binary(&self, binary: &syn::ExprBinary) -> AnvilResult<ShellObject> {
        let left = self.evaluate_expr(&binary.left)?;
        let right = self.evaluate_expr(&binary.right)?;

        match binary.op {
            BinOp::Add(_) => self.add_objects(left, right),
            BinOp::Sub(_) => self.sub_objects(left, right),
            BinOp::Mul(_) => self.mul_objects(left, right),
            BinOp::Div(_) => self.div_objects(left, right),
            BinOp::Rem(_) => self.rem_objects(left, right),
            BinOp::And(_) => self.and_objects(left, right),
            BinOp::Or(_) => self.or_objects(left, right),
            BinOp::BitXor(_) => self.xor_objects(left, right),
            BinOp::BitAnd(_) => self.bitand_objects(left, right),
            BinOp::BitOr(_) => self.bitor_objects(left, right),
            BinOp::Shl(_) => self.shl_objects(left, right),
            BinOp::Shr(_) => self.shr_objects(left, right),
            BinOp::Eq(_) => Ok(ShellObject::Boolean(self.eq_objects(&left, &right))),
            BinOp::Lt(_) => Ok(ShellObject::Boolean(self.lt_objects(&left, &right)?)),
            BinOp::Le(_) => Ok(ShellObject::Boolean(self.le_objects(&left, &right)?)),
            BinOp::Ne(_) => Ok(ShellObject::Boolean(!self.eq_objects(&left, &right))),
            BinOp::Ge(_) => Ok(ShellObject::Boolean(self.ge_objects(&left, &right)?)),
            BinOp::Gt(_) => Ok(ShellObject::Boolean(self.gt_objects(&left, &right)?)),
            _ => Err(AnvilError::eval(format!("Unsupported binary operator: {:?}", binary.op))),
        }
    }

    fn evaluate_unary(&self, unary: &syn::ExprUnary) -> AnvilResult<ShellObject> {
        let operand = self.evaluate_expr(&unary.expr)?;

        match unary.op {
            UnOp::Not(_) => match operand {
                ShellObject::Boolean(b) => Ok(ShellObject::Boolean(!b)),
                _ => Err(AnvilError::type_error("boolean", operand.type_name())),
            },
            UnOp::Neg(_) => match operand {
                ShellObject::Integer(i) => Ok(ShellObject::Integer(-i)),
                ShellObject::Float(f) => Ok(ShellObject::Float(-f)),
                _ => Err(AnvilError::type_error("numeric", operand.type_name())),
            },
            _ => Err(AnvilError::eval(format!("Unsupported unary operator: {:?}", unary.op))),
        }
    }

    fn evaluate_array(&self, array: &syn::ExprArray) -> AnvilResult<ShellObject> {
        let mut elements = Vec::new();
        for elem in &array.elems {
            elements.push(self.evaluate_expr(elem)?);
        }
        Ok(ShellObject::Array(elements))
    }

    fn evaluate_tuple(&self, tuple: &syn::ExprTuple) -> AnvilResult<ShellObject> {
        if tuple.elems.is_empty() {
            return Ok(ShellObject::Unit);
        }

        let mut elements = Vec::new();
        for elem in &tuple.elems {
            elements.push(self.evaluate_expr(elem)?);
        }
        Ok(ShellObject::Array(elements)) // Represent tuples as arrays for simplicity
    }

    fn evaluate_call(&self, call: &syn::ExprCall) -> AnvilResult<ShellObject> {
        // For now, we'll handle a few built-in functions
        if let Expr::Path(path) = &*call.func {
            let func_name = path.path.segments.iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");

            match func_name.as_str() {
                "println" | "print" => {
                    let mut output = String::new();
                    for (i, arg) in call.args.iter().enumerate() {
                        if i > 0 {
                            output.push(' ');
                        }
                        let value = self.evaluate_expr(arg)?;
                        output.push_str(&value.to_display_string());
                    }
                    if func_name == "println" {
                        println!("{}", output);
                    } else {
                        print!("{}", output);
                    }
                    Ok(ShellObject::Unit)
                }
                "format" => {
                    // Simplified format implementation
                    if call.args.is_empty() {
                        return Ok(ShellObject::String(String::new()));
                    }
                    let format_str = self.evaluate_expr(&call.args[0])?;
                    if let ShellObject::String(s) = format_str {
                        Ok(ShellObject::String(s))
                    } else {
                        Ok(ShellObject::String(format_str.to_display_string()))
                    }
                }
                "len" => {
                    if call.args.len() != 1 {
                        return Err(AnvilError::eval("len() requires exactly one argument"));
                    }
                    let arg = self.evaluate_expr(&call.args[0])?;
                    match arg {
                        ShellObject::String(s) => Ok(ShellObject::Integer(s.len() as i64)),
                        ShellObject::Array(arr) => Ok(ShellObject::Integer(arr.len() as i64)),
                        _ => Err(AnvilError::type_error("string or array", arg.type_name())),
                    }
                }
                _ => Err(AnvilError::eval(format!("Unknown function: {}", func_name))),
            }
        } else {
            Err(AnvilError::eval("Complex function calls not supported yet"))
        }
    }

    fn evaluate_method_call(&self, method_call: &syn::ExprMethodCall) -> AnvilResult<ShellObject> {
        let receiver = self.evaluate_expr(&method_call.receiver)?;
        let method_name = method_call.method.to_string();

        match method_name.as_str() {
            "len" => match receiver {
                ShellObject::String(s) => Ok(ShellObject::Integer(s.len() as i64)),
                ShellObject::Array(arr) => Ok(ShellObject::Integer(arr.len() as i64)),
                _ => Err(AnvilError::eval(format!("Type {} has no method len", receiver.type_name()))),
            },
            "is_empty" => match receiver {
                ShellObject::String(s) => Ok(ShellObject::Boolean(s.is_empty())),
                ShellObject::Array(arr) => Ok(ShellObject::Boolean(arr.is_empty())),
                _ => Err(AnvilError::eval(format!("Type {} has no method is_empty", receiver.type_name()))),
            },
            "push" => {
                if method_call.args.len() != 1 {
                    return Err(AnvilError::eval("push() requires exactly one argument"));
                }
                let arg = self.evaluate_expr(&method_call.args[0])?;
                match receiver {
                    ShellObject::Array(mut arr) => {
                        arr.push(arg);
                        Ok(ShellObject::Array(arr))
                    }
                    _ => Err(AnvilError::eval(format!("Type {} has no method push", receiver.type_name()))),
                }
            }
            "get" => {
                if method_call.args.len() != 1 {
                    return Err(AnvilError::eval("get() requires exactly one argument"));
                }
                let key = self.evaluate_expr(&method_call.args[0])?;
                match (receiver, key) {
                    (ShellObject::Map(map), ShellObject::String(key_str)) => {
                        Ok(map.get(&key_str).cloned().unwrap_or(ShellObject::Unit))
                    }
                    (ShellObject::Array(arr), ShellObject::Integer(idx)) => {
                        if idx >= 0 && (idx as usize) < arr.len() {
                            Ok(arr[idx as usize].clone())
                        } else {
                            Ok(ShellObject::Unit)
                        }
                    }
                    _ => Err(AnvilError::eval("Invalid get() operation")),
                }
            }
            _ => {
                // Try to get field from the object
                receiver.get_field(&method_name)
            }
        }
    }

    fn evaluate_field_access(&self, field: &syn::ExprField) -> AnvilResult<ShellObject> {
        let base = self.evaluate_expr(&field.base)?;
        
        if let syn::Member::Named(field_name) = &field.member {
            base.get_field(&field_name.to_string())
        } else {
            Err(AnvilError::eval("Tuple field access not supported"))
        }
    }

    fn evaluate_index(&self, index: &syn::ExprIndex) -> AnvilResult<ShellObject> {
        let base = self.evaluate_expr(&index.expr)?;
        let index_val = self.evaluate_expr(&index.index)?;

        match (base, index_val) {
            (ShellObject::Array(arr), ShellObject::Integer(idx)) => {
                if idx >= 0 && (idx as usize) < arr.len() {
                    Ok(arr[idx as usize].clone())
                } else {
                    Err(AnvilError::runtime(format!("Index {} out of bounds for array of length {}", idx, arr.len())))
                }
            }
            (ShellObject::Map(map), ShellObject::String(key)) => {
                Ok(map.get(&key).cloned().unwrap_or(ShellObject::Unit))
            }
            (ShellObject::String(s), ShellObject::Integer(idx)) => {
                if idx >= 0 && (idx as usize) < s.len() {
                    let chars: Vec<char> = s.chars().collect();
                    Ok(ShellObject::String(chars[idx as usize].to_string()))
                } else {
                    Err(AnvilError::runtime(format!("Index {} out of bounds for string of length {}", idx, s.len())))
                }
            }
            _ => Err(AnvilError::eval("Invalid index operation")),
        }
    }

    fn evaluate_block(&self, _block: &syn::ExprBlock) -> AnvilResult<ShellObject> {
        // Block evaluation would require more complex state management
        Err(AnvilError::eval("Block expressions not supported in simple evaluation"))
    }

    fn evaluate_if(&self, _if_expr: &syn::ExprIf) -> AnvilResult<ShellObject> {
        // If expressions would require control flow
        Err(AnvilError::eval("If expressions not supported in simple evaluation"))
    }

    fn evaluate_match(&self, _match_expr: &syn::ExprMatch) -> AnvilResult<ShellObject> {
        // Match expressions would require pattern matching
        Err(AnvilError::eval("Match expressions not supported in simple evaluation"))
    }

    // Arithmetic operations
    fn add_objects(&self, left: ShellObject, right: ShellObject) -> AnvilResult<ShellObject> {
        match (left, right) {
            (ShellObject::Integer(a), ShellObject::Integer(b)) => Ok(ShellObject::Integer(a + b)),
            (ShellObject::Float(a), ShellObject::Float(b)) => Ok(ShellObject::Float(a + b)),
            (ShellObject::Integer(a), ShellObject::Float(b)) => Ok(ShellObject::Float(a as f64 + b)),
            (ShellObject::Float(a), ShellObject::Integer(b)) => Ok(ShellObject::Float(a + b as f64)),
            (ShellObject::String(a), ShellObject::String(b)) => Ok(ShellObject::String(a + &b)),
            (ShellObject::Array(mut a), ShellObject::Array(b)) => {
                a.extend(b);
                Ok(ShellObject::Array(a))
            }
            (a, b) => Err(AnvilError::type_error("compatible types for addition", &format!("{} + {}", a.type_name(), b.type_name()))),
        }
    }

    fn sub_objects(&self, left: ShellObject, right: ShellObject) -> AnvilResult<ShellObject> {
        match (left, right) {
            (ShellObject::Integer(a), ShellObject::Integer(b)) => Ok(ShellObject::Integer(a - b)),
            (ShellObject::Float(a), ShellObject::Float(b)) => Ok(ShellObject::Float(a - b)),
            (ShellObject::Integer(a), ShellObject::Float(b)) => Ok(ShellObject::Float(a as f64 - b)),
            (ShellObject::Float(a), ShellObject::Integer(b)) => Ok(ShellObject::Float(a - b as f64)),
            (a, b) => Err(AnvilError::type_error("numeric types for subtraction", &format!("{} - {}", a.type_name(), b.type_name()))),
        }
    }

    fn mul_objects(&self, left: ShellObject, right: ShellObject) -> AnvilResult<ShellObject> {
        match (left, right) {
            (ShellObject::Integer(a), ShellObject::Integer(b)) => Ok(ShellObject::Integer(a * b)),
            (ShellObject::Float(a), ShellObject::Float(b)) => Ok(ShellObject::Float(a * b)),
            (ShellObject::Integer(a), ShellObject::Float(b)) => Ok(ShellObject::Float(a as f64 * b)),
            (ShellObject::Float(a), ShellObject::Integer(b)) => Ok(ShellObject::Float(a * b as f64)),
            (a, b) => Err(AnvilError::type_error("numeric types for multiplication", &format!("{} * {}", a.type_name(), b.type_name()))),
        }
    }

    fn div_objects(&self, left: ShellObject, right: ShellObject) -> AnvilResult<ShellObject> {
        match (left, right) {
            (ShellObject::Integer(a), ShellObject::Integer(b)) => {
                if b == 0 {
                    Err(AnvilError::runtime("Division by zero"))
                } else {
                    Ok(ShellObject::Float(a as f64 / b as f64))
                }
            }
            (ShellObject::Float(a), ShellObject::Float(b)) => {
                if b == 0.0 {
                    Err(AnvilError::runtime("Division by zero"))
                } else {
                    Ok(ShellObject::Float(a / b))
                }
            }
            (ShellObject::Integer(a), ShellObject::Float(b)) => {
                if b == 0.0 {
                    Err(AnvilError::runtime("Division by zero"))
                } else {
                    Ok(ShellObject::Float(a as f64 / b))
                }
            }
            (ShellObject::Float(a), ShellObject::Integer(b)) => {
                if b == 0 {
                    Err(AnvilError::runtime("Division by zero"))
                } else {
                    Ok(ShellObject::Float(a / b as f64))
                }
            }
            (a, b) => Err(AnvilError::type_error("numeric types for division", &format!("{} / {}", a.type_name(), b.type_name()))),
        }
    }

    fn rem_objects(&self, left: ShellObject, right: ShellObject) -> AnvilResult<ShellObject> {
        match (left, right) {
            (ShellObject::Integer(a), ShellObject::Integer(b)) => {
                if b == 0 {
                    Err(AnvilError::runtime("Division by zero"))
                } else {
                    Ok(ShellObject::Integer(a % b))
                }
            }
            (a, b) => Err(AnvilError::type_error("integer types for remainder", &format!("{} % {}", a.type_name(), b.type_name()))),
        }
    }

    // Logical operations
    fn and_objects(&self, left: ShellObject, right: ShellObject) -> AnvilResult<ShellObject> {
        match (left, right) {
            (ShellObject::Boolean(a), ShellObject::Boolean(b)) => Ok(ShellObject::Boolean(a && b)),
            (a, b) => Err(AnvilError::type_error("boolean types for logical AND", &format!("{} && {}", a.type_name(), b.type_name()))),
        }
    }

    fn or_objects(&self, left: ShellObject, right: ShellObject) -> AnvilResult<ShellObject> {
        match (left, right) {
            (ShellObject::Boolean(a), ShellObject::Boolean(b)) => Ok(ShellObject::Boolean(a || b)),
            (a, b) => Err(AnvilError::type_error("boolean types for logical OR", &format!("{} || {}", a.type_name(), b.type_name()))),
        }
    }

    // Bitwise operations (simplified for integers only)
    fn xor_objects(&self, left: ShellObject, right: ShellObject) -> AnvilResult<ShellObject> {
        match (left, right) {
            (ShellObject::Integer(a), ShellObject::Integer(b)) => Ok(ShellObject::Integer(a ^ b)),
            (a, b) => Err(AnvilError::type_error("integer types for XOR", &format!("{} ^ {}", a.type_name(), b.type_name()))),
        }
    }

    fn bitand_objects(&self, left: ShellObject, right: ShellObject) -> AnvilResult<ShellObject> {
        match (left, right) {
            (ShellObject::Integer(a), ShellObject::Integer(b)) => Ok(ShellObject::Integer(a & b)),
            (a, b) => Err(AnvilError::type_error("integer types for bitwise AND", &format!("{} & {}", a.type_name(), b.type_name()))),
        }
    }

    fn bitor_objects(&self, left: ShellObject, right: ShellObject) -> AnvilResult<ShellObject> {
        match (left, right) {
            (ShellObject::Integer(a), ShellObject::Integer(b)) => Ok(ShellObject::Integer(a | b)),
            (a, b) => Err(AnvilError::type_error("integer types for bitwise OR", &format!("{} | {}", a.type_name(), b.type_name()))),
        }
    }

    fn shl_objects(&self, left: ShellObject, right: ShellObject) -> AnvilResult<ShellObject> {
        match (left, right) {
            (ShellObject::Integer(a), ShellObject::Integer(b)) => {
                if b >= 0 && b < 64 {
                    Ok(ShellObject::Integer(a << b))
                } else {
                    Err(AnvilError::runtime("Shift amount out of range"))
                }
            }
            (a, b) => Err(AnvilError::type_error("integer types for left shift", &format!("{} << {}", a.type_name(), b.type_name()))),
        }
    }

    fn shr_objects(&self, left: ShellObject, right: ShellObject) -> AnvilResult<ShellObject> {
        match (left, right) {
            (ShellObject::Integer(a), ShellObject::Integer(b)) => {
                if b >= 0 && b < 64 {
                    Ok(ShellObject::Integer(a >> b))
                } else {
                    Err(AnvilError::runtime("Shift amount out of range"))
                }
            }
            (a, b) => Err(AnvilError::type_error("integer types for right shift", &format!("{} >> {}", a.type_name(), b.type_name()))),
        }
    }

    // Comparison operations
    fn eq_objects(&self, left: &ShellObject, right: &ShellObject) -> bool {
        match (left, right) {
            (ShellObject::Integer(a), ShellObject::Integer(b)) => a == b,
            (ShellObject::Float(a), ShellObject::Float(b)) => a == b,
            (ShellObject::Integer(a), ShellObject::Float(b)) => *a as f64 == *b,
            (ShellObject::Float(a), ShellObject::Integer(b)) => *a == *b as f64,
            (ShellObject::String(a), ShellObject::String(b)) => a == b,
            (ShellObject::Boolean(a), ShellObject::Boolean(b)) => a == b,
            (ShellObject::Unit, ShellObject::Unit) => true,
            _ => false,
        }
    }

    fn lt_objects(&self, left: &ShellObject, right: &ShellObject) -> AnvilResult<bool> {
        match (left, right) {
            (ShellObject::Integer(a), ShellObject::Integer(b)) => Ok(a < b),
            (ShellObject::Float(a), ShellObject::Float(b)) => Ok(a < b),
            (ShellObject::Integer(a), ShellObject::Float(b)) => Ok((*a as f64) < *b),
            (ShellObject::Float(a), ShellObject::Integer(b)) => Ok(*a < (*b as f64)),
            (ShellObject::String(a), ShellObject::String(b)) => Ok(a < b),
            (a, b) => Err(AnvilError::type_error("comparable types", &format!("{} < {}", a.type_name(), b.type_name()))),
        }
    }

    fn le_objects(&self, left: &ShellObject, right: &ShellObject) -> AnvilResult<bool> {
        Ok(self.lt_objects(left, right)? || self.eq_objects(left, right))
    }

    fn gt_objects(&self, left: &ShellObject, right: &ShellObject) -> AnvilResult<bool> {
        Ok(!self.le_objects(left, right)?)
    }

    fn ge_objects(&self, left: &ShellObject, right: &ShellObject) -> AnvilResult<bool> {
        Ok(!self.lt_objects(left, right)?)
    }

    // Variable management
    pub fn set_variable(&mut self, name: String, value: ShellObject) {
        self.variables.insert(name, value);
    }

    pub fn get_variable(&self, name: &str) -> Option<&ShellObject> {
        self.variables.get(name)
    }

    pub fn remove_variable(&mut self, name: &str) -> Option<ShellObject> {
        self.variables.remove(name)
    }

    pub fn clear_variables(&mut self) {
        self.variables.clear();
    }

    pub fn variables(&self) -> &HashMap<String, ShellObject> {
        &self.variables
    }
}

impl Default for EvaluationEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_evaluation() {
        let engine = EvaluationEngine::new();
        
        let result = engine.evaluate_expression("42").unwrap();
        assert!(matches!(result, ShellObject::Integer(42)));
        
        let result = engine.evaluate_expression("3.14").unwrap();
        assert!(matches!(result, ShellObject::Float(f) if (f - 3.14).abs() < f64::EPSILON));
        
        let result = engine.evaluate_expression("\"hello\"").unwrap();
        assert!(matches!(result, ShellObject::String(s) if s == "hello"));
        
        let result = engine.evaluate_expression("true").unwrap();
        assert!(matches!(result, ShellObject::Boolean(true)));
    }

    #[test]
    fn test_arithmetic_operations() {
        let engine = EvaluationEngine::new();
        
        let result = engine.evaluate_expression("2 + 3").unwrap();
        assert!(matches!(result, ShellObject::Integer(5)));
        
        let result = engine.evaluate_expression("10 - 4").unwrap();
        assert!(matches!(result, ShellObject::Integer(6)));
        
        let result = engine.evaluate_expression("6 * 7").unwrap();
        assert!(matches!(result, ShellObject::Integer(42)));
        
        let result = engine.evaluate_expression("15 / 3").unwrap();
        assert!(matches!(result, ShellObject::Float(f) if (f - 5.0).abs() < f64::EPSILON));
    }

    #[test]
    fn test_string_operations() {
        let engine = EvaluationEngine::new();
        
        let result = engine.evaluate_expression("\"hello\" + \" world\"").unwrap();
        assert!(matches!(result, ShellObject::String(s) if s == "hello world"));
    }

    #[test]
    fn test_array_operations() {
        let engine = EvaluationEngine::new();
        
        let result = engine.evaluate_expression("[1, 2, 3]").unwrap();
        if let ShellObject::Array(arr) = result {
            assert_eq!(arr.len(), 3);
            assert!(matches!(arr[0], ShellObject::Integer(1)));
            assert!(matches!(arr[1], ShellObject::Integer(2)));
            assert!(matches!(arr[2], ShellObject::Integer(3)));
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_comparison_operations() {
        let engine = EvaluationEngine::new();
        
        let result = engine.evaluate_expression("5 > 3").unwrap();
        assert!(matches!(result, ShellObject::Boolean(true)));
        
        let result = engine.evaluate_expression("2 == 2").unwrap();
        assert!(matches!(result, ShellObject::Boolean(true)));
        
        let result = engine.evaluate_expression("\"abc\" < \"def\"").unwrap();
        assert!(matches!(result, ShellObject::Boolean(true)));
    }

    #[test]
    fn test_variables() {
        let mut engine = EvaluationEngine::new();
        engine.set_variable("x".to_string(), ShellObject::Integer(42));
        
        let result = engine.evaluate_expression("x + 8").unwrap();
        assert!(matches!(result, ShellObject::Integer(50)));
    }
}