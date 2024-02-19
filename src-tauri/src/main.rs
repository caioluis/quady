// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::Duration;

use rusb::{Device, DeviceHandle, UsbContext};
use tauri::utils::Error;

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[derive(Debug, Clone, Copy)]
enum VendorIdType {
    NA,
    EU,
}

#[derive(Debug, Clone, Copy)]
enum ProductIdType {
    NA,
    EU1,
    EU2,
    EU3,
    EU4,
}

struct Microphone {
    vendor: VendorIdType,
    product: ProductIdType,
}

impl Microphone {
    fn print_value(&self) {
        let microphone_type = match (self.vendor, self.product) {
            (VendorIdType::NA, ProductIdType::NA) => "NA",
            (VendorIdType::EU, ProductIdType::EU1) => "EU1",
            (VendorIdType::EU, ProductIdType::EU2) => "EU2",
            (VendorIdType::EU, ProductIdType::EU3) => "EU3",
            (VendorIdType::EU, ProductIdType::EU4) => "EU4",
            _ => "Unknown",
        };

        println!("This is microphone type: {}.", microphone_type);
    }
}

fn to_vendor_id(id: u16) -> Result<VendorIdType, Error> {
    match id {
        0x0951 => Ok(VendorIdType::NA),
        0x03f0 => Ok(VendorIdType::EU),
        _ => Err(Error::InvalidPattern("t-v-e".to_owned())),
    }
}

fn to_product_id(id: u16) -> Result<ProductIdType, Error> {
    match id {
        0x171f => Ok(ProductIdType::NA),
        0x0f8b => Ok(ProductIdType::EU1),
        0x028c => Ok(ProductIdType::EU2),
        0x048c => Ok(ProductIdType::EU3),
        0x068c => Ok(ProductIdType::EU4),
        _ => Err(Error::InvalidPattern("t-p-e".to_owned())),
    }
}
#[derive(Debug)]
struct Endpoint {
    config: u8,
    iface: u8,
    setting: u8,
    address: u8,
}

// returns all readable endpoints for given usb device and descriptor
fn find_readable_endpoints<T: UsbContext>(device: &mut Device<T>) -> rusb::Result<Vec<Endpoint>> {
    let device_desc = device.device_descriptor()?;
    let mut endpoints = vec![];
    for n in 0..device_desc.num_configurations() {
        let config_desc = match device.config_descriptor(n) {
            Ok(c) => c,
            Err(_) => continue,
        };
        // println!("{:#?}", config_desc);
        for interface in config_desc.interfaces() {
            for interface_desc in interface.descriptors() {
                // println!("{:#?}", interface_desc);
                for endpoint_desc in interface_desc.endpoint_descriptors() {
                    // println!("{:#?}", endpoint_desc);
                    endpoints.push(Endpoint {
                        config: config_desc.number(),
                        iface: interface_desc.interface_number(),
                        setting: interface_desc.setting_number(),
                        address: endpoint_desc.address(),
                    });
                }
            }
        }
    }

    Ok(endpoints)
}

fn main() {
    for mut device in rusb::devices().unwrap().iter() {
        let device_desc = device.device_descriptor().unwrap();

        let vendor_id = to_vendor_id(device_desc.vendor_id());
        let product_id = to_product_id(device_desc.product_id());

        if vendor_id.is_ok() && product_id.is_ok() {
            let endpoints = find_readable_endpoints(&mut device).unwrap();
            let endpoint = endpoints
                .get(1)
                .expect("No Configurable endpoint found on device");

            println!("{:#04x}", endpoint.config);
            println!("{:#04x}", endpoint.iface);
            println!("{:#04x}", endpoint.setting);
            println!("{:#04x}", endpoint.address);

            Microphone {
                vendor: vendor_id.unwrap(),
                product: product_id.unwrap(),
            }
            .print_value();
        }
    }

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
