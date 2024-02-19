// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
        _ => Err(Error::InvalidPattern(
            "Vendor ID does not match Quadcast's vendors' ID.".to_owned(),
        )),
    }
}

fn to_product_id(id: u16) -> Result<ProductIdType, Error> {
    match id {
        0x171f => Ok(ProductIdType::NA),
        0x0f8b => Ok(ProductIdType::EU1),
        0x028c => Ok(ProductIdType::EU2),
        0x048c => Ok(ProductIdType::EU3),
        0x068c => Ok(ProductIdType::EU4),
        _ => Err(Error::InvalidPattern(
            "Product ID does not match Quadcast's product IDs.".to_owned(),
        )),
    }
}

fn main() {
    for device in rusb::devices().unwrap().iter() {
        let device_desc = device.device_descriptor().unwrap();

        let vendor_id = to_vendor_id(device_desc.vendor_id());
        let product_id = to_product_id(device_desc.product_id());

        if vendor_id.is_ok() && product_id.is_ok() {
            Microphone {
                vendor: vendor_id.unwrap(),
                product: product_id.unwrap(),
            }
            .print_value()
        }
    }

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
