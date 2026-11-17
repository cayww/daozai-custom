//! OpenGL 初始化失败时使用独立的图形后端。
//!
//! 典型场景：笔记本接扩展坞 / DisplayLink 外接屏、远程桌面、显卡驱动缺失时，窗口所在的显示设备
//! 只有 Windows 自带的 OpenGL 1.1（GDI Generic），egui_glow 报「requires OpenGL 2.0+」。
//! 这时改用 Direct3D 12 重新启动，不需要用户拔扩展坞。

/// 兼容图形后端（低功耗显卡优先）：指定 Direct3D 12 或 Vulkan。
fn compatible(mut options:eframe::NativeOptions,backends:eframe::wgpu::Backends)->eframe::NativeOptions {
    options.renderer=eframe::Renderer::Wgpu;
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup)=&mut options.wgpu_options.wgpu_setup {
        setup.instance_descriptor.backends=backends;
        setup.power_preference=eframe::wgpu::PowerPreference::LowPower;
        // 缓冲分配策略：实测改用 MemoryHints::Manual（小内存块池 8–64MB）。
        // dhat+VMMap A/B（同为新鲜进程、展开态悬停 15s 后测，2026-11-17）：
        //   Performance（wgpu 默认，大块池）：工作集 303MB / 私有提交 600MB
        //   MemoryUsage（精确分配）：反而比 Performance 多 ~85MB 提交——
        //     egui 每帧重建流式缓冲，精确分配产生大量 D3D 缓冲对象，驱动侧
        //     固定开销超过省下的池化空间
        //   Manual（8–64MB）：工作集 230MB / 私有提交 288MB —— 提交砍半、
        //     工作集 −24%，本负载资源集合小且稳定，块开销可忽略
        // 收起深休眠时另有工作集裁剪（见 resources::trim_working_set），
        // 任务管理器内存列从 ~230MB 掉到 ~30MB。
        // 回退开关：DAOZAI_WGPU_MEMHINTS=performance 用回 wgpu 默认。
        setup.device_descriptor=std::sync::Arc::new(|adapter| {
            let base_limits=if adapter.get_info().backend==eframe::wgpu::Backend::Gl {
                eframe::wgpu::Limits::downlevel_webgl2_defaults()
            }else{
                eframe::wgpu::Limits::default()
            };
            eframe::wgpu::DeviceDescriptor {
                label:Some("daozai wgpu device (Manual 8-64MB)"),
                required_features:eframe::wgpu::Features::default(),
                required_limits:eframe::wgpu::Limits{
                    max_texture_dimension_2d:8192,
                    ..base_limits
                },
                memory_hints:match std::env::var("DAOZAI_WGPU_MEMHINTS").as_deref().map(str::trim) {
                    Ok(v) if v.eq_ignore_ascii_case("memory_usage")=>eframe::wgpu::MemoryHints::MemoryUsage,
                    Ok(v) if v.eq_ignore_ascii_case("performance")=>eframe::wgpu::MemoryHints::Performance,
                    _=>eframe::wgpu::MemoryHints::Manual {
                        suballocated_device_memory_block_size:(8*1024*1024)..(64*1024*1024),
                    },
                },
            }
        });
    }
    options
}

/// 兼容渲染优先用 Direct3D 12（Windows 原生、睡眠唤醒与换屏处理最稳）；
/// 设备不支持时再退到 Vulkan。两者都不经过显卡的 OpenGL 驱动。
fn launch_compatible(options:eframe::NativeOptions,launch:&mut impl FnMut(eframe::NativeOptions)->eframe::Result<()>)->eframe::Result<()> {
    match launch(compatible(options.clone(),eframe::wgpu::Backends::DX12)) {
        Err(eframe::Error::Wgpu(_))=>launch(compatible(options,eframe::wgpu::Backends::VULKAN)),
        result=>result,
    }
}

/// 验收与排障：设置环境变量 `DAOZAI_RENDERER=wgpu` 直接使用兼容图形后端，
/// 在正常电脑上也能检查 OpenGL 失败后用户会看到的画面。
fn forced_compatible()->bool {
    std::env::var("DAOZAI_RENDERER").is_ok_and(|value|value.trim().eq_ignore_ascii_case("wgpu"))
}

