//! D3D11 原生材质包（方案 B）。
//!
//! 材质插件只提供一个 `float4 material(MaterialIn i)` 函数；宿主用下面的 HLSL
//! prelude 包住它，编译成 `ps_5_0` DXBC，然后直接用岛仔现有的 D3D11 全屏三角形
//! pass 绘制。`sdk-material`（wgpu/WGSL）那套代码不参与。

use base64::Engine as _;
use std::sync::Mutex;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn fail(text: &str) -> Box<dyn std::error::Error> {
    text.to_owned().into()
}

/// 宿主提供给材质入口的 HLSL 前缀。插件不要重复声明 `U`/`P` cbuffer、`bg`/`samp`、
/// `vs`/`fs` 或 `MaterialIn`，只实现 `material()`。
///
/// `b0` 与内置玻璃的 `GlassParams` 完全一致；`b1` 是插件设置值，8 个 float4
/// 对应清单里最多 32 个 number 设置项。
pub const MATERIAL_PRELUDE: &str = r#"
#define DZ_MAX_CTL 13

cbuffer U : register(b0) {
    float4 bounds;
    float4 params;
    float4 extra;
    float4 tint;
    float4 quad;
    float4 misc;
    float4 crect[DZ_MAX_CTL];
    float4 cprm[DZ_MAX_CTL];
    float4 cstyle[DZ_MAX_CTL];
    float4 look;
    float4 notch;
    float4 lightcfg;
    float4 meta;
    float4 meta2;
};
cbuffer P : register(b1) {
    float4 plugin_params[8];
};

Texture2D<float4> bg : register(t0);
SamplerState samp : register(s0);

struct VSOut {
    float4 pos : SV_POSITION;
    float2 uv  : TEXCOORD0;
};

VSOut dz_vs(uint id : SV_VertexID) {
    float2 xy[3] = { float2(-1,-1), float2(3,-1), float2(-1,3) };
    float2 uv[3] = { float2(0,0),  float2(2,0),  float2(0,2)  };
    VSOut o;
    o.pos = float4(xy[id], 0.0, 1.0);
    o.uv  = uv[id];
    return o;
}

struct MaterialIn {
    float2 uv;        // 画布归一化坐标（0..1）
    float2 pixel;     // 窗口内物理像素，y 从底向上（与 bounds 同基准）
    float4 bounds;    // 岛体矩形 xy=左下，zw=宽高
    float radius;     // 岛体圆角半径（物理像素）
    float4 values[8]; // 插件设置值，按清单 settings 顺序排列
};

// 采样岛体背后的桌面。px 是窗口内、底原点的物理像素。
float3 dz_backdrop(float2 px) {
    uint w, h;
    bg.GetDimensions(w, h);
    float2 screen_px = float2(px.x + quad.y, (extra.z - px.y) + quad.z);
    float2 uv = saturate(screen_px / float2(max(w, 1u), max(h, 1u)));
    if (quad.x > 0.5) uv.y = 1.0 - uv.y;
    return bg.SampleLevel(samp, uv, 0.0).rgb;
}

float dz_rr_sdf(float2 p, float2 b, float r) {
    float2 q = abs(p) - b + r;
    return min(max(q.x, q.y), 0.0) + length(max(q, float2(0,0))) - r;
}

// 岛体形状遮罩：<0 在岛内。第一版只按圆角矩形，刘海/左右停靠形状沿用内置玻璃。
float dz_island_sdf(float2 local) {
    float2 half_size = max(bounds.zw * 0.5 - 3.0, float2(2,2));
    float r = max(min(extra.x - 3.0, min(half_size.x, half_size.y)), 0.0);
    return dz_rr_sdf(local, half_size, r);
}
"#;

