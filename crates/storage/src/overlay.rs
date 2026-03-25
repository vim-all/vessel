use nix::mount::{mount, MsFlags};
use std::fs::create_dir_all;
use crate::image::get_image_layers;

pub fn setup_overlay(container_id: &str, image: &str) -> anyhow::Result<String> {
    let base = format!("/var/lib/vessel/containers/{}", container_id);

    let upper = format!("{}/upper", base);
    let work = format!("{}/work", base);
    let merged = format!("{}/merged", base);
    let layers = get_image_layers(image)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let mut layers = layers;
    layers.reverse();
    
    let lowerdirs = layers.join(":");
    create_dir_all(&upper)?;
    create_dir_all(&work)?;
    create_dir_all(&merged)?;

    let options = format!(
        "lowerdir={},upperdir={},workdir={}",
        lowerdirs, upper, work
    );

    mount(
        Some("overlay"),
        merged.as_str(),
        Some("overlay"),
        MsFlags::empty(),
        Some(options.as_str()),
    )?;

    Ok(merged)
}