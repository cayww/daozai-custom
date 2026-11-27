//! 兼容渲染（`UpdateLayeredWindow`）连续提交几千帧不同画面，进程内存和 GDI 对象都不能随帧数增长。
//!
//! 单独成一个测试二进制：这项断言量的是**整个进程**的私有内存，和别的测试同进程并行跑时，
//! 别的测试线程的临时分配会被算进增长里（实测会假报 +8MB）。独立进程里只有自己的分配。
#![cfg(windows)]

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::{GetCurrentProcess, GetGuiResources, GR_GDIOBJECTS, GR_USEROBJECTS};
use windows::Win32::UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE};

#[test]
fn presenting_many_frames_does_not_leak() {
    unsafe {
        let (w, h) = (960usize, 285usize);
        let owner = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            windows::core::w!("BUTTON"),
            windows::core::w!(""),
            WS_POPUP | WS_VISIBLE,
            -10000,
            -10000,
            w as i32,
            h as i32,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let mut layered = daozai::layered::Layered::new().unwrap();
        let mut frame = vec![0u8; w * h * 4];
        let mut present = |n: u32, layered: &mut daozai::layered::Layered| {
            for i in 0..n {
                frame[(i as usize * 4) % (w * h * 4)] = (i % 251) as u8;
                frame[3] = (i % 200) as u8;
                assert!(layered.present(owner, [w, h], &frame).unwrap(), "每帧画面不同，必须真正提交");
            }
        };
        present(300, &mut layered);
        let gdi = |kind| GetGuiResources(GetCurrentProcess(), kind);
        let (mem0, gdi0, user0) = (
            daozai::resources::snapshot().unwrap().private_bytes,
            gdi(GR_GDIOBJECTS),
            gdi(GR_USEROBJECTS),
        );
        present(3000, &mut layered);
        let (mem1, gdi1, user1) = (
            daozai::resources::snapshot().unwrap().private_bytes,
            gdi(GR_GDIOBJECTS),
            gdi(GR_USEROBJECTS),
        );
        eprintln!(
            "3000 帧：私有内存 {:+.1} MB，GDI {gdi0}→{gdi1}，USER {user0}→{user1}",
            (mem1 as f64 - mem0 as f64) / 1048576.0
        );
        assert!(mem1 < mem0 + 8 * 1024 * 1024, "3000 帧后私有内存增长超过 8MB");
        assert!(gdi1 <= gdi0 + 2 && user1 <= user0 + 2, "GDI/USER 对象随帧数增长");
        drop(layered);
        let _ = DestroyWindow(owner as HWND);
    }
}
