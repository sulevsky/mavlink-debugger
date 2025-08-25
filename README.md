# Mavlink debugger

## Build
```sh
cargo build
```

## Run tests
```sh
cargo test
```

## Run
```sh
 ./target/debug/mavlink-debugger tcpout:127.0.0.1:5760
```

## Plan
[TODO] add asciicinema
[TODO] mission edit
[TODO] enter button
[TODO] parameters edit
[TODO] save settings/plan to local file, load from file 
[TODO] check why first parameter and mission item is not selected by default 
[TODO] check mavlink async methods `pub async fn connect_async<M: Message + Sync + Send>` 
