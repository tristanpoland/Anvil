use crate::error::{AnvilError, AnvilResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::time::SystemTime;

/// Core trait for all shell objects
pub trait ShellObjectTrait: fmt::Debug + Send + Sync {
    /// Get the type name of this object
    fn type_name(&self) -> &'static str;
    
    /// Get a field value by name
    fn get_field(&self, name: &str) -> AnvilResult<ShellObject>;
    
    /// Set a field value by name
    fn set_field(&mut self, name: &str, value: ShellObject) -> AnvilResult<()>;
    
    /// List all available field names
    fn field_names(&self) -> Vec<String>;
    
    /// Convert to a display string
    fn to_display_string(&self) -> String;
    
    /// Check if this object can be called as a function
    fn is_callable(&self) -> bool { false }
    
    /// Call this object as a function with arguments
    fn call(&self, _args: Vec<ShellObject>) -> AnvilResult<ShellObject> {
        Err(AnvilError::unsupported(format!("{} is not callable", self.type_name())))
    }
}

/// Universal shell object that can hold any type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShellObject {
    // Primitive types
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Unit,
    
    // Collections
    Array(Vec<ShellObject>),
    Map(HashMap<String, ShellObject>),
    
    // File system objects
    File(FileObject),
    Directory(DirectoryObject),
    Path(PathObject),
    
    // Process objects
    Process(ProcessObject),
    Command(CommandObject),
    
    // System objects
    Environment(EnvironmentObject),
    
    // Function objects
    Function(FunctionObject),
    
    // Error object
    Error(String),
}

impl ShellObject {
    pub fn type_name(&self) -> &'static str {
        match self {
            ShellObject::String(_) => "String",
            ShellObject::Integer(_) => "Integer", 
            ShellObject::Float(_) => "Float",
            ShellObject::Boolean(_) => "Boolean",
            ShellObject::Unit => "Unit",
            ShellObject::Array(_) => "Array",
            ShellObject::Map(_) => "Map",
            ShellObject::File(_) => "File",
            ShellObject::Directory(_) => "Directory",
            ShellObject::Path(_) => "Path",
            ShellObject::Process(_) => "Process",
            ShellObject::Command(_) => "Command",
            ShellObject::Environment(_) => "Environment",
            ShellObject::Function(_) => "Function",
            ShellObject::Error(_) => "Error",
        }
    }

    pub fn get_field(&self, name: &str) -> AnvilResult<ShellObject> {
        match self {
            ShellObject::String(s) => match name {
                "length" => Ok(ShellObject::Integer(s.len() as i64)),
                "chars" => Ok(ShellObject::Array(s.chars().map(|c| ShellObject::String(c.to_string())).collect())),
                "bytes" => Ok(ShellObject::Array(s.bytes().map(|b| ShellObject::Integer(b as i64)).collect())),
                "is_empty" => Ok(ShellObject::Boolean(s.is_empty())),
                _ => Err(AnvilError::object(format!("String has no field '{}'", name))),
            },
            ShellObject::Array(arr) => match name {
                "length" => Ok(ShellObject::Integer(arr.len() as i64)),
                "is_empty" => Ok(ShellObject::Boolean(arr.is_empty())),
                "first" => arr.first().cloned().ok_or_else(|| AnvilError::object("Array is empty")),
                "last" => arr.last().cloned().ok_or_else(|| AnvilError::object("Array is empty")),
                _ => Err(AnvilError::object(format!("Array has no field '{}'", name))),
            },
            ShellObject::File(file) => file.get_field(name),
            ShellObject::Directory(dir) => dir.get_field(name),
            ShellObject::Path(path) => path.get_field(name),
            ShellObject::Process(proc) => proc.get_field(name),
            ShellObject::Command(cmd) => cmd.get_field(name),
            ShellObject::Environment(env) => env.get_field(name),
            ShellObject::Map(map) => {
                map.get(name).cloned().ok_or_else(|| AnvilError::object(format!("Map has no field '{}'", name)))
            },
            _ => Err(AnvilError::object(format!("{} has no fields", self.type_name()))),
        }
    }

    pub fn field_names(&self) -> Vec<String> {
        match self {
            ShellObject::String(_) => vec!["length".to_string(), "chars".to_string(), "bytes".to_string(), "is_empty".to_string()],
            ShellObject::Array(_) => vec!["length".to_string(), "is_empty".to_string(), "first".to_string(), "last".to_string()],
            ShellObject::File(file) => file.field_names(),
            ShellObject::Directory(dir) => dir.field_names(),
            ShellObject::Path(path) => path.field_names(),
            ShellObject::Process(proc) => proc.field_names(),
            ShellObject::Command(cmd) => cmd.field_names(),
            ShellObject::Environment(env) => env.field_names(),
            ShellObject::Map(map) => map.keys().cloned().collect(),
            _ => vec![],
        }
    }

    pub fn to_display_string(&self) -> String {
        match self {
            ShellObject::String(s) => s.clone(),
            ShellObject::Integer(i) => i.to_string(),
            ShellObject::Float(f) => f.to_string(),
            ShellObject::Boolean(b) => b.to_string(),
            ShellObject::Unit => "()".to_string(),
            ShellObject::Array(arr) => {
                let items: Vec<String> = arr.iter().map(|obj| obj.to_display_string()).collect();
                format!("[{}]", items.join(", "))
            },
            ShellObject::Map(map) => {
                let items: Vec<String> = map.iter()
                    .map(|(k, v)| format!("{}: {}", k, v.to_display_string()))
                    .collect();
                format!("{{{}}}", items.join(", "))
            },
            ShellObject::File(file) => file.to_display_string(),
            ShellObject::Directory(dir) => dir.to_display_string(),
            ShellObject::Path(path) => path.to_display_string(),
            ShellObject::Process(proc) => proc.to_display_string(),
            ShellObject::Command(cmd) => cmd.to_display_string(),
            ShellObject::Environment(env) => env.to_display_string(),
            ShellObject::Function(func) => func.to_display_string(),
            ShellObject::Error(err) => format!("Error: {}", err),
        }
    }

    /// Convert Rust types to ShellObject
    pub fn from_rust_value<T: Into<ShellObject>>(value: T) -> ShellObject {
        value.into()
    }
}

