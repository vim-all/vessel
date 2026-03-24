use std::path::Path;
use serde::{Serialize, Deserialize};
use std::fs;

#[derive(Serialize, Deserialize, Debug)]
pub struct ImageMetadata {
    pub name: String,
    pub tag: String,
    pub size: String,
    pub created_at: u64,
}

pub fn list_images() -> Vec<ImageMetadata> {
    let base = "/var/lib/vessel/images";
    let mut images = Vec::new();

    if !Path::new(base).exists() {
        return images;
    }

    for entry in fs::read_dir(base).unwrap() {
        let entry = entry.unwrap();
        if entry.path().is_dir() {
            let name = entry.file_name().into_string().unwrap();
            if let Some(img) = load_image(&name) {
                images.push(img);
            }
        }
    }

    images
}

pub fn load_image(name: &str) -> Option<ImageMetadata> {
    let path = format!("/var/lib/vessel/images/{}/manifest.json", name);
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

pub fn image_exists(image: &str) -> bool {
    let path = format!("/var/lib/vessel/images/{}/rootfs", image);
    Path::new(&path).exists()
}

pub fn get_image_rootfs(image: &str) -> String {
    format!("/var/lib/vessel/images/{}/rootfs", image)
}