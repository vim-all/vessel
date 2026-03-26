use std::path::Path;
use serde::{Serialize, Deserialize};
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use sha2::{Sha256, Digest};
use std::fs::File;
use std::io::Read;
use crate::container::load_metadata;

const IMAGE_ROOT: &str = "/var/lib/vessel/images";
const LAYER_ROOT: &str = "/var/lib/vessel/layers";

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

pub fn sha256_file(path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();

    let mut buffer = [0; 8192];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 { break; }
        hasher.update(&buffer[..n]);
    }

    Ok(format!("{:x}", hasher.finalize()))
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

pub fn get_image_layers(image: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let manifest_path = format!("{}/{}/manifest.json", IMAGE_ROOT, image);
    let data = fs::read_to_string(manifest_path)?;

    let manifest: ImageManifest = serde_json::from_str(&data)?;

    let paths = manifest
        .layers
        .iter()
        .map(|hash| format!("{}/{}", LAYER_ROOT, hash))
        .collect();

    Ok(paths)
}

fn get_upperdir(container_id: &str) -> String {
    format!("/var/lib/vessel/containers/{}/upper", container_id)
}

fn create_layer_tar(container_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let upper = get_upperdir(container_id);
    let tar_path = format!("/tmp/{}_layer.tar.gz", container_id);

    Command::new("tar")
        .arg("-czf")
        .arg(&tar_path)
        .arg("-C")
        .arg(&upper)
        .arg(".")
        .status()?;

    Ok(tar_path)
}

fn store_layer(tar_path: &str, hash: &str) -> Result<(), Box<dyn std::error::Error>> {
    let layer_dir = format!("/var/lib/vessel/layers/{}", hash);

    if Path::new(&layer_dir).exists() {
        return Ok(()); // already exists
    }

    fs::create_dir_all(&layer_dir)?;

    Command::new("tar")
        .arg("-xzf")
        .arg(tar_path)
        .arg("-C")
        .arg(&layer_dir)
        .status()?;

    Ok(())
}

pub fn image_exists(image: &str) -> bool {
    let path = format!("{}/{}/manifest.json", IMAGE_ROOT, image);
    Path::new(&path).exists()
}

pub fn load_image(name: &str) -> Option<ImageMetadata> {
    let path = format!("{}/{}/manifest.json", IMAGE_ROOT, name);
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
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

pub fn pull_image(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let image_dir = format!("{}/{}", IMAGE_ROOT, name);
    let tar_path = format!("{}/image.tar.gz", image_dir);

    // If image already exists
    if Path::new(&format!("{}/manifest.json", image_dir)).exists() {
        println!("Image '{}' already exists", name);
        return Ok(());
    }

    fs::create_dir_all(&image_dir)?;

    let url = "https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/x86/alpine-minirootfs-3.23.0-x86.tar.gz";
    
    println!("Downloading image...");

    Command::new("wget")
        .arg("-O")
        .arg(&tar_path)
        .arg(url)
        .status()?;

    println!("Hashing layer...");
    let hash = sha256_file(&tar_path)?;

    let layer_dir = format!("{}/{}", LAYER_ROOT, hash);

    // If layer not already stored
    if !Path::new(&layer_dir).exists() {
        println!("Extracting layer...");

        fs::create_dir_all(&layer_dir)?;

        Command::new("tar")
            .arg("-xzf")
            .arg(&tar_path)
            .arg("-C")
            .arg(&layer_dir)
            .status()?;
    } else {
        println!("Layer already exists, skipping extraction");
    }

    fs::remove_file(&tar_path)?;

    // Calculate size
    let size = dir_size(Path::new(&layer_dir));
    let size_str = format_size(size);

    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs();

    // Create manifest
    let manifest = ImageMetadata {
        name: name.to_string(),
        tag: "latest".to_string(),
        size: size_str,
        created_at,
        layers: vec![hash],
    };

    let json = serde_json::to_string_pretty(&manifest)?;
    fs::write(format!("{}/manifest.json", image_dir), json)?;

    println!("Image '{}' pulled successfully", name);

    Ok(())
}

pub fn commit_image(container_id: &str, new_image: &str) -> Result<(), Box<dyn std::error::Error>> {
    let meta = load_metadata(container_id)?;

    let base_image = meta.image;

    // 1. Create tar from upperdir
    let tar_path = create_layer_tar(container_id)?;

    // 2. Hash it
    let hash = sha256_file(&tar_path)?;

    // 3. Store layer
    store_layer(&tar_path, &hash)?;

    fs::remove_file(&tar_path)?;

    // 4. Get base image layers
    let layers = get_image_layers(&base_image)?;

    // convert full paths → hashes
    let base_layers: Vec<String> = layers
        .iter()
        .map(|p| {
            p.split("/").last().unwrap().to_string()
        })
        .collect();

    // 5. Append new layer
    let mut new_layers = base_layers;
    new_layers.push(hash);

    // 6. Create new image manifest
    let new_image_dir = format!("{}/{}", IMAGE_ROOT, new_image);
    fs::create_dir_all(&new_image_dir)?;

    // 7. Calculate total image size from all layers
    let size: u64 = new_layers
        .iter()
        .map(|layer_hash| {
            let layer_path = format!("{}/{}", LAYER_ROOT, layer_hash);
            dir_size(Path::new(&layer_path))
        })
        .sum();
    let size_str = format_size(size);

    let manifest = ImageMetadata {
        name: new_image.to_string(),
        tag: "latest".to_string(),
        size: size_str,
        created_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        layers: new_layers,
    };

    let json = serde_json::to_string_pretty(&manifest)?;
    fs::write(format!("{}/manifest.json", new_image_dir), json)?;

    println!("Image '{}' created from container {}", new_image, container_id);

    Ok(())
}