// Implement conversions from Rust types
impl From<String> for ShellObject {
    fn from(s: String) -> Self { ShellObject::String(s) }
}

impl From<&str> for ShellObject {
    fn from(s: &str) -> Self { ShellObject::String(s.to_string()) }
}

impl From<i64> for ShellObject {
    fn from(i: i64) -> Self { ShellObject::Integer(i) }
}

impl From<f64> for ShellObject {
    fn from(f: f64) -> Self { ShellObject::Float(f) }
}

impl From<bool> for ShellObject {
    fn from(b: bool) -> Self { ShellObject::Boolean(b) }
}

impl From<Vec<ShellObject>> for ShellObject {
    fn from(arr: Vec<ShellObject>) -> Self { ShellObject::Array(arr) }
}

impl From<HashMap<String, ShellObject>> for ShellObject {
    fn from(map: HashMap<String, ShellObject>) -> Self { ShellObject::Map(map) }
}

// File system objects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileObject {
    pub path: PathBuf,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub permissions: String,
}

impl FileObject {
    pub fn get_field(&self, name: &str) -> AnvilResult<ShellObject> {
        match name {
            "path" => Ok(ShellObject::String(self.path.to_string_lossy().to_string())),
            "name" => Ok(ShellObject::String(
                self.path.file_name().unwrap_or_default().to_string_lossy().to_string()
            )),
            "extension" => Ok(ShellObject::String(
                self.path.extension().unwrap_or_default().to_string_lossy().to_string()
            )),
            "size" => Ok(ShellObject::Integer(self.size as i64)),
            "permissions" => Ok(ShellObject::String(self.permissions.clone())),
            _ => Err(AnvilError::object(format!("File has no field '{}'", name))),
        }
    }

    pub fn field_names(&self) -> Vec<String> {
        vec!["path".to_string(), "name".to_string(), "extension".to_string(), "size".to_string(), "permissions".to_string()]
    }

