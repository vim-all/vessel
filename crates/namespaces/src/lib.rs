use nix::mount::{mount, umount2, MsFlags, MntFlags};
use nix::sched::{clone, CloneFlags};
use nix::sys::wait::waitpid;
use nix::unistd::{chdir, execvp, sethostname, pivot_root};
use std::ffi::CString;
use std::process::exit;

const STACK_SIZE: usize = 1024 * 1024;

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
