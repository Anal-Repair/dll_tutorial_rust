use std::{net::TcpStream, sync::Mutex, time::Duration};

use windows_sys::{s, Win32::{System::SystemServices::{
    DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, DLL_THREAD_ATTACH, DLL_THREAD_DETACH,
}, UI::WindowsAndMessaging::MessageBoxA}};




fn dll_main() { 
    let stream = TcpStream::connect("127.0.0.1:7331").unwrap();

    tracing_subscriber::fmt()
        .with_writer(Mutex::new(stream))
        .init();

    log::info!("Hello from inside notepad!");
    loop { 
        log::info!("Code running inside notepad!");
        std::thread::sleep(Duration::from_millis(1000));
    }
}




use u32 as DWORD;
type LPVOID = *mut core::ffi::c_void;

#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub unsafe extern "stdcall" fn DllMain(module: usize, reason: DWORD, _: LPVOID) -> u8 {
    match reason {
        DLL_PROCESS_ATTACH => {
            std::thread::spawn(move || dll_main());
            1
        }
        DLL_PROCESS_DETACH | DLL_THREAD_ATTACH | DLL_THREAD_DETACH => 1,
        _ => 0,
    }
}