    pub fn to_display_string(&self) -> String {
        format!("File({})", self.path.display())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryObject {
    pub path: PathBuf,
    pub entries: Vec<String>,
}

impl DirectoryObject {
    pub fn get_field(&self, name: &str) -> AnvilResult<ShellObject> {
        match name {
            "path" => Ok(ShellObject::String(self.path.to_string_lossy().to_string())),
            "name" => Ok(ShellObject::String(
                self.path.file_name().unwrap_or_default().to_string_lossy().to_string()
            )),
            "entries" => Ok(ShellObject::Array(
                self.entries.iter().map(|e| ShellObject::String(e.clone())).collect()
            )),
            "count" => Ok(ShellObject::Integer(self.entries.len() as i64)),
            _ => Err(AnvilError::object(format!("Directory has no field '{}'", name))),
        }
    }

    pub fn field_names(&self) -> Vec<String> {
        vec!["path".to_string(), "name".to_string(), "entries".to_string(), "count".to_string()]
    }

    pub fn to_display_string(&self) -> String {
        format!("Directory({})", self.path.display())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathObject {
    pub path: PathBuf,
}

impl PathObject {
    pub fn get_field(&self, name: &str) -> AnvilResult<ShellObject> {
        match name {
            "path" => Ok(ShellObject::String(self.path.to_string_lossy().to_string())),
            "parent" => Ok(ShellObject::String(
                self.path.parent().unwrap_or(&self.path).to_string_lossy().to_string()
            )),
            "filename" => Ok(ShellObject::String(
                self.path.file_name().unwrap_or_default().to_string_lossy().to_string()
            )),
            "extension" => Ok(ShellObject::String(
                self.path.extension().unwrap_or_default().to_string_lossy().to_string()
            )),
            "exists" => Ok(ShellObject::Boolean(self.path.exists())),
            "is_file" => Ok(ShellObject::Boolean(self.path.is_file())),
            "is_dir" => Ok(ShellObject::Boolean(self.path.is_dir())),
            _ => Err(AnvilError::object(format!("Path has no field '{}'", name))),
        }
    }

    pub fn field_names(&self) -> Vec<String> {
        vec!["path".to_string(), "parent".to_string(), "filename".to_string(), 
             "extension".to_string(), "exists".to_string(), "is_file".to_string(), "is_dir".to_string()]
    }

    pub fn to_display_string(&self) -> String {
        self.path.to_string_lossy().to_string()
    }
}

// Process objects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessObject {
    pub pid: u32,
    pub name: String,
    pub command: String,
    pub status: String,
}

impl ProcessObject {
    pub fn get_field(&self, name: &str) -> AnvilResult<ShellObject> {
        match name {
            "pid" => Ok(ShellObject::Integer(self.pid as i64)),
            "name" => Ok(ShellObject::String(self.name.clone())),
            "command" => Ok(ShellObject::String(self.command.clone())),
            "status" => Ok(ShellObject::String(self.status.clone())),
            _ => Err(AnvilError::object(format!("Process has no field '{}'", name))),
        }
    }

    pub fn field_names(&self) -> Vec<String> {
        vec!["pid".to_string(), "name".to_string(), "command".to_string(), "status".to_string()]
    }

    pub fn to_display_string(&self) -> String {
        format!("Process(pid={}, name={})", self.pid, self.name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandObject {
    pub name: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
}

impl CommandObject {
    pub fn get_field(&self, name: &str) -> AnvilResult<ShellObject> {
        match name {
            "name" => Ok(ShellObject::String(self.name.clone())),
            "args" => Ok(ShellObject::Array(
                self.args.iter().map(|a| ShellObject::String(a.clone())).collect()
            )),
            "env" => Ok(ShellObject::Map(
                self.env.iter().map(|(k, v)| (k.clone(), ShellObject::String(v.clone()))).collect()
            )),
            _ => Err(AnvilError::object(format!("Command has no field '{}'", name))),
        }
    }

    pub fn field_names(&self) -> Vec<String> {
        vec!["name".to_string(), "args".to_string(), "env".to_string()]
    }

    pub fn to_display_string(&self) -> String {
        format!("Command({})", self.name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentObject {
    pub vars: HashMap<String, String>,
}

impl EnvironmentObject {
    pub fn get_field(&self, name: &str) -> AnvilResult<ShellObject> {
        self.vars.get(name)
            .map(|v| ShellObject::String(v.clone()))
            .ok_or_else(|| AnvilError::object(format!("Environment variable '{}' not found", name)))
    }

    pub fn field_names(&self) -> Vec<String> {
        self.vars.keys().cloned().collect()
    }

    pub fn to_display_string(&self) -> String {
        format!("Environment({} vars)", self.vars.len())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionObject {
    pub name: String,
    pub signature: String,
    pub body: String,
}

impl FunctionObject {
    pub fn get_field(&self, name: &str) -> AnvilResult<ShellObject> {
        match name {
            "name" => Ok(ShellObject::String(self.name.clone())),
            "signature" => Ok(ShellObject::String(self.signature.clone())),
            "body" => Ok(ShellObject::String(self.body.clone())),
            _ => Err(AnvilError::object(format!("Function has no field '{}'", name))),
        }
    }

    pub fn field_names(&self) -> Vec<String> {
        vec!["name".to_string(), "signature".to_string(), "body".to_string()]
    }

    pub fn to_display_string(&self) -> String {
        format!("Function({})", self.name)
    }
}

impl fmt::Display for ShellObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_display_string())
    }
}