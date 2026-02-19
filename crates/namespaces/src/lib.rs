use nix::mount::{mount, MsFlags};
use nix::sched::{clone, CloneFlags};
use nix::sys::wait::waitpid;
use nix::unistd::{chdir, chroot, execvp, sethostname};
use std::ffi::CString;
use std::process::exit;

const STACK_SIZE: usize = 1024 * 1024;

pub fn spawn(rootfs: &str, command: &str) -> nix::Result<()> {
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

fn container_init(rootfs: &str, command: &str) -> nix::Result<()> {
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

    // Change root
    chroot(rootfs)?;
    chdir("/")?;

    // Ensure /proc exists
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
