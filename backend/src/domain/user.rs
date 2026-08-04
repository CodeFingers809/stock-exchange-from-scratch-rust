use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct UserInner {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct User(Arc<Mutex<UserInner>>);

impl User {
    pub fn new(id: String, name: String) -> Self {
        Self(Arc::new(Mutex::new(UserInner { id, name })))
    }

    pub fn id(&self) -> String {
        self.0.lock().unwrap().id.clone()
    }

    pub fn name(&self) -> String {
        self.0.lock().unwrap().name.clone()
    }

    pub fn set_name(&self, new_name: String) {
        self.0.lock().unwrap().name = new_name;
    }
}