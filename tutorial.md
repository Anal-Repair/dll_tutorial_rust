In this guide ill be showing you how to do the following 
- Create and compile a DLL using Rust 
- Inject said DLL into the process using `dll_syringe`
- Hookup `log` output to a `TcpStreamer` so we can collect logs outside of the process

## dll_example.dll

First we can start by creating a new library crate

```powershell
cargo new dll_example --lib
```

Then to compile the library as a DLL we need to edit the `Cargo.toml` and append this to the end

```toml
[lib]
name = "inject_me"
path = "src/lib.rs"
crate-type = ["cdylib"]
```

Here the `crate-type` is what instructs the compiler to create a dynamic system library with results in a DLL on windows 

We also going to using be a separate binary that injects the DLL and then listens for the logging information over `TcpListener` so that if we encounter a panic or crash we can get some basic information back for debugging.

Create a new file in `src/` called `main.rs` and just give it a basic main function

```rs
fn main() { }
```

```toml
[[bin]]
name = "injector"
path = "src/main.rs"
```

```
C:.
│   Cargo.lock
│   Cargo.toml
│  
└───src
        lib.rs
        main.rs   
```

So we have a basic project structure here. `main.rs` is going to be the DLL injector and the logging listener and `lib.rs` will be our compiled DLL. 

For a DLL to begin execution in our process we need to define an entry point that will be called upon loading with `LoadLibrary` and unloaded when using `FreeLibrary`

