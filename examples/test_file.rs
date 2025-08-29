// Example Rust file to test the language server MCP

use std::collections::HashMap;
use std::fmt::Display;

// Trait definition to test implementations tool
trait Greetable {
    fn greet(&self) -> String;
    fn introduce(&self) -> String;
}

// Struct that implements the trait
struct User {
    name: String,
    age: u32,
    email: String,
}

// Another struct that implements the same trait
struct Guest {
    name: String,
}

impl User {
    fn new(name: String, age: u32, email: String) -> Self {
        User { name, age, email }
    }
}

// Implementation of Greetable trait for User
impl Greetable for User {
    fn greet(&self) -> String {
        format!("Hello, I'm {}", self.name)
    }

    fn introduce(&self) -> String {
        format!(
            "Hi! I'm {} and I'm {} years old. You can reach me at {}",
            self.name, self.age, self.email
        )
    }
}

// Implementation of Greetable trait for Guest
impl Greetable for Guest {
    fn greet(&self) -> String {
        format!("Hello, I'm {} (visiting)", self.name)
    }

    fn introduce(&self) -> String {
        format!("Hi! I'm {} and I'm just visiting", self.name)
    }
}

// Also implement Display for User to test multiple trait implementations
impl Display for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.age)
    }
}

fn main() {
    // Create a new user
    let user = User::new("Alice".to_string(), 30, "alice@example.com".to_string());

    // Test hover: hover over 'user' to see its type
    println!("{}", user.greet());

    // Use the introduce method to avoid warning
    println!("{}", user.introduce());

    // Test completion: type 'user.' to see available methods
    let _message = user.greet();

    // Create a guest to avoid warning
    let guest = Guest {
        name: "Bob".to_string(),
    };
    println!("{}", guest.greet());

    // Test diagnostics: uncomment this line to see an error
    // let x: i32 = "not a number";

    // Test goto definition: click on HashMap to go to its definition
    let mut scores: HashMap<String, i32> = HashMap::new();
    scores.insert("Alice".to_string(), 100);

    // Use calculate_score function to avoid warning
    if let Some(score) = calculate_score(&scores, "Alice") {
        println!("Alice's score: {}", score);
    }

    // Test references: find all usages of the 'user' variable
    if user.age > 18 {
        println!("{} is an adult", user.name);
        println!("Contact: {}", user.email); // Use email field to avoid warning
    }
}

// Helper function to demonstrate more features
fn calculate_score(scores: &HashMap<String, i32>, name: &str) -> Option<i32> {
    scores.get(name).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_creation() {
        let user = User::new("Test".to_string(), 25, "test@example.com".to_string());
        assert_eq!(user.name, "Test");
        assert_eq!(user.age, 25);
    }

    #[test]
    fn test_greet() {
        let user = User::new("Alice".to_string(), 30, "alice@example.com".to_string());
        assert_eq!(user.greet(), "Hello, I'm Alice");
    }

    #[test]
    fn test_calculate_score() {
        let mut scores = HashMap::new();
        scores.insert("Alice".to_string(), 100);
        assert_eq!(calculate_score(&scores, "Alice"), Some(100));
        assert_eq!(calculate_score(&scores, "Bob"), None);
    }
}

// Note: Benchmarks removed - use `cargo bench` with nightly Rust for benchmarking
