use std::path::Path;
use serde::{Serialize, Deserialize};
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Debug)]
pub struct ImageMetadata {
    pub name: String,
    pub tag: String,
    pub size: String,
    pub created_at: u64,
}

pub fn dir_size(path: &Path) -> u64 {
    let mut size = 0;

    if path.is_file() {
        return path.metadata().map(|m| m.len()).unwrap_or(0);
    }

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            size += dir_size(&entry.path());
        }
    }

    size
}

pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub fn pull_image(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let base = format!("/var/lib/vessel/images/{}", name);
    let rootfs = format!("{}/rootfs", base);

    if std::path::Path::new(&rootfs).exists() {
        println!("Image '{}' already exists", name);
        return Ok(());
    }

    fs::create_dir_all(&rootfs)?;

    let tar_path = format!("{}/image.tar.gz", base);

    let url = "https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/x86/alpine-minirootfs-3.23.0-x86.tar.gz";
    

    println!("Downloading image...");

    Command::new("wget")
        .arg("-O")
        .arg(&tar_path)
        .arg(url)
        .status()?;

    println!("Extracting...");

    Command::new("tar")
        .arg("-xzf")
        .arg(&tar_path)
        .arg("-C")
        .arg(&rootfs)
        .status()?;

    // Remove tar
    fs::remove_file(&tar_path)?;

    let size = dir_size(Path::new(&rootfs));
    let size_str = format_size(size);

    // Create manifest
    let meta = format!(
        r#"{{
        "name": "{}",
        "tag": "latest",
        "created_at": {},
        "size": {}
        }}"#,
        name,
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),size_str
    );

    fs::write(format!("{}/manifest.json", base), meta)?;

    println!("Image '{}' pulled successfully", name);

    Ok(())
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