#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod branding;
mod diagnostics;
mod documents;
mod icons;
mod preferences;
mod theme;

fn main() {
    #[cfg(all(windows, not(debug_assertions)))]
    if std::env::args_os().len() > 1 {
        // CLI invocations retain the parent's console; GUI launches create no console.
        // SAFETY: AttachConsole takes only the documented parent-process sentinel.
        unsafe {
            windows_sys::Win32::System::Console::AttachConsole(
                windows_sys::Win32::System::Console::ATTACH_PARENT_PROCESS,
            );
        }
    }
    match nexus_cli::dispatch() {
        Ok(true) => return,
        Ok(false) => {}
        Err(e) => {
            eprintln!("NexusLab: {e}");
            std::process::exit(1);
        }
    }
    let mut options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_icon(branding::window_icon())
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([800.0, 560.0])
            .with_maximized(true),
        renderer: eframe::Renderer::Wgpu,
        vsync: true,
        ..Default::default()
    };
    let mut setup = eframe::egui_wgpu::WgpuSetupCreateNew {
        instance_descriptor: eframe::wgpu::InstanceDescriptor {
            backends: eframe::wgpu::Backends::DX12
                | eframe::wgpu::Backends::VULKAN
                | eframe::wgpu::Backends::METAL,
            ..Default::default()
        },
        ..Default::default()
    };
    if cfg!(windows) {
        setup.native_adapter_selector = Some(std::sync::Arc::new(|adapters, surface| {
            adapters
                .iter()
                .filter(|a| surface.is_none_or(|s| a.is_surface_supported(s)))
                .min_by_key(|a| {
                    let info = a.get_info();
                    (
                        info.device_type == eframe::wgpu::DeviceType::Cpu,
                        if info.backend == eframe::wgpu::Backend::Dx12 {
                            0
                        } else {
                            1
                        },
                        if info.device_type == eframe::wgpu::DeviceType::DiscreteGpu {
                            0
                        } else {
                            1
                        },
                    )
                })
                .cloned()
                .ok_or_else(|| "No compatible DX12 or Vulkan adapter".into())
        }));
    }
    options.wgpu_options.wgpu_setup = eframe::egui_wgpu::WgpuSetup::CreateNew(setup);
    if let Err(e) = eframe::run_native(
        &format!("NexusLab v{} Pro", env!("CARGO_PKG_VERSION")),
        options,
        Box::new(|cc| Ok(Box::new(app::NexusApp::new(cc)))),
    ) {
        eprintln!("NexusLab graphics initialization failed: {e}");
        std::process::exit(1);
    }
}
