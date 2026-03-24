use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub enum Request {
    Run { rootfs: String, command: Vec<String> },
    Ps,
    Stop { id: String },
    Rm { id: String },
    Logs { id: String },
}

#[derive(Serialize, Deserialize)]
pub enum Response {
    Ok(String),
    List(Vec<(String, i32, String)>),
    Error(String),
}