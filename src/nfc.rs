//! Raspberry Pi 4 demo.
//! This example makes use the `std` feature
//! and `anyhow` dependency to make error handling more ergonomic.
//!
//! # Connections
//!
//! - 3V3    = VCC
//! - GND    = GND
//! - GPIO9  = MISO
//! - GPIO10 = MOSI
//! - GPIO11 = SCLK (SCK)
//! - GPIO8 = NSS  (SDA)
//!
// CREDIT: based on code found in this repo -> https://gitlab.com/jspngh/mfrc522/-/tree/main/examples/rpi4

use linux_embedded_hal as hal;

use std::collections::HashMap;
use std::convert::TryInto;

use std::thread;
use std::time::Duration;

use anyhow::Result;
use embedded_hal::delay::DelayNs;
use hal::spidev::{SpiModeFlags, SpidevOptions};
use hal::SpidevDevice;
use hal::Delay;
use mfrc522::comm::{Interface, blocking::spi::SpiInterface};
use mfrc522::{Initialized, Mfrc522};

const SCAN_DELAY_MS: u16 = 500;
const OPERATING_SYSTEM: &str = std::env::consts::OS;

fn get_spi() -> Result<SpidevDevice, bool> {
    let spi = match SpidevDevice::open("/dev/spidev0.0") {
        Ok(spi) => spi, // return the SPI device if we successfully opened it as a SpidevDevice
        Err(e) => {
            println!("Failed to open SPI device: {:?}", e);
            return Err(false);
        }
    };
    Ok(spi)
}

fn is_linux_os() -> Result<bool> {
    if OPERATING_SYSTEM != "linux" {
        println!(
            "
            \nLINUX OS REQUIRED.
            \nDETECTED: {OPERATING_SYSTEM}.
            \nPROGRAM TERMINATED.\n
            "
        );
        return Ok(false);
    }
    Ok(true) // os is linux
}

pub fn read() -> Result<bool> {
    
    if !is_linux_os()? {
        return Ok(false);
    }

    #[allow(nonstandard_style)]
    let TAGS_HMAP = HashMap::from([
        ([132, 35, 165, 229], "ACCEPTED CARD - SERVICE ENGINEER"),
         ([105, 126, 202, 6], "ACCEPTED CARD - ADMIN (clean card)"),
        ([222, 183, 17, 6], "ACCEPTED TAG - ADMIN (clean tag)"),
    ]);

    let mut delay = Delay;

    // do not change this, these settings are required for the MFRC522 to work properly. (max speed is 10MHz, but we can go lower for stability)
    let options = SpidevOptions::new()
        .max_speed_hz(1_000_000)
        // .mode(SpiModeFlags::SPI_MODE_0 | SpiModeFlags::SPI_NO_CS)
        .mode(SpiModeFlags::SPI_MODE_0)
        .build();

    // i put it into a loop to avoid crashes that kill the process if the SPI device is not ready when we try to open it.
    //this way it will keep retrying every 3 seconds until it successfully opens the SPI device, which is more robust and user-friendly than crashing immediately.

    let mut spi = loop {
        match get_spi() {
            Ok(s) => break s, // multi threading not needed - loop blocks the current process until condition satisfied
            Err(_) => {
                println!("Retrying to open SPI device in 3 second...");
                thread::sleep(Duration::from_secs(3));
                continue;
            }
        }
    };

    spi.configure(&options)
        .expect("Failed to configure SPI device");

    let itf = SpiInterface::new(spi);
    let mut mfrc522 = Mfrc522::new(itf).init()?;

    let vers = mfrc522.version()?;

    if vers == 0x91 || vers == 0x92 {
        println!("MFRC522 Version 1 - {vers}\nPROGRAM INITIALIZING (3 seconds...)");
    } else if vers == 0x90 {
        println!("MFRC522 Version 2 - {vers}\nPROGRAM INITIALIZING (3 seconds...)");
    } else if vers == 0x82{
        println!("OLDER MFRC522 VERSION - {vers}\nPROGRAM INITIALIZING (3 seconds...)");
    } else {
        println!(
            "UNKNOWN MFRC522 VERSION - 0x{:x}\nPROGRAM TERMINATING.",
            vers
        );
        return Ok(false); // kill the program if we cant find version; no point continuing if we possibly cant talk to the card reader
    }

    loop {
        if let Ok(atqa) = mfrc522.reqa() {
            if let Ok(uid) = mfrc522.select(&atqa) {
                let uid_array: [u8; 4] = uid.as_bytes().try_into().unwrap();
                println!("SCANNED UID FOUND: {:?}", uid_array);

                let found = TAGS_HMAP.contains_key(&uid_array);
                if found {
                    println!("{}", TAGS_HMAP[&uid_array]);
                } else {
                    println!("UNKNOWN TAG/CARD DETECTED - NOT IN DB/HMAP");
                }

                return Ok(found);

                // this decrypts the card and reads block 1, which should be empty on new cards, but can be used to store data on used cards;
                // this is just an example of how to read data from a card after authenticating with the default key.

                // handle_authenticate(&mut mfrc522, &uid, |m| {
                //     // read block 1
                //     let data = m.mf_read(1)?;
                //     // print the data - do nothing else. (data can be used later on to store info or run specific commands based on the data read from the card, but for this demo we just print it out)
                //      println!("READ DATA: {:?}", data);    
                // Ok(())
                // })
                // .ok();
            }
        }

        delay.delay_ms(SCAN_DELAY_MS as u32);
    }
}

fn handle_authenticate<E, COMM: Interface<Error = E>, F>(
    // this block is out of my technical expertise...
    mfrc522: &mut Mfrc522<COMM, Initialized>,
    uid: &mfrc522::Uid,
    action: F,
) -> Result<()>
where
    F: FnOnce(&mut Mfrc522<COMM, Initialized>) -> Result<()>,
    E: std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static,
{
    let key = [0xFF; 6];
    if mfrc522.mf_authenticate(uid, 1, &key).is_ok() {
        action(mfrc522)?;
    } else {
        println!("Could not authenticate");
    }

    mfrc522.hlta()?;
    mfrc522.stop_crypto1()?;
    Ok(())
}
