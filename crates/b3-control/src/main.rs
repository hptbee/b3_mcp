use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
};

use b3_control::{serve, ControlError, ServeOptions};

#[tokio::main]
async fn main() {
    match parse_args() {
        Ok(options) => {
            if let Err(error) = serve(options).await {
                eprintln!("{error:?}");
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("{error:?}");
            std::process::exit(1);
        }
    }
}

fn parse_args() -> Result<ServeOptions, ControlError> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "serve".to_string());
    if command != "serve" {
        return Err(ControlError::bad_request(
            "expected command: b3-control-server serve",
        ));
    }

    let mut options = ServeOptions::default();
    let mut host = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let mut port = options.bind_addr.port();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--project" => {
                options.project_path = PathBuf::from(next_arg(&mut args, "--project")?);
            }
            "--database" => {
                options.database_path = PathBuf::from(next_arg(&mut args, "--database")?);
            }
            "--port" => {
                let value = next_arg(&mut args, "--port")?;
                port = value
                    .parse::<u16>()
                    .map_err(|_| ControlError::bad_request("--port must be a valid u16"))?;
            }
            "--host" => {
                let value = next_arg(&mut args, "--host")?;
                host = value
                    .parse::<IpAddr>()
                    .map_err(|_| ControlError::bad_request("--host must be an IP address"))?;
            }
            "--allow-non-local-bind" => {
                options.allow_non_local_bind = true;
            }
            "--watch" => {
                options.watch = true;
            }
            "--debounce-ms" => {
                let value = next_arg(&mut args, "--debounce-ms")?;
                options.debounce_ms = value
                    .parse::<u64>()
                    .map_err(|_| ControlError::bad_request("--debounce-ms must be a valid u64"))?;
            }
            "--help" | "-h" => {
                println!(
                    "Usage: b3-control-server serve --project . --database .b3/b3.db --port 7777 --watch"
                );
                std::process::exit(0);
            }
            _ => {
                return Err(ControlError::bad_request(format!(
                    "unknown argument: {arg}"
                )));
            }
        }
    }

    options.bind_addr = SocketAddr::new(host, port);
    options.validate()?;
    Ok(options)
}

fn next_arg(
    args: &mut impl Iterator<Item = String>,
    name: &'static str,
) -> Result<String, ControlError> {
    args.next()
        .ok_or_else(|| ControlError::bad_request(format!("{name} requires a value")))
}
