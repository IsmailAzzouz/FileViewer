//! Application executable entry point.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use file_viewer::app::AppView;
use gpui::*;
use std::path::PathBuf;

/// Forces NVIDIA Optimus drivers to select the dedicated high-performance RTX GPU
/// instead of the integrated Intel iGPU.
#[cfg(target_os = "windows")]
#[no_mangle]
#[used]
pub static NvOptimusEnablement: u32 = 0x0000_0001;

/// Forces AMD PowerXpress / Hybrid Graphics drivers to select the discrete Radeon GPU.
#[cfg(target_os = "windows")]
#[no_mangle]
#[used]
pub static AmdPowerXpressRequestHighPerformance: i32 = 1;

#[cfg(target_os = "windows")]
mod gpu_preference {
    use std::os::windows::ffi::OsStrExt;

    type Hkey = *mut std::ffi::c_void;
    type Lstatus = i32;

    const HKEY_CURRENT_USER: Hkey = 0x8000_0001_usize as Hkey;
    const KEY_SET_VALUE: u32 = 0x0002;
    const REG_SZ: u32 = 1;

    #[link(name = "advapi32")]
    extern "system" {
        fn RegCreateKeyExW(
            hKey: Hkey,
            lpSubKey: *const u16,
            Reserved: u32,
            lpClass: *const u16,
            dwOptions: u32,
            samDesired: u32,
            lpSecurityAttributes: *const std::ffi::c_void,
            phkResult: *mut Hkey,
            lpdwDisposition: *mut u32,
        ) -> Lstatus;

        fn RegSetValueExW(
            hKey: Hkey,
            lpValueName: *const u16,
            Reserved: u32,
            dwType: u32,
            lpData: *const u8,
            cbData: u32,
        ) -> Lstatus;

        fn RegCloseKey(hKey: Hkey) -> Lstatus;
    }

    /// Configures Windows 10/11 DirectX `UserGpuPreferences` to `GpuPreference=2;`
    /// (High Performance GPU) for the current executable before DXGI initializes.
    pub fn ensure_high_performance_gpu() {
        std::env::set_var("SHIM_MCCOMPAT", "0x800000001");

        let Ok(exe_path) = std::env::current_exe() else {
            return;
        };

        let subkey: Vec<u16> = "Software\\Microsoft\\DirectX\\UserGpuPreferences\0"
            .encode_utf16()
            .collect();
        let value_name: Vec<u16> = exe_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let value_data: Vec<u16> = "GpuPreference=2;\0".encode_utf16().collect();

        unsafe {
            let mut hkey: Hkey = std::ptr::null_mut();
            let mut disposition: u32 = 0;
            let status = RegCreateKeyExW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                std::ptr::null(),
                0,
                KEY_SET_VALUE,
                std::ptr::null(),
                &mut hkey,
                &mut disposition,
            );
            if status == 0 && !hkey.is_null() {
                let bytes = std::slice::from_raw_parts(
                    value_data.as_ptr() as *const u8,
                    value_data.len() * 2,
                );
                let _ = RegSetValueExW(
                    hkey,
                    value_name.as_ptr(),
                    0,
                    REG_SZ,
                    bytes.as_ptr(),
                    bytes.len() as u32,
                );
                RegCloseKey(hkey);
            }
        }
    }
}

fn main() {
    #[cfg(target_os = "windows")]
    gpu_preference::ensure_high_performance_gpu();

    let file_arg = std::env::args().nth(1).map(PathBuf::from);

    let app = Application::new();

    app.run(move |cx| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: Point {
                    x: px(80.0),
                    y: px(80.0),
                },
                size: Size {
                    width: px(1120.0),
                    height: px(780.0),
                },
            })),
            titlebar: Some(TitlebarOptions {
                title: Some("FileViewer - JSON Editor & Viewer".into()),
                appears_transparent: false,
                ..Default::default()
            }),
            focus: true,
            show: true,
            ..Default::default()
        };

        if let Err(e) = cx.open_window(options, |window, cx| {
            let view = cx.new(|cx| AppView::new(file_arg.clone(), cx));
            view.update(cx, |this, _cx| {
                this.focus_handle().focus(window);
            });
            view
        }) {
            eprintln!("Failed to open main window: {e}");
        }
    });
}
