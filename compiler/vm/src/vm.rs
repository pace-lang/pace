use std::collections::HashMap;
use mir::{BlockId, Inst, Place, RValue, Terminator, Value, Program};
use ast::{BinaryOp, UnaryOp};

pub struct Frame {
    pub memory_vars: HashMap<String, Value>,
    pub memory_temps: HashMap<usize, Value>,
}

#[derive(Debug, Clone)]
pub struct Object {
    pub class_name: String,
    pub fields: HashMap<String, Value>,
}

pub struct VirtualMachine<'a> {
    program: &'a Program,
    frames: Vec<Frame>,
    heap: Vec<Object>,
    maps: HashMap<usize, Vec<(Value, Value)>>,
    next_map_id: usize,
    files: HashMap<usize, std::fs::File>,
    next_file_id: usize,
}

impl<'a> VirtualMachine<'a> {
    pub fn new(program: &'a Program) -> Self {
        Self {
            program,
            frames: Vec::new(),
            heap: Vec::new(),
            maps: HashMap::new(),
            next_map_id: 1,
            files: HashMap::new(),
            next_file_id: 1,
        }
    }

    pub fn dump_memory(&self) {
        println!("--- VM Memory Dump ---");
        if let Some(frame) = self.frames.last() {
            for (k, v) in &frame.memory_vars {
                println!("{} = {:?}", k, v);
            }
        }
        println!("----------------------");
    }

    pub fn execute(&mut self) -> Option<Value> {
        self.call_function("main", &[])
    }

    pub fn call_function(&mut self, name: &str, args: &[Value]) -> Option<Value> {
        if name == "print" {
            let mut out = String::new();
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                let resolved = match arg {
                    Value::Place(_) => panic!("Pass evaluated arguments to print"),
                    _ => arg.clone()
                };
                match resolved {
                    Value::Int(v) => out.push_str(&v.to_string()),
                    Value::Float(v) => out.push_str(&v.to_string()),
                    Value::String(v) => out.push_str(&v),
                    Value::Boolean(v) => out.push_str(&v.to_string()),
                    Value::Object(id) => {
                        let obj = &self.heap[id];
                        out.push_str(&format!("<{} object at {}>", obj.class_name, id));
                    },
                    Value::Void => out.push_str("void"),
                    Value::Null => out.push_str("null"),
                    Value::Array(_) => out.push_str("[Array]"),
                    Value::EnumVariant(enum_name, tag, _payload) => out.push_str(&format!("<{} Variant {}>", enum_name, tag)),
                    Value::Place(_) => unreachable!(),
                }
            }
            println!("{}", out);
            return Some(Value::Void);
        }

