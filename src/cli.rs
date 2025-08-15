use clap::Parser;

/**
Simple program to debug Mavlink messages
*/
#[derive(Parser)]
#[command(about)]
pub struct Args {
    /// (tcpout|tcpin|udpout|udpin|udpbcast|serial|file):(ip|dev|path):(port|baud)
    /// ex. `tcpout:127.0.0.1:5760`
    #[arg(required = true)]
    pub address: String,
}
