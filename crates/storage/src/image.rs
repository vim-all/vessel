use std::path::Path;

pub fn image_exists(image: &str) -> bool {
    let path = format!("/var/lib/vessel/images/{}/rootfs", image);
    Path::new(&path).exists()
}

pub fn get_image_rootfs(image: &str) -> String {
    format!("/var/lib/vessel/images/{}/rootfs", image)
}