This is the [DllMain]](https://learn.microsoft.com/en-us/windows/win32/dlls/dllmain) and in the article linked the code is for C++ but we are writing Rust! Luckily there is a nice crate provided by Microsoft called [windows-sys](https://crates.io/crates/windows) that lets us easily call any Windows API. (It also has a ton of other nice things such as constants)

On the main page for the windows crate you will see that the example `Cargo.toml` lists a ton of `features=` If you are looking for a specific function from the windows API documentation. Make sure that you have the correct feature flags enabled

Add the following to your `Cargo.toml`
```toml
[dependencies]

windows-sys = {version = "0.52.0", features = [
                "Win32_Foundation",
                "Win32_System_SystemServices"
                ]} 
```

Head over to `lib.rs` and just delete everything. We don't need any unit tests 😎 and replace it with this

```rust 
//lib.rs
use windows_sys::Win32::System::SystemServices::{
    DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, DLL_THREAD_ATTACH, DLL_THREAD_DETACH,
};

use u32 as DWORD;
type LPVOID = *mut core::ffi::c_void;

  
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub unsafe extern "stdcall" fn DllMain(module: usize, reason: DWORD, _: LPVOID) -> u8 {
    match reason {
        DLL_PROCESS_ATTACH => {
            1
        }
        DLL_PROCESS_DETACH | DLL_THREAD_ATTACH | DLL_THREAD_DETACH => 1,
        _ => 0,
    }
}
```

Above is a barebones `DllMain` there are a few interesting things going on here
- what on earth is `#[no_mangle]`
- why do we need `unsafe extern "stdcall"`

#### #[no_mangle]
This instructs the compiler not to modify the function name at compile time meaning that the code can be called externally, in this example `LoadLibary` needs to find the `DllMain` and if the compiler mangles it to `aoefhwfouihwfuDllMain` that the entry point will never be called.

#### unsafe extern "stdcall" 
This just defines the calling convention/ABI (application binary interface) there are a number of options you can find them [here](https://doc.rust-lang.org/reference/items/external-blocks.html)
specifically we are interested in `stdcall` which is the default for the Win32 API (`LoadLibrary` calls our function!) we could also just use `system` but its better to be explicit here.

### Injecting

Now that we have a barebones DLL we could compile this and inject into a process but not much would happen because after `DLL_PROCESS_ATTACH` we just return 1 and also we don't have any way to execute our code!

At this point nothing is stopping us from writing our own `LoadLibary` injector using `windows-rs` but for this tutorial we are just going to use the `dll_syringe` crate 
```
cargo add dll_syringe
```

```rust
//main.rs
use dll_syringe::{Syringe, process::OwnedProcess};

fn main() {

    let proc = OwnedProcess::find_first_by_name("Notepad").unwrap();
    let syringe = Syringe::for_process(proc);
    let injected_payload = syringe.inject("./target/debug/inject_me.dll").unwrap();

}
```

From this point on we can just use `cargo b; cargo r` so that we first build the DLL and the Binary and then run the injector which will take our debug build and automatically inject it. 

Now we can open Notepad and inject our DLL. But nothing will happen because we have not defined any behaviour. We have a number of options here, you could call `AllocConsole` and spawn a console window attached to the process. For now calling `MessageBoxA` is simple enough task
```rust
//cargo.toml
windows-sys = {version = "0.52.0", features = [
                "Win32_Foundation",
                "Win32_System_SystemServices",
                "Win32_UI_WindowsAndMessaging",
                ]}
                
```

```rust
//lib.rs


fn dll_main() {
    unsafe {
        // Create a message box
        MessageBoxA(0,
            s!("Hello from notepad!"),
            s!("inject_me.dll"),
            Default::default()
        );
    };
}

use u32 as DWORD;
type LPVOID = *mut core::ffi::c_void;

#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub unsafe extern "stdcall" fn DllMain(module: usize, reason: DWORD, _: LPVOID) -> u8 {
    match reason {
        DLL_PROCESS_ATTACH => {
            dll_main(); //added call to dll_main()
            1
        }
        DLL_PROCESS_DETACH | DLL_THREAD_ATTACH | DLL_THREAD_DETACH => 1,
        _ => 0,
    }
}


```

If everything worked correctly you should get a nice message box appear.
You might have noticed that the rest of the window has become frozen. This is due to `MessageBoxA` being a modal dialogue and is running on the main thread of the program. We can avoid this issue by spawning a new thread to run our function. 

```rust 
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
```


### Logging functionality 
For the logging there are a number of options. You could either write the logs to a file, spawn a console (and possibly a panic hook so you can read the output on crash) or my preferred option is just to send all of the data through a `TcpStream`

```
//Cargo.toml 

[dependencies]
tracing = "0.1.40"
color-eyre = "0.6.3"
log = "0.4.21"
tracing-subscriber = "0.3.18"

```

Add the following dependencies using `cargo add`


```rust
//main.rs
use dll_syringe::{Syringe, process::OwnedProcess};
use tracing::metadata::LevelFilter;
use std::{
    io::{Read, Write},
    net::TcpListener,
};

  
fn main() -> color_eyre::eyre::Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::INFO)
        .init();


    let listener = TcpListener::bind("127.0.0.1:7331")?;
    log::info!("Starting debug console...");
    let proc = OwnedProcess::find_first_by_name("Notepad").unwrap();
    let syringe = Syringe::for_process(proc);
    let injected_payload = syringe.inject("./target/debug/inject_me.dll").unwrap();
    let (mut stream, address) = listener.accept()?;
    log::info!("{address} has connected");
    let mut buf = vec![0u8; 1024];
    let mut stdout = std::io::stdout();
    while let Ok(n) = stream.read(&mut buf[..]) {
        stdout.write_all(&buf[..n])?;
    }
    Ok(())

}
```

Now we get a nice output 
```
2024-04-12T21:08:08.228593Z  INFO injector: Starting debug console...
```
All we have to do now is connect to the `TcpStream` in `lib.rs` and send all of our logging through there.

```rust
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

```

And thats it! Its as simple as that 

```
2024-04-12T21:12:18.391703Z  INFO injector: Starting debug console...    
2024-04-12T21:12:18.491669Z  INFO injector: 127.0.0.1:56938 has connected    
2024-04-12T21:12:18.481231Z  INFO inject_me: Hello from inside notepad!
2024-04-12T21:12:18.481371Z  INFO inject_me: Code running inside notepad!
2024-04-12T21:12:19.481484Z  INFO inject_me: Code running inside notepad!    
2024-04-12T21:12:20.481732Z  INFO inject_me: Code running inside notepad!    
2024-04-12T21:12:21.482296Z  INFO inject_me: Code running inside notepad!   
```

Code for this tutorial can be found [here]()