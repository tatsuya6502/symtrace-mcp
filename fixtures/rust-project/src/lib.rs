/// A named entity in the system.
pub trait Named {
    fn display_name(&self) -> &str;
}

/// A user with a name and age.
pub struct User {
    name: String,
    age: u32,
}

impl User {
    pub fn new(name: &str, age: u32) -> Self {
        Self { name: name.to_string(), age }
    }

    pub fn greet(&self) -> String {
        format!("Hello, {}!", self.name)
    }
}

impl Named for User {
    fn display_name(&self) -> &str {
        &self.name
    }
}

/// An admin wrapping a user.
pub struct Admin {
    user: User,
}

impl Admin {
    pub fn promote(&self) -> bool {
        true
    }
}

impl Named for Admin {
    fn display_name(&self) -> &str {
        self.user.display_name()
    }
}

/// Greet a named entity.
pub fn greet_named(entity: &dyn Named) -> String {
    format!("Hi, {}!", entity.display_name())
}

/// Main entry point.
pub fn main() {
    let user = User::new("Alice", 30);
    let _greeting = greet_named(&user);
    let admin = Admin { user: User::new("Bob", 25) };
    let _admin_greeting = greet_named(&admin);
}
