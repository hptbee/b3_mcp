use std::path::PathBuf;

use b3_mcp_runtime::{serve_local_stdio, RuntimeBootstrapConfig, ToolProfileName};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        return Err(usage());
    };

    match command.as_str() {
        "serve" => {
            let mut project = PathBuf::from(".");
            let mut database = None;
            let mut profile = ToolProfileName::default();
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--project" => {
                        project = args
                            .next()
                            .map(PathBuf::from)
                            .ok_or_else(|| "--project requires a path".to_string())?;
                    }
                    "--database" => {
                        database = Some(
                            args.next()
                                .map(PathBuf::from)
                                .ok_or_else(|| "--database requires a path".to_string())?,
                        );
                    }
                    "--profile" | "--tool-profile" => {
                        profile = args
                            .next()
                            .ok_or_else(|| format!("{arg} requires a profile name"))?
                            .parse()?;
                    }
                    _ => return Err(format!("unknown argument: {arg}\n{}", usage())),
                }
            }

            let mut config = RuntimeBootstrapConfig::local_project(project);
            if let Some(database) = database {
                config.database_path = database;
            }
            config.tool_profile = profile;
            serve_local_stdio(config)
        }
        _ => Err(usage()),
    }
}

fn usage() -> String {
    "usage: b3-mcp-runtime serve --project <path> [--database <path>] [--profile <tiny|optimized|full|debug|readonly|editing|web-app|enterprise>]".to_string()
}
