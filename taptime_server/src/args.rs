use std::net::SocketAddr;

use clap::Parser;

#[derive(Debug, clap::Parser)]
#[command(name = "taptime server")]
#[command(version, about, long_about = None)]
#[command(next_line_help = false)]
pub struct Args {
  #[arg(short, long, default_value = "127.0.0.1:50051")]
  pub address: SocketAddr,

  #[arg(short = 'L', long, default_value_t = tracing::Level::INFO)]
  pub log_level: tracing::Level,
}

#[must_use]
pub fn parse() -> Args {
  Args::parse()
}
