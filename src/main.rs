#![allow(unused_imports)]
#![allow(clippy::all)]
#![allow(unused)]

use clap::{Args, Parser, Subcommand};
use std::collections::HashMap;
use std::default::Default;
use std::path::PathBuf;

use futures_util::stream;
use futures_util::stream::StreamExt;

use bollard::container::{InspectContainerOptions, KillContainerOptions, ListContainersOptions};
use bollard::image::ListImagesOptions;
use bollard::models::ContainerSummary;
use bollard::secret::{ContainerInspectResponse, ImageSummary};
use bollard::Docker;

use prettytable::{row, Cell, Row, Table};

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Optional name to operate on
    name: Option<String>,

    /// Sets a custom config file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Working with Docker Images
    Img(Img),
    /// Show Docker Processes
    Ps(Ps),
}

#[derive(Debug, Args)]
pub struct Img {
    #[clap(subcommand)]
    pub command: ImgOptions,
}

#[derive(Debug, Subcommand)]
pub enum ImgOptions {
    /// List All OCI Images
    List,
}

#[derive(Debug, Args)]
pub struct Ps {
    #[clap(subcommand)]
    pub command: PsOptions,
}

#[derive(Debug, Subcommand)]
pub enum PsOptions {
    /// All Running Containers
    Info,
    /// Kill A Running Containers Process
    Kill(ContainerInfo),
}

#[derive(Debug, Args)]
pub struct ContainerInfo {
    /// Container Name of the Docker Container
    pub container_name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + 'static>> {
    let cli = Cli::parse();
    let docker = Docker::connect_with_socket_defaults().unwrap();

    // You can check the value provided by positional arguments, or option arguments
    if let Some(name) = cli.name.as_deref() {
        println!("Value for name: {name}");
    }

    if let Some(config_path) = cli.config.as_deref() {
        println!("Value for config: {}", config_path.display());
    }

    // You can see how many times a particular flag or argument occurred
    // Note, only flags can have multiple occurrences
    match cli.debug {
        0 => println!("Debug mode is off"),
        1 => println!("Debug mode is kind of on"),
        2 => println!("Debug mode is on"),
        _ => println!("Don't be crazy"),
    }

    // You can check for the existence of subcommands, and if found use their
    // matches just as you would the top level cmd
    match &cli.command {
        Some(Commands::Img(Img { command })) => match command {
            // ./exe img list
            ImgOptions::List => {
                let images = &docker
                    .list_images(Some(ListImagesOptions::<String> {
                        all: true,
                        ..Default::default()
                    }))
                    .await
                    .unwrap();

                // Container Summary table
                let mut image_summary_table = Table::new();
                image_summary_table.add_row(row![b->"ID", b->"Image Tag", b->"Size(KB)"]);

                for ImageSummary { id, size, repo_tags, .. } in images.iter() {
                    let image_summary_row = Row::new(vec![
                        Cell::new(&id.strip_prefix("sha256:").unwrap()[..12]),
                        Cell::new(repo_tags.iter().next().unwrap()),
                        Cell::new(&(size / (1024 as i64)).to_string()),
                    ]);

                    image_summary_table.add_row(image_summary_row);
                }

                image_summary_table.printstd();

                // for image in images {
                //     let ImageSummary { id, .. } = &image;
                //     // println!("[->] {:?}", image);
                //     println!("[->] Container ID {:?}", id);
                // }
                Ok(())
            }
        },
        Some(Commands::Ps(Ps { command })) => match command {
            // ./exe ps info
            PsOptions::Info => {
                let mut list_container_filters = HashMap::new();
                list_container_filters.insert("status", vec!["running"]);

                let containers = &docker
                    .list_containers(Some(ListContainersOptions {
                        all: true,
                        filters: list_container_filters,
                        ..Default::default()
                    }))
                    .await?;

                // let docker_stream = stream::repeat(docker);
                // docker_stream
                //     .zip(stream::iter(containers))
                //     .for_each_concurrent(2, conc)
                //     .await;
                // println!("[#] Running container {:?}", containers);

                // Container Summary table
                let mut container_summary_table = Table::new();
                container_summary_table
                    .add_row(row![b->"ID", b->"Container Name", b->"Image", b->"State"]);

                for ContainerSummary {
                    id,
                    names,
                    image,
                    state,
                    ..
                } in containers.iter()
                {
                    let container_summary_row = Row::new(vec![
                        Cell::new(&id.as_deref().unwrap_or("")[..12]),
                        Cell::new(
                            &names
                                .as_ref()
                                .map_or_else(|| "n/a".to_string(), |vec| vec.join(", "))
                                .strip_prefix("/")
                                .unwrap_or_else(|| "n/a"),
                        ),
                        Cell::new(image.as_deref().unwrap_or("")),
                        Cell::new(state.as_deref().unwrap_or("")),
                    ]);

                    container_summary_table.add_row(container_summary_row);
                }

                // Print the table to stdout
                container_summary_table.printstd();

                Ok(println!("All Running Docker Containers Info"))
            }
            // ./exe ps kill <container_name>
            PsOptions::Kill(opt) => {
                let options = KillContainerOptions { signal: "SIGTERM" };
                match opt {
                    ContainerInfo { container_name } => {
                        let _ = &docker.kill_container(container_name, Some(options));
                        Ok(println!("Kills Container ID: {container_name:?}"))
                    }
                }
            }
        },
        None => Ok(()),
    }
}

async fn conc(arg: (Docker, &ContainerSummary)) {
    let (docker, container) = arg;

    let stats = docker
        .inspect_container(
            container.id.as_ref().unwrap(),
            None::<InspectContainerOptions>,
        )
        .await
        .unwrap();
    let ContainerInspectResponse {
        id,
        name,
        image,
        size_root_fs,
        state,
        ..
    } = stats;

    // println!("[#] Container name  {:?}", name);

    // Create the table
    let mut stats_table = Table::new();
    stats_table.add_row(
        row![b->"ID", b->"Container Name", b->"Image ID", b->"Container Size", b->"State",],
    );

    let stats_row = Row::new(vec![
        Cell::new(id.as_deref().unwrap_or("")),
        Cell::new(name.as_deref().unwrap_or("")),
        Cell::new(image.as_deref().unwrap_or("")),
        Cell::new(
            &size_root_fs
                .map(|s| s.to_string())
                .unwrap_or_else(|| String::from("-")),
        ),
        Cell::new(
            state
                .unwrap()
                .status
                .as_ref()
                .map(|st| st.as_ref())
                .unwrap(),
        ),
    ]);
    stats_table.add_row(stats_row);

    // Print the table to stdout
    stats_table.printstd();
}
