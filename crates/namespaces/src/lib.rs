use nix::mount::{mount, umount2, MsFlags, MntFlags};
use nix::sched::{clone, CloneFlags};
use nix::unistd::{chdir, execvp, sethostname, pivot_root};
use std::ffi::CString;
use std::process::exit;
use std::fs::{create_dir_all, write, OpenOptions};
use std::os::unix::io::IntoRawFd;
use std::path::Path;
use std::process::Command;
use nix::unistd::{pipe, read, write as fd_write, close, dup2};

const STACK_SIZE: usize = 1024 * 1024;

fn setup_cgroup(pid: i32) -> Result<(), Box<dyn std::error::Error>> {
    println!("Setting up cgroup for PID: {}", pid);
    let cgroup_root = "/sys/fs/cgroup";
    let vessel_group = format!("{}/vessel", cgroup_root);
    let container_group = format!("{}/{}", vessel_group, pid);

    // Create /sys/fs/cgroup/vessel
    if !Path::new(&vessel_group).exists() {
        create_dir_all(&vessel_group)?;
    }

    // Enable CPU controller
    write(
        format!("{}/cgroup.subtree_control", vessel_group),
        "+cpu +memory",
    )?;

    // Create container-specific cgroup
    create_dir_all(&container_group)?;

    // Limit CPU to 2%
    write(
        format!("{}/cpu.max", container_group),
        "2000 100000",
    )?;

    // hard limit memory to 100mb
    write(format!("{}/memory.max", container_group),
    "104857600")?;

    // soft limit memory to 80mb
    write(format!("{}/memory.high", container_group),
    "83886080")?;

    // Add process to cgroup
    write(
        format!("{}/cgroup.procs", container_group),
        pid.to_string(),
    )?;

    Ok(())
}

fn setup_network(pid: i32) -> Result<(), Box<dyn std::error::Error>> {
    let veth_host = format!("veth{}", pid);
    let veth_container = format!("veth{}c", pid);

    // Create veth pair
    Command::new("ip")
        .args([
            "link", "add",
            &veth_host,
            "type", "veth",
            "peer", "name", &veth_container
        ])
        .status()?;

    // Attach host side to bridge
    Command::new("ip")
        .args(["link", "set", &veth_host, "master", "vessel0"])
        .status()?;

    Command::new("ip")
        .args(["link", "set", &veth_host, "up"])
        .status()?;

    // Move container side into net namespace
    Command::new("ip")
        .args([
            "link", "set",
            &veth_container,
            "netns", &pid.to_string()
        ])
        .status()?;

    Ok(())
}

pub fn spawn(rootfs: &str, command: &Vec<String>, log_path: &str) -> Result<i32, Box<dyn std::error::Error>> {
    // Leak the stack so it outlives the parent process in detached mode.
    // The child runs in a separate process after clone(), so if the parent
    // returns and frees this Vec, the child would crash.
    let stack = vec![0u8; STACK_SIZE].into_boxed_slice();
    let stack: &'static mut [u8] = Box::leak(stack);

    let (reader, writer) = pipe()?;
    let log_path_owned = log_path.to_string();
    let child = unsafe {
        clone(
            Box::new(move || {
                close(writer).unwrap();
                let mut buf = [0u8; 1];
                read(reader, &mut buf).unwrap();
                close(reader).unwrap();

                // Redirect stdout and stderr to the log file so output
                // is captured even after the parent process exits.
                if let Ok(file) = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_path_owned)
                {
                    let fd = file.into_raw_fd();
                    let _ = dup2(fd, 1); // stdout
                    let _ = dup2(fd, 2); // stderr
                    let _ = close(fd);
                }

                container_init(rootfs, command).unwrap();
                0
            }),
            stack,
            CloneFlags::CLONE_NEWPID
                | CloneFlags::CLONE_NEWUTS
                | CloneFlags::CLONE_NEWNS
                | CloneFlags::CLONE_NEWNET,
            Some(libc::SIGCHLD),
        )?
    };
    close(reader).unwrap();

    println!("Spawned container with PID: {}", child);
    setup_cgroup(child.as_raw())?;
    println!("Cgroup setup complete for PID: {}", child);

    setup_network(child.as_raw())?;
    fd_write(writer, &[1])?;
    close(writer).unwrap();

    Ok(child.as_raw())
}

fn container_init(rootfs: &str, command: &Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    // Set hostname
    sethostname("vessel")?;

    // Make mounts private
    mount::<str, str, str, str>(
        None,
        "/",
        None,
        MsFlags::MS_REC | MsFlags::MS_PRIVATE,
        None,
    )?;

    mount(
        Some(rootfs),
        rootfs,
        None::<&str>,
        MsFlags::MS_BIND | MsFlags::MS_REC,
        None::<&str>,
    )?;

    let old_root = format!("{}/old_root", rootfs);
    std::fs::create_dir_all(&old_root)?;

    chdir(rootfs)?;

    pivot_root(".", "./old_root")?;

    chdir("/")?;

    create_dir_all("/dev")?;

    mount(
        Some("devtmpfs"),
        "/dev",
        Some("devtmpfs"),
        MsFlags::empty(),
        None::<&str>,
    )?;

    create_dir_all("/dev/shm")?;

    mount(
        Some("tmpfs"),
        "/dev/shm",
        Some("tmpfs"),
        MsFlags::empty(),
        Some("size=256m"),
    )?;

    umount2("/old_root", MntFlags::MNT_DETACH)?;

    std::fs::remove_dir_all("/old_root")?;

    std::fs::create_dir_all("/proc").ok();

    // Mount proc filesystem
    mount(
        Some("proc"),
        "/proc",
        Some("proc"),
        MsFlags::empty(),
        None::<&str>,
    )?;

    // Bring loopback up
    Command::new("ip")
        .args(["link", "set", "lo", "up"])
        .status()?;

    // Find the veth interface dynamically
    let output = Command::new("sh")
        .args(["-c", "ip -o link show | awk -F': ' '{print $2}' | grep veth"])
        .output()?;

    let raw_iface = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();

    let iface = raw_iface
        .split('@')
        .next()
        .unwrap_or("")
        .to_string();

    println!("veth name is found to be: {}", iface);
    if !iface.is_empty() {
        Command::new("ip")
            .args(["link", "set", &iface, "name", "eth0"])
            .status()?;
    }

    // Bring eth0 up
    Command::new("ip")
        .args(["link", "set", "eth0", "up"])
        .status()?;

    // Assign IP
    Command::new("ip")
        .args(["addr", "add", "10.0.0.2/24", "dev", "eth0"])
        .status()?;

    // Add default route
    Command::new("ip")
        .args(["route", "add", "default", "via", "10.0.0.1"])
        .status()?;

    println!("pid inside namespace: {}", nix::unistd::getpid());

    // Exec the command
    let cstrings: Vec<CString> = command
        .iter()
        .map(|arg| CString::new(arg.as_str()).unwrap())
        .collect();

    // execvp(&cstrings[0], &cstrings)?;
    match execvp(&cstrings[0], &cstrings) {
    Ok(_) => {}
    Err(e) => {
        eprintln!("EXEC FAILED: {}", e);
        std::process::exit(1);
    }
}

    exit(1);
}
