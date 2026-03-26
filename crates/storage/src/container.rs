use serde::{Serialize, Deserialize};
use std::fs;
use std::path::Path;

pub const VESSEL_ROOT: &str = "/var/lib/vessel/containers";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ContainerState {
    Running,
    Stopped,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContainerMetadata {
    pub id: String,
    pub pid: i32,
    pub rootfs: String,
    pub image: String,
    pub command: String,
    pub state: ContainerState,
    pub created_at: u64,
}

fn is_process_alive(pid: i32) -> bool {
    Path::new(&format!("/proc/{}", pid)).exists()
}

pub fn container_dir(id: &str) -> String {
    format!("{}/{}", VESSEL_ROOT, id)
}

pub fn save_metadata(meta: &ContainerMetadata) -> Result<(), Box<dyn std::error::Error>> {
    let dir = container_dir(&meta.id);
    fs::create_dir_all(&dir)?;

    let data = serde_json::to_string_pretty(meta)?;
    fs::write(format!("{}/metadata.json", dir), data)?;

    Ok(())
}

pub fn load_metadata(id: &str) -> Result<ContainerMetadata, Box<dyn std::error::Error>> {
    let path = format!("{}/{}/metadata.json", VESSEL_ROOT, id);
    let data = fs::read_to_string(path)?;
    let meta: ContainerMetadata = serde_json::from_str(&data)?;
    if !is_process_alive(meta.pid) {
        let mut meta = meta.clone();
        meta.state = ContainerState::Stopped;
        save_metadata(&meta)?;
    }
    Ok(meta)
}

pub fn list_containers() -> Result<Vec<ContainerMetadata>, Box<dyn std::error::Error>> {
    let mut containers = Vec::new();

    if !Path::new(VESSEL_ROOT).exists() {
        return Ok(containers);
    }

    for entry in fs::read_dir(VESSEL_ROOT)? {
        let entry = entry?;
        if entry.path().is_dir() {
            let id = entry.file_name().into_string().unwrap();
            if let Ok(meta) = load_metadata(&id) {
                containers.push(meta);
            }
        }
    }

    Ok(containers)
}