        let base_name = name.split('_').next().unwrap_or(name);
        if let Some(foreign) = self.program.foreign_functions.get(name).or_else(|| self.program.foreign_functions.get(base_name)) {
            // Intercept standard library foreign functions natively in VM
            match base_name {
                // String methods
                "stringLen" => {
                    if let Value::String(s) = &args[0] {
                        return Some(Value::Int(s.len() as i64));
                    }
                }
                "stringConcat" => {
                    if let (Value::String(a), Value::String(b)) = (&args[0], &args[1]) {
                        return Some(Value::String(format!("{}{}", a, b)));
                    }
                }
                "stringSubstring" => {
                    if let (Value::String(s), Value::Int(start), Value::Int(end)) = (&args[0], &args[1], &args[2]) {
                        let st = (*start).max(0) as usize;
                        let en = (*end).max(0).min(s.len() as i64) as usize;
                        let sub = if st <= en && st < s.len() { s[st..en].to_string() } else { "".to_string() };
                        return Some(Value::String(sub));
                    }
                }
                "stringContains" => {
                    if let (Value::String(s), Value::String(sub)) = (&args[0], &args[1]) {
                        return Some(Value::Boolean(s.contains(sub)));
                    }
                }
                // Array methods (Arrays are represented as Objects with stringified integer keys and a "length" property)
                "arrayLen" => {
                    if let Value::Object(id) = &args[0] {
                        if let Some(obj) = self.heap.get(*id) {
                            if let Some(Value::Int(len)) = obj.fields.get("length") {
                                return Some(Value::Int(*len));
                            }
                        }
                    }
                }
                "arrayPush" => {
                    if let Value::Object(id) = &args[0] {
                        let item = args[1].clone();
                        if let Some(obj) = self.heap.get_mut(*id) {
                            if let Some(Value::Int(len)) = obj.fields.get("length").cloned() {
                                obj.fields.insert(len.to_string(), item);
                                obj.fields.insert("length".to_string(), Value::Int(len + 1));
                                return Some(Value::Void);
                            }
                        }
                    }
                }
                "arrayPop" => {
                    if let Value::Object(id) = &args[0] {
                        if let Some(obj) = self.heap.get_mut(*id) {
                            if let Some(Value::Int(len)) = obj.fields.get("length").cloned() {
                                if len > 0 {
                                    let item = obj.fields.remove(&(len - 1).to_string()).unwrap_or(Value::Void);
                                    obj.fields.insert("length".to_string(), Value::Int(len - 1));
                                    return Some(Value::EnumVariant("Option".to_string(), 0, vec![item])); // Some
                                } else {
                                    return Some(Value::EnumVariant("Option".to_string(), 1, vec![])); // None
                                }
                            }
                        }
                    }
                }
                "arrayGet" => {
                    if let (Value::Object(id), Value::Int(idx)) = (&args[0], &args[1]) {
                        if let Some(obj) = self.heap.get(*id) {
                            if let Some(Value::Int(len)) = obj.fields.get("length") {
                                if *idx >= 0 && *idx < *len {
                                    let item = obj.fields.get(&idx.to_string()).cloned().unwrap_or(Value::Void);
                                    return Some(Value::EnumVariant("Option".to_string(), 0, vec![item])); // Some
                                }
                            }
                            return Some(Value::EnumVariant("Option".to_string(), 1, vec![])); // None
                        }
                    }
                }
                "arraySet" => {
                    if let (Value::Object(id), Value::Int(idx)) = (&args[0], &args[1]) {
                        let item = args[2].clone();
                        if let Some(obj) = self.heap.get_mut(*id) {
                            if let Some(Value::Int(len)) = obj.fields.get("length") {
                                if *idx >= 0 && *idx < *len {
                                    obj.fields.insert(idx.to_string(), item);
                                }
                            }
                        }
                        return Some(Value::Void);
                    }
                }
                // Map methods
                "mapInit" => {
                    let id = self.next_map_id;
                    self.next_map_id += 1;
                    self.maps.insert(id, Vec::new());
                    return Some(Value::Int(id as i64)); // Return ID as pseudo-pointer
                }
                "mapSet" => {
                    if let Value::Int(id) = &args[0] {
                        let key = args[1].clone();
                        let value = args[2].clone();
                        if let Some(map) = self.maps.get_mut(&(*id as usize)) {
                            if let Some(pos) = map.iter().position(|(k, _)| *k == key) {
                                map[pos] = (key, value);
                            } else {
                                map.push((key, value));
                            }
                        }
                        return Some(Value::Void);
                    }
                }
                "mapGet" => {
                    if let Value::Int(id) = &args[0] {
                        let key = args[1].clone();
                        if let Some(map) = self.maps.get(&(*id as usize)) {
                            if let Some((_, value)) = map.iter().find(|(k, _)| *k == key) {
                                return Some(Value::EnumVariant("Option".to_string(), 0, vec![value.clone()])); // Some
                            }
                        }
                        return Some(Value::EnumVariant("Option".to_string(), 1, vec![])); // None
                    }
                }
                "mapRemove" => {
                    if let Value::Int(id) = &args[0] {
                        let key = args[1].clone();
                        if let Some(map) = self.maps.get_mut(&(*id as usize)) {
                            if let Some(pos) = map.iter().position(|(k, _)| *k == key) {
                                map.remove(pos);
                            }
                        }
                        return Some(Value::Void);
                    }
                }
                "mapContains" => {
                    if let Value::Int(id) = &args[0] {
                        let key = args[1].clone();
                        if let Some(map) = self.maps.get(&(*id as usize)) {
                            return Some(Value::Boolean(map.iter().any(|(k, _)| *k == key)));
                        }
                        return Some(Value::Boolean(false));
                    }
                }
                // File methods
                "fileOpen" => {
                    if let (Value::String(path), Value::String(mode)) = (&args[0], &args[1]) {
                        use std::fs::OpenOptions;
                        let mut options = OpenOptions::new();
                        if mode == "r" { options.read(true); }
                        else if mode == "w" { options.write(true).create(true).truncate(true); }
                        else if mode == "a" { options.write(true).create(true).append(true); }
                        
                        if let Ok(file) = options.open(path) {
                            let id = self.next_file_id;
                            self.next_file_id += 1;
                            self.files.insert(id, file);
                            return Some(Value::Int(id as i64));
                        }
                        return Some(Value::Int(0)); // Null pointer on failure
                    }
                }
                "fileReadAll" => {
                    if let Value::Int(id) = &args[0] {
                        use std::io::Read;
                        if let Some(file) = self.files.get_mut(&(*id as usize)) {
                            let mut buf = String::new();
                            if file.read_to_string(&mut buf).is_ok() {
                                return Some(Value::String(buf));
                            }
                        }
                    }
                    return Some(Value::String("".to_string()));
                }
                "fileWrite" => {
                    if let (Value::Int(id), Value::String(data)) = (&args[0], &args[1]) {
                        use std::io::Write;
                        if let Some(file) = self.files.get_mut(&(*id as usize)) {
                            if file.write_all(data.as_bytes()).is_ok() {
                                return Some(Value::Int(data.len() as i64));
                            }
                        }
                    }
                    return Some(Value::Int(0));
                }
                "fileClose" => {
                    if let Value::Int(id) = &args[0] {
                        self.files.remove(&(*id as usize));
                    }
                    return Some(Value::Void);
                }
                "fileIsValid" => {
                    if let Value::Int(id) = &args[0] {
                        return Some(Value::Boolean(*id != 0 && self.files.contains_key(&(*id as usize))));
                    }
                    return Some(Value::Boolean(false));
                }
                _ => {}
            }

            // Fallback for missing foreign function implementation
            if let Some(ret_ty) = &foreign.return_type {
                return Some(match ret_ty {
                    mir::ForeignAbiType::I8 | mir::ForeignAbiType::I16 | mir::ForeignAbiType::I32 | mir::ForeignAbiType::I64 => Value::Int(0),
                    mir::ForeignAbiType::F32 | mir::ForeignAbiType::F64 => Value::Float(0.0),
                    mir::ForeignAbiType::Pointer => Value::Null,
                });
            }
            return Some(Value::Void);
        }

