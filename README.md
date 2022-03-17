# ESPHome Rust PoC

This is a rust implementation of the ESPHome API for an ESP32-C3.

## Why?
I was looking for an embedded Rust project and was curious how the ESPHome API works.

## Will you port everything to Rust?
No! My main goal is to replace a few of my ESP at home with Rust, nothing more.

## Features
Not many ...

### Light
There is a simple on/off light (`ColorMode::COLOR_MODE_ON_OFF`) as well as a RGB LED (`ColorMode::COLOR_MODE_BRIGHTNESS, ColorMode::COLOR_MODE_RGB`) implemented.

### Sensor
I've hocked up a BME280 via I2C but the corresponsing crate is broken. So. while the code works it is useless.

### mDNS
Name is advertised as `esphome-rs-poc.local`

### Log
Stubbed, nothing more.

## How to build
`cargo build --features=native`

## How to flash
`espflash [--monitor] --speed 460800 /dev/ttyUSB0 target/riscv32imc-esp-espidf/debug/esphome-rs-poc`

## Special Thanks
- @ivmarkov for their [std-demo](https://github.com/ivmarkov/rust-esp32-std-demo)
- The folks at [esp-rs](https://matrix.to/#/#esp-rs:matrix.org) 
- Eveybody involved in ESP32 IDF Rust support!