/// 必须放在插件源码之后：HLSL 不允许在定义前调用函数。
const MATERIAL_EPILOGUE: &str = r#"
float4 dz_fs(VSOut i) : SV_TARGET {
    float2 px = i.uv * extra.xy;
    float2 local = px - bounds.xy - bounds.zw * 0.5;
    MaterialIn m;
    m.uv = i.uv;
    m.pixel = px;
    m.bounds = bounds;
    m.radius = extra.x;
    [unroll]
    for (int k = 0; k < 8; k++) { m.values[k] = plugin_params[k]; }
    float4 c = material(m);
    float aa = 1.0 - smoothstep(-1.0, 1.0, dz_island_sdf(local));
    float cov = saturate(c.a * aa);
    // DComp 交换链需要预乘 alpha。
    return float4(c.rgb * cov, cov);
}
"#;

#[derive(Default)]
struct ActiveMaterial {
    generation: u64,
    key: String,
    id: Option<String>,
    dxbc: Option<Vec<u8>>,
    params: [f32; 32],
}

fn active() -> &'static Mutex<ActiveMaterial> {
    static ACTIVE: Mutex<ActiveMaterial> = Mutex::new(ActiveMaterial {
        generation: 0,
        key: String::new(),
        id: None,
        dxbc: None,
        params: [0.0; 32],
    });
    &ACTIVE
}

fn lock() -> std::sync::MutexGuard<'static, ActiveMaterial> {
    active().lock().unwrap_or_else(|e| e.into_inner())
}

/// App 每帧同步当前激活的材质。只有源码/入口变化时才重新编译，参数变化只更新 uniform。
pub fn set_active(id: Option<&str>, entry: &str, source: &str, params: [f32; 32]) -> Result<()> {
    if id.is_none() || source.is_empty() {
        clear();
        return Ok(());
    }
    let id = id.unwrap_or_default();
    let key = format!("{id}\0{entry}\0{source}");
    {
        let state = lock();
        if state.key == key && state.dxbc.is_some() {
            drop(state);
            let mut state = lock();
            state.params = params;
            return Ok(());
        }
    }
    let compiled = compile_package_source(entry, source);
    let mut state = lock();
    state.generation += 1;
    state.key = key;
    state.id = Some(id.to_owned());
    state.params = params;
    match compiled {
        Ok(dxbc) => {
            state.dxbc = Some(dxbc);
            Ok(())
        }
        Err(error) => {
            // 记录失败，避免每帧重复编译/刷日志；源码变了自然会重新编译。
            state.dxbc = None;
            Err(fail(&error))
        }
    }
}

/// 没有激活材质时清掉像素着色器，D3D11 运行时回退到内置液态玻璃 pass。
pub fn clear() {
    let mut state = lock();
    if state.dxbc.is_none() && state.id.is_none() {
        return;
    }
    state.generation += 1;
    state.key.clear();
    state.id = None;
    state.dxbc = None;
    state.params = [0.0; 32];
}

/// 渲染线程取当前 DXBC。generation 变化代表需要重建 PS。
pub fn snapshot() -> Option<(u64, Vec<u8>, [f32; 32])> {
    let state = lock();
    let dxbc = state.dxbc.clone()?;
    Some((state.generation, dxbc, state.params))
}

/// 插件设置值（即使没有材质也返回全 0，便于每帧更新 b1）。
pub fn active_params() -> [f32; 32] {
    lock().params
}

/// 安装/加载材质包时调用。`.hlsl` 现场编译，`.dxbc` 是 base64 文本形式的预编译 blob。
pub fn validate_plugin_shader(entry: &str, source: &str) -> std::result::Result<(), String> {
    compile_package_source(entry, source).map(|_| ())
}

