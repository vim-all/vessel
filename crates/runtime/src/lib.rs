use namespaces::spawn;
use uuid::Uuid;
use storage::{ContainerMetadata, ContainerState, save_metadata, list_containers, load_metadata, container_dir};
use std::time::{SystemTime, UNIX_EPOCH, Duration, Instant};
use nix::sys::signal::{kill, SIGTERM, SIGKILL};
use nix::unistd::Pid;
use std::thread::sleep;
use std::path::Path;
use storage::{setup_overlay, image_exists, list_images, pull_image, commit_image};
use nix::mount::{umount2, MntFlags};
use storage::image::{load_image, ImageConfig};

pub fn run(image: &str, command: &[String]) -> Result<String, Box<dyn std::error::Error>> {
    if !image_exists(image) {
        return Err(format!("Image '{}' not found", image).into());
    }

    let image_meta = load_image(image)
        .ok_or("Failed to load image metadata")?;

    let config = image_meta.config.unwrap_or(ImageConfig {
        cmd: vec!["/bin/sh".to_string()],
        working_dir: "/".to_string(),
    });

    let final_cmd: Vec<String> = if command.is_empty() {
        config.cmd.clone()
    } else {
        command.to_vec()
    };

    let working_dir = Some(config.working_dir.clone());

    let id = Uuid::new_v4().to_string();
    let merged_rootfs = setup_overlay(&id, image)?;

    let dir = container_dir(&id);
    std::fs::create_dir_all(&dir)?;
    let log_path = format!("{}/container.log", dir);

    let pid = spawn(&merged_rootfs, &final_cmd, working_dir, &log_path)?;

    let meta = ContainerMetadata {
        id: id.clone(),
        pid,
        rootfs: merged_rootfs.clone(),
        image: image.to_string(),
        command: final_cmd.join(" "), // updated
        state: ContainerState::Running,
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs(),
    };

    save_metadata(&meta)?;

    Ok(id)
}

fn is_process_alive(pid: i32) -> bool {
    Path::new(&format!("/proc/{}", pid)).exists()
}

pub fn ps() -> Result<Vec<ContainerMetadata>, Box<dyn std::error::Error>> {
    let containers = list_containers()?;

    println!("{:<40} {:<8} {}", "ID", "PID", "STATE");

    for c in &containers {
        println!("{:<40} {:<8} {:?}", c.id, c.pid, c.state);
    }

    Ok(containers)
}

pub fn stop(id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut meta = load_metadata(id)?;

    let pid = meta.pid;

    if !is_process_alive(pid) {
        println!("Container {} already stopped", id);
        meta.state = ContainerState::Stopped;
        save_metadata(&meta)?;
        return Ok(());
    }

    kill(Pid::from_raw(pid),SIGTERM)?;

    // Wait up to 3 seconds for graceful shutdown
    let timeout = Duration::from_secs(3);
    let start = Instant::now();

    while is_process_alive(pid) {
        if start.elapsed() >= timeout {
            break;
        }
        sleep(Duration::from_millis(100));
    }

    if is_process_alive(pid) {
        kill(Pid::from_raw(pid), SIGKILL)?;

        // wait for forced kill
        let start = Instant::now();
        while is_process_alive(pid) {
            if start.elapsed() >= Duration::from_secs(2) {
                return Err("Failed to kill container".into());
            }
            sleep(Duration::from_millis(100));
        }
    }

    meta.state = ContainerState::Stopped;
    save_metadata(&meta)?;

    println!("Container {} stopped successfully", id);

    Ok(())
}

pub fn logs(id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let log_path = format!("{}/container.log", container_dir(id));
    if !Path::new(&log_path).exists() {
        return Ok(format!("No logs found for container {}", id));
    }
    let contents = std::fs::read_to_string(&log_path)?;
    print!("{}", contents);
    Ok(contents)
}

pub fn rm(id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let meta = load_metadata(id)?;

    if is_process_alive(meta.pid) {
        return Err(format!(
            "Container {} is still running (PID {}). Stop it first.",
            id, meta.pid
        ).into());
    }

    let dir = container_dir(id);
    let merged = format!("{}/merged", dir);

    if Path::new(&merged).exists() {
        if let Err(e) = umount2(merged.as_str(), MntFlags::MNT_DETACH) {
            eprintln!("Warning: failed to unmount {}: {}", merged, e);
        }
    }

    if Path::new(&dir).exists() {
        std::fs::remove_dir_all(&dir)?;
    }

    println!("Container {} removed", id);

    Ok(())
}

pub fn images() -> Result<Vec<storage::ImageMetadata>, Box<dyn std::error::Error>> {
    let images = list_images();

    println!("{:<15} {:<10} {}", "NAME", "TAG", "SIZE");

    for img in &images {
        println!("{:<15} {:<10} {}", img.name, img.tag, img.size);
    }

    Ok(images)
}

pub fn pull(image: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Pulling image '{}'...", image);
    pull_image(image)
}

pub fn commit(id: &str, new_image: &str) -> Result<(), Box<dyn std::error::Error>> {
    let meta = load_metadata(id)?;

    if is_process_alive(meta.pid) {
        return Err(format!(
            "Container {} is still running (PID {}). Stop it first.",
            id, meta.pid
        ).into());
    }

    commit_image(id, new_image)?;

    println!("Container {} committed as image '{}'", id, new_image);

    Ok(())
}