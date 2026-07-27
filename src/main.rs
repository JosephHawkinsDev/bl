use core::sync::atomic::{AtomicBool, Ordering};
use dht_sensor::dht11;
use dht_sensor::DhtReading;
use esp_idf_svc::hal::gpio::Pull;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::delay::Ets;
use esp_idf_svc::hal::gpio::{AnyIOPin, PinDriver};
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::http::server::{Configuration as HttpConfig, EspHttpServer};
use esp_idf_svc::http::Method;
use esp_idf_svc::io::Write;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{BlockingWifi, ClientConfiguration, Configuration, EspWifi};
use std::sync::Mutex;

#[toml_cfg::toml_config]
pub struct Config {
    #[default("")]
    wifi_ssid: &'static str,
    #[default("")]
    wifi_psk: &'static str,
}

static BLINKING: AtomicBool = AtomicBool::new(false);
static READINGS: Mutex<(f32, f32)> = Mutex::new((0.0, 0.0)); // (temp, humidity)

const PAGE: &str = r#"<!DOCTYPE html>
<html><head><title>esp32</title>
<meta name="viewport" content="width=device-width,initial-scale=1">
<style>
  body{font-family:sans-serif;text-align:center;margin-top:10vh}
  a{display:inline-block;padding:1em 2em;margin:0.5em;background:#333;
    color:#fff;text-decoration:none;border-radius:8px}
  .card{display:inline-block;padding:1em 2em;margin:1em;background:#f0f0f0;
    border-radius:8px;min-width:120px}
  .val{font-size:2em;font-weight:bold}
</style></head>
<body>
  <h1>ESP32 Sensor</h1>
  <div class="card"><div class="val" id="temp">--</div>&#176;F</div>
  <div class="card"><div class="val" id="hum">--</div>rh %</div>
  <br>
  <a href="/on">start blink</a><a href="/off">stop blink</a>
  <script>
    async function update() {
      const r = await fetch('/data');
      const d = await r.json();
      document.getElementById('temp').textContent = d.temp.toFixed(1);
      document.getElementById('hum').textContent = d.humidity.toFixed(1);
    }
    update();
    setInterval(update, 3000);
  </script>
</body></html>"#;

fn main() -> anyhow::Result<()> {
    let app_config = CONFIG;
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    let mut led = PinDriver::output(peripherals.pins.gpio2)?;
    // D4 = GPIO4, open-drain so it can be both input and output (DHT11 needs this)
    let mut dht_pin = PinDriver::input_output_od(
        unsafe { AnyIOPin::steal(4) },
		Pull::Up,
    )?;

    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sysloop.clone(), Some(nvs))?,
        sysloop,
    )?;
    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: app_config.wifi_ssid.try_into().unwrap(),
        password: app_config.wifi_psk.try_into().unwrap(),
        ..Default::default()
    }))?;
    wifi.start()?;
    wifi.connect()?;
    wifi.wait_netif_up()?;
    log::info!("ip: {:?}", wifi.wifi().sta_netif().get_ip_info()?);

    let mut server = EspHttpServer::new(&HttpConfig::default())?;

    server.fn_handler("/", Method::Get, |req| {
        req.into_ok_response()?.write_all(PAGE.as_bytes())
    })?;

    // JSON endpoint — page fetches this every 3 seconds
    server.fn_handler("/data", Method::Get, |req| {
        let (temp, humidity) = *READINGS.lock().unwrap();
        let json = format!(r#"{{"temp":{:.1},"humidity":{:.1}}}"#, temp, humidity);
        let mut resp = req.into_ok_response()?;
        resp.write_all(json.as_bytes())
    })?;

    server.fn_handler("/on", Method::Get, |req| {
        BLINKING.store(true, Ordering::Relaxed);
        req.into_response(302, None, &[("Location", "/")])?;
        Ok::<(), anyhow::Error>(())
    })?;

    server.fn_handler("/off", Method::Get, |req| {
        BLINKING.store(false, Ordering::Relaxed);
        req.into_response(302, None, &[("Location", "/")])?;
        Ok::<(), anyhow::Error>(())
    })?;

    let mut delay = Ets;
    let mut tick = 0u32;

    loop {
        // Read DHT11 every 3 seconds (every 30 ticks of 100ms)
        if tick % 30 == 0 {
            match dht11::Reading::read(&mut delay, &mut dht_pin) {
                Ok(r) => {
                    let mut readings = READINGS.lock().unwrap();
					let temp_f = r.temperature as f32 * 9.0 / 5.0 + 32.0;
					let humidity = r.relative_humidity as f32;
					let uptime_s = unsafe { esp_idf_svc::sys::esp_timer_get_time() } / 1_000_000;
					log::info!("[{:02}:{:02}:{:02}] temp: {:.1}°F  humidity: {}%",
						uptime_s / 3600,
						(uptime_s % 3600) / 60,
						uptime_s % 60,
						temp_f, humidity);
					
					*readings = (temp_f, humidity);
					//log::info!("temp: {}°F  humidity: {}%", temp_f, humidity);
                }
                Err(_e) => {},
            }
        }

        if BLINKING.load(Ordering::Relaxed) {
            led.set_high()?;
            esp_idf_svc::hal::delay::FreeRtos::delay_ms(500);
            led.set_low()?;
            esp_idf_svc::hal::delay::FreeRtos::delay_ms(500);
        } else {
            led.set_low()?;
            esp_idf_svc::hal::delay::FreeRtos::delay_ms(100);
        }

        tick = tick.wrapping_add(1);
    }
}