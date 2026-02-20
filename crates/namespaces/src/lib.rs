use nix::mount::{mount, umount2, MsFlags, MntFlags};
use nix::sched::{clone, CloneFlags};
use nix::sys::wait::waitpid;
use nix::unistd::{chdir, execvp, sethostname, pivot_root};
use std::ffi::CString;
use std::process::exit;
use std::fs::{create_dir_all, write};
use std::path::Path;

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
        "+cpu",
    )
    ?;

    // Create container-specific cgroup
    create_dir_all(&container_group)?;

    // Limit CPU to 50%
    write(
        format!("{}/cpu.max", container_group),
        "2000 100000",
    )?;

    // Add process to cgroup
    write(
        format!("{}/cgroup.procs", container_group),
        pid.to_string(),
    )?;

    Ok(())
}

pub fn spawn(rootfs: &str, command: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut stack = vec![0u8; STACK_SIZE];

    let child = unsafe {
        clone(
            Box::new(|| {
                container_init(rootfs, command).unwrap();
                0
            }),
            &mut stack,
            CloneFlags::CLONE_NEWPID
                | CloneFlags::CLONE_NEWUTS
                | CloneFlags::CLONE_NEWNS,
            Some(libc::SIGCHLD),
        )?
    };
    println!("Spawned container with PID: {}", child);
    setup_cgroup(child.as_raw())?;
    println!("Cgroup setup complete for PID: {}", child);

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

    // Exec the command
    let cmd = CString::new(command).unwrap();
    execvp(&cmd, &[cmd.clone()])?;

    exit(1);
}
