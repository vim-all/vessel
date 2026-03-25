use std::path::Path;
use serde::{Serialize, Deserialize};
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const IMAGE_ROOT: &str = "/var/lib/vessel/images";

#[derive(Serialize, Deserialize, Debug)]
pub struct ImageMetadata {
    pub name: String,
    pub tag: String,
    pub size: String,
    pub created_at: u64,
    pub layers: Vec<String>,   
}

#[derive(Deserialize)]
pub struct ImageManifest {
    pub layers: Vec<String>,
}

pub fn get_image_layers(image: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let manifest_path = format!("{}/{}/manifest.json", IMAGE_ROOT, image);
    let data = fs::read_to_string(manifest_path)?;

    let manifest: ImageManifest = serde_json::from_str(&data)?;

    let base = format!("{}/{}/layers", IMAGE_ROOT, image);

    let paths = manifest
        .layers
        .iter()
        .map(|l| format!("{}/{}", base, l))
        .collect();

    Ok(paths)
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
    let base = format!("{}/{}", IMAGE_ROOT, name);
    let layer1 = format!("{}/layers/layer1", base);

    // If already exists
    if Path::new(&layer1).exists() {
        println!("Image '{}' already exists", name);
        return Ok(());
    }

    fs::create_dir_all(&layer1)?;

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
        .arg(&layer1)   
        .status()?;

    fs::remove_file(&tar_path)?;

    // Calculate size (entire layers dir)
    let layers_dir = format!("{}/layers", base);
    let size = dir_size(Path::new(&layers_dir));
    let size_str = format_size(size);

    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs();

    let manifest = ImageMetadata {
        name: name.to_string(),
        tag: "latest".to_string(),
        size: size_str,
        created_at,
        layers: vec!["layer1".to_string()],
    };

    let json = serde_json::to_string_pretty(&manifest)?;
    fs::write(format!("{}/manifest.json", base), json)?;

    println!("Image '{}' pulled successfully", name);

    Ok(())
}

pub fn list_images() -> Vec<ImageMetadata> {
    let mut images = Vec::new();

    if !Path::new(IMAGE_ROOT).exists() {
        return images;
    }

    for entry in fs::read_dir(IMAGE_ROOT).unwrap() {
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
    let path = format!("{}/{}/manifest.json", IMAGE_ROOT, name);
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

pub fn image_exists(image: &str) -> bool {
    let path = format!("{}/{}/layers", IMAGE_ROOT, image);
    Path::new(&path).exists()
}