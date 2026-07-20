# bl

WiFi-controlled blinking LED on an ESP32, written in Rust against ESP-IDF.

Serves a small web page on the local network with start/stop controls. The HTTP
handlers flip a shared `AtomicBool`; the main loop owns the GPIO and reads it.

## Hardware

| | |
|---|---|
| Chip | ESP32-D0WD-V3, revision v3.1 |
| Cores | Dual-core Xtensa LX6 @ 240 MHz |
| RAM | 520 KB SRAM total, ~320 KB DRAM. **No PSRAM** (WROOM, not WROVER) |
| Flash | 4 MB (Zbit, mfr `5e` / device `4016`), 3.3 V set by strapping pin |
| Radio | WiFi 2.4 GHz + Bluetooth Classic + BLE. No 802.15.4, so no Zigbee/Thread |
| Crystal | 40 MHz |
| USB-serial | CP2102 (Silicon Labs) — driver ships with Windows 11 |
| MAC | `5c:01:3b:bf:7d:d8` |
| Port | COM5 |

Onboard LED is on **GPIO2**.

Software stack: ESP-IDF v5.5.3, `esp-idf-svc` 0.52.x (std, FreeRTOS-backed).

Practical limits from that spec: ~150–200 KB free heap with WiFi up, less with
BLE alongside. Fine for a BLE presence node or a pixel controller. Not enough
for audio buffering (Snapcast) or a display framebuffer — those need PSRAM.

## Setup

### One-time

1. Rust via [rustup](https://rustup.rs), plus MSVC build tools
   (Visual Studio Installer → Modify → "Desktop development with C++").
2. ESP tooling:
   ```
   cargo install espup espflash cargo-generate ldproxy
   espup install
   ```
3. Allow local scripts, so `export-esp.ps1` will run:
   ```
   Set-ExecutionPolicy -Scope CurrentUser RemoteSigned
   ```
4. Add a Windows Defender exclusion for `C:\esp` and `C:\Users\<you>\.cargo`.
   Build times roughly triple without it.

### Every shell

```
. $env:USERPROFILE\export-esp.ps1
```

Add that line to `$PROFILE` to make it automatic.

### Credentials

`cfg.toml` is gitignored. Copy the example and fill it in:

```
cp cfg.toml.example cfg.toml
```

```toml
[bl]
wifi_ssid = "..."
wifi_psk  = "..."
```

The table name must match the crate name (`bl`) exactly, or `toml-cfg`
silently falls back to empty defaults and WiFi association fails.

These values are compiled into the binary. This keeps them out of git; it does
not protect them from anyone who can read the flash.

## Build and flash

```
cargo run
```

Builds, flashes over COM5, and opens a serial monitor. The IP is printed on
connect. Open it in a browser on the same network.

First build compiles the whole ESP-IDF from source — 15–30 minutes. Everything
after that is incremental.

## Gotchas

**Path length.** `esp-idf-sys` hard-fails if the project path is longer than
about 10 characters, independent of Windows long-path support. Hence `C:\esp\bl`.
Keep it out of OneDrive-redirected Documents.

**Bad data checksum while flashing.** Serial link quality, not a code problem.
Retry first; if it persists, drop the baud rate in `.cargo/config.toml`:

```toml
[target.xtensa-esp32-espidf]
runner = "espflash flash --monitor --baud 115200"
```

Cheap or poorly-seated USB cables are the usual cause. Micro-USB on these boards
seats about a millimetre later than it feels like it does — a lit power LED with
no COM port means power pins are connected and data pins aren't.

**2.4 GHz only.** No 5 GHz radio. A 5 GHz-only SSID will never associate.

**Stack overflow on boot.** The WiFi stack is hungry. Raise it in
`sdkconfig.defaults`, then `cargo clean` (that file is only read at IDF config time):

```
CONFIG_ESP_MAIN_TASK_STACK_SIZE=8000
```

**Duplicate crate versions.** Don't declare `embedded-svc` directly — `esp-idf-svc`
re-exports it. Two copies in the tree produce "expected `Method`, found `Method`".

## Next

- mDNS, so it answers to `bl.local` instead of a DHCP lease
- WiFi credentials in NVS with an AP-mode setup page — reflashing to change
  networks is the current annoyance
- MQTT publish + Home Assistant discovery
- BLE scan and RSSI reporting (needs a rolling median; raw RSSI is noisy)
- Enclosure: fits a snus tin. Plastic doesn't attenuate 2.4 GHz. Keep the PCB
  antenna away from any foil liner, and put a pinhole in the lid for airflow.
