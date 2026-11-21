//! Migration build (egui 0.35): the old wgpu/DX12 compatibility-renderer
//! selection is removed. The new stack renders the liquid glass with HLSL on a
//! single D3D11 device + DirectComposition (see egui-directx11 in the sibling
//! repo); eframe is kept only as the egui host for now, using the glow renderer.

pub fn run(
    mut options: eframe::NativeOptions,
    mut launch: impl FnMut(eframe::NativeOptions) -> eframe::Result<()>,
) -> eframe::Result<()> {
    let _ = daozai::renderer::apply_pending_fallback();
    // New stack owns its own composition; clear the inherited DComp toggle.
    std::env::remove_var("DAOZAI_DCOMP");
    options.renderer = eframe::Renderer::Glow;
    launch(options)
}
