use std::{net::Ipv4Addr, time::Duration};

use anyhow::bail;
use embedded_svc::wifi::{self, AuthMethod, ClientConfiguration};
use esp_idf_hal::{peripheral, peripherals};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    netif::{EspNetif, EspNetifWait},
    nvs::EspDefaultNvsPartition,
    wifi::{EspWifi, WifiWait},
};
use esp_idf_sys as _; // If using the `binstart` feature of `esp-idf-sys`, always keep this module imported

use log::info;

fn main() -> Result<(), anyhow::Error> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let per = peripherals::Peripherals::take().unwrap();
    let sysloop = EspSystemEventLoop::take().unwrap();

    let bla = wifi("Wokwi-Guest", "", per.modem, sysloop)?;

    println!("Hello, world!");

    loop {
        std::thread::sleep(Duration::from_millis(1000));
    }

    Ok(())
}

pub fn wifi(
    ssid: &str,
    psk: &str,
    modem: impl peripheral::Peripheral<P = esp_idf_hal::modem::Modem> + 'static,
    sysloop: EspSystemEventLoop,
) -> anyhow::Result<Box<EspWifi<'static>>> {
    if ssid.is_empty() {
        anyhow::bail!("missing WiFi name")
    }
    if psk.is_empty() {
        info!("Wifi password is empty");
    }

    let mut wifi = Box::new(EspWifi::new(
        modem,
        sysloop.clone(),
        Some(EspDefaultNvsPartition::take()?),
    )?);

    wifi.start()?;
    info!("Starting wifi...");

    if !WifiWait::new(&sysloop)?
        .wait_with_timeout(Duration::from_secs(20), || wifi.is_started().unwrap())
    {
        bail!("Wifi did not start");
    }

    info!("Searching for Wifi network {}", ssid);

    let ap_infos = wifi.scan()?;

    let ours = ap_infos.into_iter().find(|a| a.ssid == ssid);

    let channel = if let Some(ours) = ours {
        info!(
            "Found configured access point {} on channel {}",
            ssid, ours.channel
        );
        Some(ours.channel)
    } else {
        info!(
            "Configured access point {} not found during scanning, will go with unknown channel",
            ssid
        );
        None
    };

    //std::thread::sleep(Duration::from_millis(10000));

    info!("setting Wifi configuration");
    wifi.set_configuration(&wifi::Configuration::Client(ClientConfiguration {
        ssid: ssid.into(),
        password: psk.into(),
        channel,
        auth_method: AuthMethod::None,
        ..Default::default()
    }))?;

    info!("Connecting wifi...");
    wifi.connect()?;

    while !EspNetifWait::new::<EspNetif>(wifi.sta_netif(), &sysloop)?.wait_with_timeout(
        Duration::from_secs(7),
        || {
            wifi.is_up().unwrap()
                && wifi.sta_netif().get_ip_info().unwrap().ip != Ipv4Addr::new(0, 0, 0, 0)
        },
    ) {
        wifi.connect()?;
    }

    let ip_info = wifi.sta_netif().get_ip_info()?;

    info!("Wifi DHCP info: {:?}", ip_info);

    Ok(wifi)
}
