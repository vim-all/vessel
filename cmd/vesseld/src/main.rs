use std::collections::HashMap;
use std::os::unix::net::UnixListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::io::{Read, Write};
use api::{Request, Response};
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::Pid;
use std::time::Duration;
use serde_json;
use runtime;

fn main() -> std::io::Result<()> {
    let socket_path = "/run/vessel.sock";

    // Remove old socket
    let _ = std::fs::remove_file(socket_path);

    let listener = UnixListener::bind(socket_path)?;
    println!("vesseld listening on {}", socket_path);

    let state = Arc::new(Mutex::new(HashMap::<String, i32>::new())); 
    let state_clone = Arc::clone(&state);

    std::thread::spawn(move || {
        loop {
            match waitpid(Pid::from_raw(-1), Some(WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::Exited(pid, _)) => {
                    println!("Reaped child {}", pid);

                    let mut map = state_clone.lock().unwrap();
                    for (_, stored_pid) in map.iter_mut() {
                        if *stored_pid == pid.as_raw() {
                            *stored_pid = 0;
                        }
                    }
                }
                Ok(WaitStatus::StillAlive) => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(_) => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                _ => {}
            }
        }
    });

    for stream in listener.incoming() {
        let stream = stream?;
        let state = Arc::clone(&state);

        thread::spawn(move || {
            handle_client(stream, state);
        });
    }

    Ok(())
}


fn handle_client(
    mut stream: std::os::unix::net::UnixStream,
    _state: Arc<Mutex<HashMap<String, i32>>>,
) {
    let mut buffer = Vec::new();

    // Read full request
    if let Err(e) = stream.read_to_end(&mut buffer) {
        eprintln!("Failed to read request: {}", e);
        return;
    }

    let request: Request = match serde_json::from_slice(&buffer) {
        Ok(req) => req,
        Err(e) => {
            let _ = send_response(&mut stream, Response::Error(format!("Invalid request: {}", e)));
            return;
        }
    };

    let response = match request {
        Request::Run { image, command } => {
            match runtime::run(&image, &command) {
                Ok(id) => Response::Ok(id),
                Err(e) => Response::Error(e.to_string()),
            }
        }

        Request::Ps => {
            match runtime::ps() {
                Ok(list) => {
                    let result = list
                        .into_iter()
                        .map(|c| (c.id, c.pid, format!("{:?}", c.state)))
                        .collect();
                    Response::Containers(result)
                }
                Err(e) => Response::Error(e.to_string()),
            }
        }

        Request::Images => {
            match runtime::images() {
                Ok(list) => {
                    let result = list
                        .into_iter()
                        .map(|img| (img.name, img.size, img.tag))
                        .collect();
                    Response::Images(result)
                }
                Err(e) => Response::Error(e.to_string()),
            }
        }

        Request::Stop { id } => {
            match runtime::stop(&id) {
                Ok(_) => Response::Ok("Stopped".into()),
                Err(e) => Response::Error(e.to_string()),
            }
        }

        Request::Rm { id } => {
            match runtime::rm(&id) {
                Ok(_) => Response::Ok("Removed".into()),
                Err(e) => Response::Error(e.to_string()),
            }
        }

        Request::Logs { id } => {
            match runtime::logs(&id) {
                Ok(content) => Response::Ok(content),
                Err(e) => Response::Error(e.to_string()),
            }
        }

        Request::Pull { image } => {
            match runtime::pull(&image) {
                Ok(_) => Response::Ok(format!("Image {} pulled", image)),
                Err(e) => Response::Error(e.to_string()),
            }
        }
    };

    let _ = send_response(&mut stream, response);
}

fn send_response(
    stream: &mut std::os::unix::net::UnixStream,
    resp: Response,
) -> std::io::Result<()> {
    let data = serde_json::to_vec(&resp)?;
    stream.write_all(&data)?;
    Ok(())
}