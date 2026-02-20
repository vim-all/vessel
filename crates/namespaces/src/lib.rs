use nix::mount::{mount, umount2, MsFlags, MntFlags};
use nix::sched::{clone, CloneFlags};
use nix::sys::wait::waitpid;
use nix::unistd::{chdir, execvp, sethostname, pivot_root};
use std::ffi::CString;
use std::process::exit;
use std::fs::{create_dir_all, write};
use std::path::Path;
use std::process::Command;
use nix::unistd::{pipe, read, write as fd_write};

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

pub fn spawn(rootfs: &str, command: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut stack = vec![0u8; STACK_SIZE];
    let (reader, writer) = pipe()?;
    let child = unsafe {
        clone(
            Box::new(move || {
                let mut buf = [0u8; 1];
                read(reader, &mut buf).unwrap();
                container_init(rootfs, command).unwrap();
                0
            }),
            &mut stack,
            CloneFlags::CLONE_NEWPID
                | CloneFlags::CLONE_NEWUTS
                | CloneFlags::CLONE_NEWNS
                | CloneFlags::CLONE_NEWNET,
            Some(libc::SIGCHLD),
        )?
    };
    println!("Spawned container with PID: {}", child);
    setup_cgroup(child.as_raw())?;
    println!("Cgroup setup complete for PID: {}", child);

    setup_network(child.as_raw())?;
    fd_write(writer, &[1])?;

    waitpid(child, None)?;
    Ok(())
}

fn container_init(rootfs: &str, command: &str) -> Result<(), Box<dyn std::error::Error>> {
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

    // Exec the command
    let cmd = CString::new(command).unwrap();
    execvp(&cmd, &[cmd.clone()])?;

    exit(1);
}
