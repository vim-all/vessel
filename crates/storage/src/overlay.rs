use nix::mount::{mount, MsFlags};
use std::fs::create_dir_all;

pub fn setup_overlay(container_id: &str, image: &str) -> anyhow::Result<String> {
    let base = format!("/var/lib/vessel/containers/{}", container_id);

    let upper = format!("{}/upper", base);
    let work = format!("{}/work", base);
    let merged = format!("{}/merged", base);
    let lower = format!("/var/lib/vessel/images/{}/rootfs", image);

    create_dir_all(&upper)?;
    create_dir_all(&work)?;
    create_dir_all(&merged)?;

    let options = format!(
        "lowerdir={},upperdir={},workdir={}",
        lower, upper, work
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