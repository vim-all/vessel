use namespaces::spawn;

pub fn run(rootfs: &str, command: &str) -> Result<(), Box<dyn std::error::Error>> {
    spawn(rootfs, command)?;
    Ok(())
}
