# Installation

The Rust on ESP book [chapter](https://docs.esp-rs.org/book/installation/riscv-and-xtensa.html)

```shell
cargo install espup --locked
espup uninstall
espup install

cargo install esp-generate
esp-generate --chip=esp32s3 dummy-button

source export-esp.nu
# For bash use the default export command
# . $HOME/export-esp.sh

cargo run --release 
```
