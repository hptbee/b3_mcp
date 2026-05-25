use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
};

use b3_control::{
    index_project, init_project, serve, ControlError, ProjectCommandOptions, ServeOptions,
};

#[tokio::main]
async fn main() {
    match parse_args() {
        Ok(Command::Serve(options)) => {
            if let Err(error) = serve(options).await {
                eprintln!("{error:?}");
                std::process::exit(1);
            }
        }
        Ok(Command::Init(options)) => {
            if let Err(error) = init_project(&options) {
                eprintln!("{error:?}");
                std::process::exit(1);
            }
            println!(
                "initialized project={} database={}",
                options.project_path.to_string_lossy(),
                options.database_path.to_string_lossy()
            );
        }
        Ok(Command::Index { options, reindex }) => match index_project(&options, reindex) {
            Ok(summary) => {
                println!(
                    "project_path={}\ndatabase_path={}\nfiles_discovered={}\nfiles_indexed={}\nfiles_skipped={}\nsymbols_indexed={}\nedges_indexed={}\nparse_failures={}\nduration_ms={}\nbehavior={}",
                    summary.project_path,
                    summary.database_path,
                    summary.files_discovered,
                    summary.files_indexed,
                    summary.files_skipped,
                    summary.symbols_indexed,
                    summary.edges_indexed,
                    summary.parse_failures,
                    summary.duration_ms,
                    summary.behavior
                );
                if let Some(scope) = summary.scope {
                    println!("scope={scope}\ndry_run={}", summary.dry_run);
                }
                if let Some(preview) = summary.preview {
                    println!(
                        "matched_files={}\nsample_files={}",
                        preview.matched_files,
                        preview.sample_files.join(",")
                    );
                }
            }
            Err(error) => {
                eprintln!("{error:?}");
                std::process::exit(1);
            }
        },
        Err(error) => {
            eprintln!("{error:?}");
            std::process::exit(1);
        }
    }
}

enum Command {
    Serve(ServeOptions),
    Init(ProjectCommandOptions),
    Index {
        options: ProjectCommandOptions,
        reindex: bool,
    },
}

fn parse_args() -> Result<Command, ControlError> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "serve".to_string());
    if matches!(command.as_str(), "--help" | "-h") {
        print_help();
        std::process::exit(0);
    }

    if matches!(command.as_str(), "init" | "index" | "reindex") {
        let options = parse_project_options(&mut args)?;
        return Ok(match command.as_str() {
            "init" => Command::Init(options),
            "index" => Command::Index {
                options,
                reindex: false,
            },
            "reindex" => Command::Index {
                options,
                reindex: true,
            },
            _ => unreachable!(),
        });
    }

    if command != "serve" {
        return Err(ControlError::bad_request(
            "expected command: init, index, reindex, or serve",
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
                print_help();
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
    Ok(Command::Serve(options))
}

fn parse_project_options(
    args: &mut impl Iterator<Item = String>,
) -> Result<ProjectCommandOptions, ControlError> {
    let mut options = ProjectCommandOptions::default();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--project" => {
                options.project_path = PathBuf::from(next_arg(args, "--project")?);
            }
            "--database" => {
                options.database_path = PathBuf::from(next_arg(args, "--database")?);
            }
            "--scope" => {
                options.scope = Some(next_arg(args, "--scope")?);
            }
            "--dry-run" => {
                options.dry_run = true;
            }
            "--force" => {
                options.force = true;
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            _ => {
                return Err(ControlError::bad_request(format!(
                    "unknown argument: {arg}"
                )))
            }
        }
    }
    Ok(options)
}

fn next_arg(
    args: &mut impl Iterator<Item = String>,
    name: &'static str,
) -> Result<String, ControlError> {
    args.next()
        .ok_or_else(|| ControlError::bad_request(format!("{name} requires a value")))
}

fn print_help() {
    println!(
        "Usage:\n  b3-control-server init --project . --database .b3/b3.db\n  b3-control-server index --project . --database .b3/b3.db [--scope path:src] [--dry-run] [--force]\n  b3-control-server reindex --project . --database .b3/b3.db [--scope language:go] [--dry-run] [--force]\n  b3-control-server serve --project . --database .b3/b3.db --port 7777 --watch"
    );
}
