// Test file for rust-analyzer MCP tools that need specific scenarios

use std::fmt;

// Define a trait for testing implementations
pub trait MyTrait {
    fn do_something(&self) -> String;
    fn do_another(&self, value: i32) -> i32;
}

// Struct that implements the trait
pub struct MyStruct {
    pub name: String,
}

// Implementation of MyTrait for MyStruct
impl MyTrait for MyStruct {
    fn do_something(&self) -> String {
        format!("MyStruct: {}", self.name)
    }
    
    fn do_another(&self, value: i32) -> i32 {
        value * 2
    }
}

// Another struct implementing the same trait
pub struct AnotherStruct {
    pub id: u32,
}

impl MyTrait for AnotherStruct {
    fn do_something(&self) -> String {
        format!("AnotherStruct: {}", self.id)
    }
    
    fn do_another(&self, value: i32) -> i32 {
        value + 10
    }
}

// Function with multiple parameters for signature help testing
pub fn complex_function(name: &str, age: u32, active: bool) -> String {
    format!("Name: {}, Age: {}, Active: {}", name, age, active)
}

// Function that uses the trait
pub fn use_trait_object(obj: &dyn MyTrait) -> String {
    obj.do_something()
}

// Test function with various scenarios
pub fn test_function() {
    let my_struct = MyStruct {
        name: "Test".to_string(),
    };
    
    // Test goto_definition - should work on MyStruct
    let result = my_struct.do_something();
    
    // Test signature help - cursor inside function call
    let output = complex_function("John", 30, true);
    
    // Test code actions - intentional error for quick fix
    // let x: i32 = "not a number";  // Type mismatch error
    
    // Use trait
    let trait_result = use_trait_object(&my_struct);
}

// Test implementations - multiple implementations of Display
impl fmt::Display for MyStruct {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MyStruct({})", self.name)
    }
}

impl fmt::Display for AnotherStruct {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AnotherStruct({})", self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_my_trait() {
        let s = MyStruct {
            name: "test".to_string(),
        };
        assert_eq!(s.do_something(), "MyStruct: test");
    }
}