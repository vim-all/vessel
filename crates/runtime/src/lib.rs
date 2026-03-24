use namespaces::spawn;
use uuid::Uuid;
use storage::{ContainerMetadata, ContainerState, save_metadata, list_containers, load_metadata, container_dir};
use std::time::{SystemTime, UNIX_EPOCH, Duration, Instant};
use nix::sys::signal::{kill, SIGTERM, SIGKILL};
use nix::unistd::Pid;
use std::thread::sleep;
use std::path::Path;
use storage::setup_overlay;
use storage::image_exists;

pub fn run(image: &str, command: &Vec<String>) -> Result<String, Box<dyn std::error::Error>> {
    if !image_exists(image) {
        return Err(format!("Image '{}' not found", image).into());
    }

    let id = Uuid::new_v4().to_string();
    let merged_rootfs = setup_overlay(&id, image)?;

    // Create the container directory and log file path before spawning
    // so the child can write output there immediately.
    let dir = container_dir(&id);
    std::fs::create_dir_all(&dir)?;
    let log_path = format!("{}/container.log", dir);

    let pid = spawn(&merged_rootfs, command, &log_path)?;

    let meta = ContainerMetadata {
        id: id.clone(),
        pid,
        rootfs: merged_rootfs.clone(),
        command: command.join(" "),
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
        return Err(format!("Container {} is still running (PID {}). Stop it first.", id, meta.pid).into());
    }

    let dir = container_dir(id);
    if Path::new(&dir).exists() {
        std::fs::remove_dir_all(&dir)?;
    }

    println!("Container {} removed", id);
    Ok(())
}
