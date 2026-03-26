use std::process::Command;
use std::fs;
use crate::image::commit_image;

pub fn build_image(context: &str, image_name: &str) -> Result<(), Box<dyn std::error::Error>> {

    println!("Starting build for image: {}", image_name);

    // 1. Run temp container
    let output = Command::new("vessel")
        .args(["run", "alpine", "sleep", "infinity"])
        .output()?;

    let container_id = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string();

    println!("Temp container: {}", container_id);

    let merged = format!(
        "/var/lib/vessel/containers/{}/merged",
        container_id
    );

    // 2. Copy files into container
    println!("Copying files...");
    copy_dir_all(context, &merged)?;

    // 3. Commit container → image
    println!("Committing image...");
    commit_image(&container_id, image_name)?;

    // 4. Cleanup
    println!("Cleaning up...");
    Command::new("vessel").args(["stop", &container_id]).status()?;
    Command::new("vessel").args(["rm", &container_id]).status()?;

    println!("Build complete: {}", image_name);

    Ok(())
}

fn copy_dir_all(src: &str, dst: &str) -> std::io::Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();

        let dest_path = format!(
            "{}/{}",
            dst,
            entry.file_name().to_string_lossy()
        );

        if path.is_dir() {
            fs::create_dir_all(&dest_path)?;
            copy_dir_all(path.to_str().unwrap(), &dest_path)?;
        } else {
            fs::copy(&path, &dest_path)?;
        }
    }
    Ok(())
}