// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use rusb::{
    constants::LIBUSB_DT_DEVICE,
    ffi::{libusb_device_handle, libusb_get_descriptor},
    Device, DeviceDescriptor, GlobalContext,
};
use slint::{ModelRc, SharedString};
use std::{
    error::Error,
    mem,
    mem::MaybeUninit,
    sync::mpsc::{self, Receiver, Sender},
    thread, time,
};
slint::include_modules!();

fn list_devices() -> Vec<DeviceData> {
    let mut device_vec: Vec<DeviceData> = Vec::new();
    let devices = rusb::devices().unwrap();

    for device in devices.iter() {
        let desc = device.device_descriptor().unwrap();
        device_vec.push(DeviceData {
            bus: device.bus_number().try_into().unwrap(),
            address: device.address().try_into().unwrap(),
            vendor_id: SharedString::from(format!("{:04x}", desc.vendor_id())),
            product_id: SharedString::from(format!("{:04x}", desc.product_id())),
        });
    }

    device_vec
}

fn get_usb_device(bus: u8, address: u8) -> Result<Device<GlobalContext>, ()> {
    let devices = rusb::devices().unwrap();
    for device in devices.iter() {
        if device.bus_number() == bus && device.address() == address {
            return Ok(device);
        }
    }
    Err(())
}

fn get_device_descriptor(dev_handle: *mut libusb_device_handle) -> Result<DeviceDescriptor, ()> {
    let mut descriptor = MaybeUninit::<DeviceDescriptor>::uninit();

    let res = unsafe {
        libusb_get_descriptor(
            dev_handle,
            LIBUSB_DT_DEVICE,
            0,
            0,
            mem::transmute(descriptor.as_mut_ptr()),
            mem::size_of::<DeviceDescriptor>().try_into().unwrap(),
        )
    };

    if res < 0 {
        Err(())
    } else {
        Ok(unsafe { descriptor.assume_init() })
    }
}

enum ThreadMsg {
    LoopDevice(DeviceData),
    StopLooping,
}

fn main() -> Result<(), Box<dyn Error>> {
    let (tx, rx): (Sender<ThreadMsg>, Receiver<ThreadMsg>) = mpsc::channel();

    thread::spawn(move || loop {
        if let Ok(ThreadMsg::LoopDevice(device)) = rx.recv() {
            let usb_device = get_usb_device(
                device.bus.try_into().unwrap(),
                device.address.try_into().unwrap(),
            )
            .expect("Device not found");
            let dev = usb_device.open().unwrap();

            println!("Looping device descriptor");
            while rx.recv_timeout(time::Duration::from_millis(200)).is_err() {
                get_device_descriptor(dev.as_raw()).unwrap();
            }
            println!("Stopped looping");
        }
    });

    let ui = AppWindow::new()?;
    ui.set_devices(Default::default());

    ui.on_refresh_clicked({
        let ui_handle = ui.as_weak();
        let tx_handle = tx.clone();

        move |looping| {
            let ui = ui_handle.unwrap();

            if looping {
                ui.set_looping(false);
                tx_handle.send(ThreadMsg::StopLooping).unwrap();
            } else {
                let device_vec = list_devices();
                let device_model = ModelRc::from(device_vec.as_slice());
                ui.set_devices(device_model);
            }
        }
    });

    ui.on_device_clicked({
        let ui_handle = ui.as_weak();
        let tx_handle = tx.clone();

        move |device| {
            let ui = ui_handle.unwrap();
            ui.set_looping(true);
            tx_handle.send(ThreadMsg::LoopDevice(device)).unwrap();
        }
    });

    ui.run()?;
    Ok(())
}
