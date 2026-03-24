use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub enum Request {
    Run { image: String, command: Vec<String> },
    Ps,
    Stop { id: String },
    Rm { id: String },
    Logs { id: String },
    Images,
}

#[derive(Serialize, Deserialize)]
pub enum Response {
    Ok(String),
    Containers(Vec<(String, i32, String)>),
    Images(Vec<(String, String, String)>),
    Error(String),
}