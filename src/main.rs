use core::sync::atomic::{AtomicBool, Ordering};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::gpio::PinDriver;
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::http::server::{Configuration as HttpConfig, EspHttpServer};
use esp_idf_svc::http::Method;
use esp_idf_svc::io::Write;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{BlockingWifi, ClientConfiguration, Configuration, EspWifi};

#[toml_cfg::toml_config]
pub struct Config {
    #[default("")]
    wifi_ssid: &'static str,
    #[default("")]
    wifi_psk: &'static str,
}

static BLINKING: AtomicBool = AtomicBool::new(true);

const PAGE: &str = r#"<!DOCTYPE html>
<html><head><title>esp32</title>
<meta name="viewport" content="width=device-width,initial-scale=1">
<style>body{font-family:sans-serif;text-align:center;margin-top:20vh}
a{display:inline-block;padding:1em 2em;margin:0.5em;background:#333;
color:#fff;text-decoration:none;border-radius:8px}</style></head>
<body><h1>blinky</h1>
<a href="/on">start</a><a href="/off">stop</a></body></html>"#;

fn main() -> anyhow::Result<()> {
	let app_config = CONFIG;
	
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    let mut led = PinDriver::output(peripherals.pins.gpio2)?;

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

    loop {
        if BLINKING.load(Ordering::Relaxed) {
            led.set_high()?;
            FreeRtos::delay_ms(500);
            led.set_low()?;
            FreeRtos::delay_ms(500);
        } else {
            led.set_low()?;
            FreeRtos::delay_ms(100);
        }
    }
}