pub fn run(mut options:eframe::NativeOptions,mut launch:impl FnMut(eframe::NativeOptions)->eframe::Result<()>)->eframe::Result<()> {
    // 上次崩在显卡 OpenGL 驱动里时，崩溃处理留下的标记会在这里转成设置。
    let _=daozai::renderer::apply_pending_fallback();
    if forced_compatible() || daozai::renderer::compat_requested() {
        daozai::renderer::set_compat_active(true);
        return launch_compatible(options,&mut launch);
    }
    options.renderer=eframe::Renderer::Glow;
    let fallback_options=options.clone();
    let error=match launch(options) {
        Ok(())=>return Ok(()),
        Err(error @ (eframe::Error::OpenGL(_)|eframe::Error::Glutin(_)|eframe::Error::NoGlutinConfigs(..)))=>error,
        Err(error)=>return Err(error),
    };
    daozai::renderer::set_compat_active(true);
    launch_compatible(fallback_options,&mut launch).map_err(|fallback|eframe::Error::AppCreation(format!(
        "OpenGL 初始化失败：{error}\n兼容图形后端也无法启动：{fallback}\n请安装电脑厂商提供的显卡驱动后重试。"
    ).into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opengl_20_failure_retries_without_opengl(){
        let mut attempts=0;
        let result=run(eframe::NativeOptions::default(),|options|{
            attempts+=1;
            if attempts==1 {
                assert_eq!(options.renderer,eframe::Renderer::Glow);
                return Err(eframe::Error::OpenGL("egui_glow requires OpenGL 2.0+.".to_owned().into()));
            }
            assert_eq!(options.renderer,eframe::Renderer::Wgpu);
            let eframe::egui_wgpu::WgpuSetup::CreateNew(setup)=options.wgpu_options.wgpu_setup else{panic!("应创建兼容图形设备")};
            assert!(!setup.instance_descriptor.backends.contains(eframe::wgpu::Backends::GL));
            Ok(())
        });
        assert!(result.is_ok());assert_eq!(attempts,2);
    }

    #[test]
    fn compatible_rendering_prefers_direct3d_12_then_vulkan(){
        let mut backends=Vec::new();
        let result=launch_compatible(eframe::NativeOptions::default(),&mut |options|{
            let eframe::egui_wgpu::WgpuSetup::CreateNew(setup)=options.wgpu_options.wgpu_setup else{panic!("应创建兼容图形设备")};
            backends.push(setup.instance_descriptor.backends);
            if backends.len()==1 {Err(eframe::Error::Wgpu(eframe::egui_wgpu::WgpuError::NoSuitableAdapterFound("测试".into())))} else {Ok(())}
        });
        assert!(result.is_ok());
        assert_eq!(backends,[eframe::wgpu::Backends::DX12,eframe::wgpu::Backends::VULKAN],"先 Direct3D 12，失败再 Vulkan");
    }

    #[test]
    fn application_errors_do_not_restart_the_application(){
        let mut attempts=0;
        assert!(run(eframe::NativeOptions::default(),|_|{
            attempts+=1;Err(eframe::Error::AppCreation("应用初始化失败".into()))
        }).is_err());
        assert_eq!(attempts,1);
    }

    #[test]
    fn successful_opengl_run_is_not_restarted(){
        let mut attempts=0;
        run(eframe::NativeOptions::default(),|options|{
            attempts+=1;assert_eq!(options.renderer,eframe::Renderer::Glow);Ok(())
        }).unwrap();
        assert_eq!(attempts,1);
    }

    #[test]
    fn both_failures_are_preserved_in_the_report(){
        let mut attempts=0;
        let error=run(eframe::NativeOptions::default(),|_|{
            attempts+=1;
            if attempts==1{Err(eframe::Error::OpenGL("OpenGL 2.0".to_owned().into()))}
            else{Err(eframe::Error::AppCreation("没有可用的图形设备".into()))}
        }).unwrap_err().to_string();
        assert!(error.contains("OpenGL 2.0") && error.contains("没有可用的图形设备"));
        assert_eq!(attempts,2);
    }
}