/// 返回可以直接喂给 `ID3D11Device::CreatePixelShader` 的 DXBC。
pub fn compile_package_source(entry: &str, source: &str) -> std::result::Result<Vec<u8>, String> {
    if entry.ends_with(".dxbc") {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(source.trim())
            .map_err(|e| format!("材质 DXBC 不是有效的 base64：{e}"))?;
        if bytes.len() < 4 || &bytes[..4] != b"DXBC" {
            return Err("材质 DXBC 缺少 DXBC 文件头".to_owned());
        }
        return Ok(bytes);
    }
    if entry.ends_with(".hlsl") {
        let mut hlsl = String::with_capacity(
            MATERIAL_PRELUDE.len() + source.len() + MATERIAL_EPILOGUE.len() + 2,
        );
        hlsl.push_str(MATERIAL_PRELUDE);
        hlsl.push('\n');
        hlsl.push_str(source);
        hlsl.push_str(MATERIAL_EPILOGUE);
        return compile_hlsl(&hlsl);
    }
    if entry.ends_with(".wgsl") {
        return Err(
            "WGSL 材质包需要转译层；请让插件作者改用 D3D11 HLSL/DXBC 材质包".to_owned(),
        );
    }
    Err("材质包入口必须是 .hlsl 或 .dxbc（base64 文本）".to_owned())
}

#[cfg(windows)]
fn compile_hlsl(hlsl: &str) -> std::result::Result<Vec<u8>, String> {
    use windows::core::s;
    use windows::Win32::Graphics::Direct3D::{Fxc::D3DCompile, ID3DBlob};

    let mut code: Option<ID3DBlob> = None;
    let mut errors: Option<ID3DBlob> = None;
    let result = unsafe {
        D3DCompile(
            hlsl.as_ptr().cast(),
            hlsl.len(),
            None,
            None,
            None,
            s!("dz_fs"),
            s!("ps_5_0"),
            0,
            0,
            &mut code,
            Some(&mut errors),
        )
    };
    if let Err(error) = result {
        let detail = errors
            .as_ref()
            .map(|blob| unsafe {
                std::slice::from_raw_parts(
                    blob.GetBufferPointer().cast::<u8>(),
                    blob.GetBufferSize(),
                )
            })
            .map(|bytes| String::from_utf8_lossy(bytes).trim_matches('\0').trim().to_owned())
            .unwrap_or_default();
        return Err(format!("HLSL 材质编译失败：{error} {detail}"));
    }
    let blob = code.ok_or_else(|| "HLSL 材质编译未返回 DXBC".to_owned())?;
    let bytes = unsafe {
        std::slice::from_raw_parts(
            blob.GetBufferPointer().cast::<u8>(),
            blob.GetBufferSize(),
        )
    };
    Ok(bytes.to_vec())
}

#[cfg(not(windows))]
fn compile_hlsl(_hlsl: &str) -> std::result::Result<Vec<u8>, String> {
    Err("D3D11 材质包只在 Windows 上受支持".to_owned())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
float4 material(MaterialIn i) {
    float3 bg = dz_backdrop(i.pixel);
    float3 tint = float3(0.05, 0.35, 0.75) * saturate(i.values[0].x);
    return float4(lerp(bg, tint, 0.4), 1.0);
}
"#;

    #[test]
    fn hlsl_material_compiles_to_dxbc() {
        let dxbc = compile_package_source("material.hlsl", SAMPLE).expect("示例 HLSL 应能编译");
        assert_eq!(&dxbc[..4], b"DXBC");
    }

    #[test]
    fn wgsl_entry_gets_actionable_error() {
        let err = compile_package_source("material.wgsl", "fn material() {}").unwrap_err();
        assert!(err.contains("HLSL/DXBC"), "{err}");
    }

    #[test]
    fn dxbc_entry_requires_base64_blob_header() {
        let blob = b"DXBC\x01\x02\x03\x04".to_vec();
        let encoded = base64::engine::general_purpose::STANDARD.encode(&blob);
        assert_eq!(compile_package_source("material.dxbc", &encoded).unwrap(), blob);
        let bad = base64::engine::general_purpose::STANDARD.encode(b"NOPE");
        assert!(compile_package_source("material.dxbc", &bad).is_err());
    }
}
