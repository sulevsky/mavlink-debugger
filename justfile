import '~/common-justfile'

b:
    cargo build

run:
    ./target/debug/mavlink-debugger tcpout:127.0.0.1:5760

run-serial:
    ./target/debug/mavlink-debugger serial:/dev/tty.usbmodem123456781:115200
