use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub enum Request {
    Run { image: String, command: Vec<String> },
    Build { context: String, image: String },
    Ps,
    Stop { id: String },
    Rm { id: String },
    Logs { id: String },
    Images,
    Pull { image: String },
    Commit { id: String, image: String },
}

#[derive(Serialize, Deserialize)]
pub enum Response {
    Ok(String),
    Containers(Vec<(String, i32, String)>),
    Images(Vec<(String, String, String)>),
    Error(String),
}