        let function = self.program.functions.get(name).unwrap_or_else(|| panic!("Function '{}' not found", name));
        
        let mut frame = Frame {
            memory_vars: HashMap::new(),
            memory_temps: HashMap::new(),
        };

        // Map arguments to parameters
        for (i, param_name) in function.parameters.iter().enumerate() {
            frame.memory_vars.insert(param_name.clone(), args[i].clone());
        }

        self.frames.push(frame);

        let mut current_block_id = BlockId(0);

        loop {
            let block = function.blocks.iter().find(|b| b.id == current_block_id)
                .expect("Block not found");

            for inst in &block.instructions {
                self.execute_inst(inst);
            }

            match &block.terminator {
                Some(Terminator::Jump(next_block)) => {
                    current_block_id = *next_block;
                }
                Some(Terminator::Branch { cond, then_block, else_block }) => {
                    let cond_val = self.resolve_value(cond);
                    if let Value::Boolean(b) = cond_val {
                        if b {
                            current_block_id = *then_block;
                        } else {
                            current_block_id = *else_block;
                        }
                    } else {
                        panic!("Branch condition must be a Boolean.");
                    }
                }
                Some(Terminator::Switch { cond, cases, default }) => {
                    let cond_val = self.resolve_value(cond);
                    if let Value::Int(tag) = cond_val {
                        let mut matched = false;
                        for (case_tag, block_id) in cases {
                            if *case_tag == tag as usize {
                                current_block_id = *block_id;
                                matched = true;
                                break;
                            }
                        }
                        if !matched {
                            if let Some(def_block) = default {
                                current_block_id = *def_block;
                            } else {
                                panic!("Switch matched no cases and has no default block.");
                            }
                        }
                    } else {
                        panic!("Switch condition must be an Int tag.");
                    }
                }
                Some(Terminator::Return(val_opt)) => {
                    let ret = val_opt.as_ref().map(|v| self.resolve_value(v));
                    self.frames.pop();
                    return ret;
                }
                None => {
                    panic!("Block has no terminator!");
                }
            }
        }
    }

    fn execute_inst(&mut self, inst: &Inst) {
        match inst {
            Inst::Assign(place, rvalue) => {
                let val = match rvalue {
                    RValue::Use(v) => self.resolve_value(v),
                    RValue::UnaryOp(op, right) => self.eval_unary(op.clone(), right),
                    RValue::BinaryOp(op, left, right) => self.eval_binary(op.clone(), left, right),
                    RValue::AllocateObject(class_name) => {
                        let id = self.heap.len();
                        self.heap.push(Object {
                            class_name: class_name.clone(),
                            fields: HashMap::new(),
                        });
                        Value::Object(id)
                    }
                    RValue::GetProperty(object_place, name) => {
                        let obj_val = self.resolve_value(object_place);
                        if let Value::Object(id) = obj_val {
                            if let Some(obj) = self.heap.get(id) {
                                obj.fields.get(name).cloned().unwrap_or(Value::Void)
                            } else {
                                panic!("Object ID {} not found in heap", id);
                            }
                        } else {
                            panic!("Cannot get property on non-object");
                        }
                    }
                    RValue::WeakUpgrade(_) => {
                        // Not fully supported in naive VM
                        Value::Void
                    }
                    RValue::MethodCall(object_place, method_name, args) => {
                        let obj_val = self.resolve_value(object_place);
                        if let Value::Object(id) = obj_val {
                            let class_name = self.heap.get(id).unwrap().class_name.clone();
                            let func_name = format!("{}::{}", class_name, method_name);
                            
                            let mut resolved_args = vec![Value::Object(id)];
                            for arg in args {
                                resolved_args.push(self.resolve_value(arg));
                            }
                            
                            self.call_function(&func_name, &resolved_args).unwrap_or(Value::Void)
                        } else {
                            panic!("Cannot call method on non-object");
                        }
                    }
                    RValue::Call(func_name, args) => {
                        let mut resolved_args = Vec::new();
                        for arg in args {
                            resolved_args.push(self.resolve_value(arg));
                        }
                        
                        if self.program.classes.contains_key(func_name) {
                            let id = self.heap.len();
                            self.heap.push(Object {
                                class_name: func_name.clone(),
                                fields: HashMap::new(),
                            });
                            
                            let class_def = &self.program.classes[func_name];
                            for field in &class_def.fields {
                                self.heap[id].fields.insert(field.clone(), Value::Void);
                            }
                            
                            let init_func = format!("{}::init", func_name);
                            if self.program.functions.contains_key(&init_func) {
                                let mut init_args = vec![Value::Object(id)];
                                init_args.extend(resolved_args);
                                self.call_function(&init_func, &init_args);
                            }
                            
                            Value::Object(id)
                        } else {
                            self.call_function(func_name, &resolved_args).unwrap_or(Value::Void)
                        }
                    }
                    RValue::ForceUnwrap(val) => {
                        let eval_val = self.resolve_value(val);
                        if eval_val == Value::Null {
                            panic!("Fatal Error: Unexpectedly found null while unwrapping an Optional value.");
                        }
                        eval_val
                    }
                    RValue::Array(elements, _) => {
                        let id = self.heap.len();
                        self.heap.push(Object {
                            class_name: "[Array]".to_string(),
                            fields: HashMap::new(),
                        });
                        for (i, elem) in elements.iter().enumerate() {
                            let val = self.resolve_value(elem);
                            self.heap[id].fields.insert(i.to_string(), val);
                        }
                        self.heap[id].fields.insert("length".to_string(), Value::Int(elements.len() as i64));
                        Value::Object(id)
                    }
                    RValue::ArrayRepeat(val, count, _) => {
                        let id = self.heap.len();
                        let repeat_val = self.resolve_value(val);
                        let count_val = self.resolve_value(count);
                        let count_int = if let Value::Int(c) = count_val { c } else { panic!("Count must be int") };
                        self.heap.push(Object {
                            class_name: "[Array]".to_string(),
                            fields: HashMap::new(),
                        });
                        for i in 0..count_int {
                            self.heap[id].fields.insert(i.to_string(), repeat_val.clone());
                        }
                        self.heap[id].fields.insert("length".to_string(), Value::Int(count_int));
                        Value::Object(id)
                    }
                    RValue::IndexGet(array, index) => {
                        let obj_val = self.resolve_value(array);
                        let idx_val = self.resolve_value(index);
                        let idx_int = if let Value::Int(i) = idx_val { i } else { panic!("Index must be int") };
                        if let Value::Object(id) = obj_val {
                            let obj = &self.heap[id];
                            let len_val = obj.fields.get("length").unwrap();
                            let len_int = if let Value::Int(l) = len_val { *l } else { panic!("Length must be int") };
                            if idx_int < 0 || idx_int >= len_int {
                                panic!("Index out of bounds");
                            }
                            obj.fields.get(&idx_int.to_string()).cloned().unwrap_or(Value::Void)
                        } else {
                            panic!("Cannot index non-object");
                        }
                    }
                    RValue::ConstructVariant(enum_name, tag, payload) => {
                        let mut resolved_payload = Vec::new();
                        for p in payload {
                            resolved_payload.push(self.resolve_value(p));
                        }
                        Value::EnumVariant(enum_name.clone(), *tag, resolved_payload)
                    }
                    RValue::ExtractPayload(val, expected_tag, field_idx) => {
                        let resolved_val = self.resolve_value(val);
                        if let Value::EnumVariant(_, tag, payload) = resolved_val {
                            if tag != *expected_tag {
                                panic!("Variant tag mismatch in ExtractPayload: expected {}, got {}", expected_tag, tag);
                            }
                            payload[*field_idx].clone()
                        } else {
                            panic!("Cannot ExtractPayload from non-EnumVariant");
                        }
                    }
                    RValue::GetVariantTag(val) => {
                        let resolved_val = self.resolve_value(val);
                        if let Value::EnumVariant(_, tag, _) = resolved_val {
                            Value::Int(tag as i64)
                        } else {
                            panic!("Cannot GetVariantTag from non-EnumVariant");
                        }
                    }
                };
                self.store(place, val);
            }
            Inst::SetProperty(object_place, name, value) => {
                let obj_val = self.resolve_value(object_place);
                let val = self.resolve_value(value);
                if let Value::Object(id) = obj_val {
                    if let Some(obj) = self.heap.get_mut(id) {
                        obj.fields.insert(name.clone(), val);
                    } else {
                        panic!("Object ID {} not found in heap", id);
                    }
                } else {
                    panic!("Cannot set property on non-object");
                }
            }
            Inst::IndexSet(array, index, val) => {
                let obj_val = self.resolve_value(array);
                let idx_val = self.resolve_value(index);
                let set_val = self.resolve_value(val);
                let idx_int = if let Value::Int(i) = idx_val { i } else { panic!("Index must be int") };
                if let Value::Object(id) = obj_val {
                    if let Some(obj) = self.heap.get_mut(id) {
                        let len_val = obj.fields.get("length").unwrap();
                        let len_int = if let Value::Int(l) = len_val { *l } else { panic!("Length must be int") };
                        if idx_int < 0 || idx_int >= len_int {
                            panic!("Index out of bounds");
                        }
                        obj.fields.insert(idx_int.to_string(), set_val);
                    }
                }
            }
            Inst::Retain(_) => {}
            Inst::Release(_) => {}
            Inst::WeakRetain(_) => {}
            Inst::WeakRelease(_) => {}
        }
    }

    fn resolve_value(&self, value: &Value) -> Value {
        match value {
            Value::Place(place) => self.load(place),
            _ => value.clone(),
        }
    }

    fn load(&self, place: &Place) -> Value {
        let frame = self.frames.last().expect("No active frame");
        match place {
            Place::Var(name) => frame.memory_vars.get(name).cloned().expect("Variable not found in VM memory"),
            Place::Temp(id) => frame.memory_temps.get(id).cloned().expect("Temp not found in VM memory"),
        }
    }

    fn store(&mut self, place: &Place, value: Value) {
        let frame = self.frames.last_mut().expect("No active frame");
        match place {
            Place::Var(name) => { frame.memory_vars.insert(name.clone(), value); },
            Place::Temp(id) => { frame.memory_temps.insert(*id, value); },
        }
    }

    fn eval_unary(&self, op: UnaryOp, right: &Value) -> Value {
        let r = self.resolve_value(right);
        match op {
            UnaryOp::Negate => match r {
                Value::Int(i) => Value::Int(-i),
                Value::Float(f) => Value::Float(-f),
                _ => panic!("Cannot negate non-numeric value"),
            }
        }
    }
    // No longer need eval_call as it is in call_function

    fn eval_binary(&self, op: BinaryOp, left: &Value, right: &Value) -> Value {
        let l = self.resolve_value(left);
        let r = self.resolve_value(right);

        match op {
            BinaryOp::Add => match (l, r) {
                (Value::Int(a), Value::Int(b)) => Value::Int(a + b),
                (Value::Float(a), Value::Float(b)) => Value::Float(a + b),
                (Value::String(a), Value::String(b)) => Value::String(format!("{}{}", a, b)),
                _ => panic!("Invalid operand types for Add"),
            },
            BinaryOp::Subtract => match (l, r) {
                (Value::Int(a), Value::Int(b)) => Value::Int(a - b),
                (Value::Float(a), Value::Float(b)) => Value::Float(a - b),
                _ => panic!("Invalid operand types for Subtract"),
            },
            BinaryOp::Multiply => match (l, r) {
                (Value::Int(a), Value::Int(b)) => Value::Int(a * b),
                (Value::Float(a), Value::Float(b)) => Value::Float(a * b),
                _ => panic!("Invalid operand types for Multiply"),
            },
            BinaryOp::Divide => match (l, r) {
                (Value::Int(a), Value::Int(b)) => Value::Int(a / b),
                (Value::Float(a), Value::Float(b)) => Value::Float(a / b),
                _ => panic!("Invalid operand types for Divide"),
            },
            BinaryOp::Equal => Value::Boolean(l == r),
            BinaryOp::NotEqual => Value::Boolean(l != r),
            BinaryOp::Less => match (l, r) {
                (Value::Int(a), Value::Int(b)) => Value::Boolean(a < b),
                (Value::Float(a), Value::Float(b)) => Value::Boolean(a < b),
                _ => panic!("Invalid operand types for Less"),
            },
            BinaryOp::LessEqual => match (l, r) {
                (Value::Int(a), Value::Int(b)) => Value::Boolean(a <= b),
                (Value::Float(a), Value::Float(b)) => Value::Boolean(a <= b),
                _ => panic!("Invalid operand types for LessEqual"),
            },
            BinaryOp::Greater => match (l, r) {
                (Value::Int(a), Value::Int(b)) => Value::Boolean(a > b),
                (Value::Float(a), Value::Float(b)) => Value::Boolean(a > b),
                _ => panic!("Invalid operand types for Greater"),
            },
            BinaryOp::GreaterEqual => match (l, r) {
                (Value::Int(a), Value::Int(b)) => Value::Boolean(a >= b),
                (Value::Float(a), Value::Float(b)) => Value::Boolean(a >= b),
                _ => panic!("Invalid operand types for GreaterEqual"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir::{BasicBlock, BlockId, Function, Inst, Place, RValue, Terminator, Value};
    use ast::BinaryOp;

    #[test]
    fn test_execute_math() {
        let mut fun = Function::new("main".into(), vec![], std::collections::HashSet::new(), false);
        let mut block0 = BasicBlock::new(BlockId(0));
        
        block0.instructions.push(Inst::Assign(Place::Temp(0), RValue::BinaryOp(BinaryOp::Add, Value::Int(10), Value::Int(5))));
        block0.instructions.push(Inst::Assign(Place::Var("x".into()), RValue::Use(Value::Place(Place::Temp(0)))));
        block0.terminator = Some(Terminator::Return(Some(Value::Place(Place::Var("x".into())))));
        
        fun.blocks.push(block0);

        let mut program = Program::new();
        program.functions.insert("main".into(), fun);

        let mut vm = VirtualMachine::new(&program);
        let result = vm.execute();

        assert_eq!(result, Some(Value::Int(15)));
    }
}
