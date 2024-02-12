# ESPHome Rust PoC

This is a rust implementation of the ESPHome API for an ESP32-C3.
![image](https://user-images.githubusercontent.com/2230104/158893398-22839275-8f7f-4a48-909d-9974edda332a.png)


## Why?
I was looking for an embedded Rust project and was curious how the ESPHome API works.

## Will you port everything to Rust?
No! My main goal is to (maybe ) replace a few of my ESPs at home with Rust, nothing more.

## Features
Not many ...

### Light
The following light [components](https://esphome.io/#light-components) are implemented:
- [binary](https://esphome.io/components/light/binary.html)
- [monochromatic](https://esphome.io/components/light/monochromatic.html)
- [RGB](https://esphome.io/components/light/rgb.html)

### Sensor
BME280 via I²C works, supporting temperature, humidity and preasure. To enable use feature `"has_bme280"`.

### mDNS
Name is advertised as `esphome-rs-poc.local`

### Log
Colors result in a stack overflow, beside that, it is working.

## How to build
`cargo build --features=native`

## How to flash
`espflash [--monitor] --speed 460800 /dev/ttyUSB0 target/riscv32imc-esp-espidf/debug/esphome-rs-poc`

## Special Thanks
- [ivmarkov](https://github.com/ivmarkov) for their [std-demo](https://github.com/ivmarkov/rust-esp32-std-demo)
- The folks at [esp-rs](https://matrix.to/#/#esp-rs:matrix.org) 
- Eveybody involved in ESP32 IDF Rust support!
