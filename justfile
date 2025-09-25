import '~/common-justfile'

b:
    cargo build

run:
    ./target/debug/mavlink-debugger tcpout:127.0.0.1:5760

run-serial:
    ./target/debug/mavlink-debugger serial:/dev/tty.usbmodem123456781:115200

run-log-reader:
    ./target/debug/log_reader serial:/dev/tty.usbmodem123456781:115200

build-run-log-reader:
    @just b
    @just run-log-reader
