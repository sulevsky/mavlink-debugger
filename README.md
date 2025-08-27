# Mavlink debugger
`mavlink-debugger` is a simple command-line tool to watch Mavlink events stream

[![asciicast](https://asciinema.org/a/736064.svg)](https://asciinema.org/a/736064)

## Requirements
- [Rust](https://www.rust-lang.org/)
- [Cargo package manager](https://doc.rust-lang.org/cargo/)

## Installation
```sh
cargo install --git https://github.com/sulevsky/mavlink-debugger
```

Or download source code and build `mavlink-debugger` manually
```sh
git clone https://github.com/sulevsky/mavlink-debugger
cd mavlink-debugger
cargo build --release
```
Executable location is `target/release/mavlink-debugger`

## Run
```sh
mavlink-debugger tcpout:127.0.0.1:5760
```
### Connection address format

The application expects a connection address in the format:
`(tcpout|udpin|serial):(ip|path):(port|baud)`

Examples:
- TCP: `tcpout:0.0.0.0:5760`
- UDP: `udpin:0.0.0.0:14550`
- Serial: `serial:/dev/tty.usbmodem1101:57600`

## Development
### Build
```sh
cargo build --release
```
### Run tests
```sh
cargo test
```
### Run
```sh
cargo run -- "tcpout:127.0.0.1:5760"
```
### Additionally

#### Tests
```sh
cargo test
```
#### Format
```sh
cargo format
```
#### Linter
```sh
cargo clippy
```

## Future improvements
- mission edit
- parameters edit
- save settings/plan to local file, load from file 
- check mavlink async methods `pub async fn connect_async<M: Message + Sync + Send>` 
