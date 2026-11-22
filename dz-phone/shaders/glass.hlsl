// glass.hlsl - D3D11 port of dao zai's liquid-glass WGSL (gpu_glass.rs).
// Full port of dao zai's liquid-glass WGSL: rounded-rect SDF, notch SDF,
// 5-tap chromatic dispersion, Snell-style refraction (-g*k*strength),
// directional Schlick-like rim highlight, bevel dark band, light bleed,
// Beer-Lambert absorption, dither, plus per-control inner glass lenses.

#define MAX_CTL 13

cbuffer U : register(b0) {
    float4 bounds;          // xy = top-left (device px), zw = size (device px)
    float4 params;          // x=chroma, y=unused, z=morph, w=material(0..1)
    float4 extra;           // x=radius, y=texW, z=texH, w=dock(0=top,1=left,2=right)
    float4 tint;            // rgb tint, a=strength
    float4 quad;            // unused (fullscreen triangle)
    float4 misc;            // xy=pointer(px), z=refract strength, w=flat mode(0/1/2)
    float4 crect[MAX_CTL];  // (x, y, w, h) per control, capture-device px
    float4 cprm[MAX_CTL];   // (radius, enabled, magnify, refract) per control
    float4 look;            // x=blur, y=band, z=edge width, w=edge opacity
    float4 notch;           // x=flare, y=fh, z=isNotch, w=isDComp
    float4 lightcfg;        // x=follow strength
    float4 meta;            // 融球水滴: xy=中心(capture px, 顶原点), zw=半宽半高
    float4 meta2;           // x=smooth-min 半径 k, y=锚点 y(capture px), z=开关(0=关), w=保留
};

Texture2D<float4> bg : register(t0);
SamplerState samp : register(s0);

struct VSOut {
    float4 pos : SV_POSITION;
    float2 uv  : TEXCOORD0;
};

VSOut vs(uint id : SV_VertexID) {
    // Fullscreen triangle: no vertex buffer / input layout needed.
    float2 xy[3] = { float2(-1,-1), float2(3,-1), float2(-1,3) };
    float2 uv[3] = { float2(0,0),  float2(2,0),  float2(0,2)  };
    VSOut o;
    o.pos = float4(xy[id], 0.0, 1.0);
    o.uv  = uv[id];
    return o;
}

float rr_sdf(float2 p, float2 b, float r) {
    float2 q = abs(p) - b + r;
    return min(max(q.x, q.y), 0.0) + length(max(q, float2(0,0))) - r;
}

// 梯度感知 smooth-union (Apple QuartzCore sdf_union / IQ 多项式 smin)。
// a=vec3(d, gx, gy), b 同理。k 越大颈部越粗。返回一个新的 (d, g)。
float3 sdf_union3(float3 a, float3 b, float k) {
    float2 ga = normalize(a.yz + float2(1e-6, 1e-6));
    float2 gb = normalize(b.yz + float2(1e-6, 1e-6));
    float kEff = k * saturate(0.5 - 0.5 * dot(ga, gb)) + 1e-4;
    float h = saturate(0.5 + 0.5 * (b.x - a.x) / kEff);
    float d = lerp(b.x, a.x, h) - kEff * h * (1.0 - h);
    float2 g = normalize(lerp(b.yz, a.yz, h) + float2(1e-6, 1e-6));
    return float3(d, g);
}

// Notch (Liu Hai) outline SDF: shoulders flare outward near the top, matching
// shape::sections. Bottom corners rounded, top edge straight where shoulders extend.
// p 是 D3D11 局部坐标：+b.y 是岛体顶、-b.y 是底（bounds 已做 h-by-bh 翻转）。
// egui::sections 肩部 t=0 在顶、wing=flare*(1-t)^2.4 顶部最宽——t=0 也必须在顶。
float notch_sdf(float2 p, float2 b, float r, float flare, float fh) {
    float t = saturate((b.y - p.y) / max(fh, 1.0));
    float wing = flare * pow(1.0 - t, 2.4);
    float hx = b.x + wing;
    // 圆角在底部（-b.y 侧），不是顶部。迁移前 bounds 是顶原点、写 p.y > b.y-r；
    // 迁移后 bounds 做了 h-by-bh 翻转，底原点系里底部是 p.y=-b.y，故改这里。
    if (p.y < -(b.y - r)) {
        float qx = max(abs(p.x) - (b.x - r), 0.0);
        // qy = 底部圆角圆心(p.y=-(b.y-r)) 到当前点的距离，必须是减号；
        // 写成 abs(p.y)+(b.y-r) 会让底部整条 SDF 变正 → 判成岛体外 → 玻璃铺不满。
        float qy = -(p.y) - (b.y - r);
        return length(float2(qx, max(qy, 0.0))) - r;
    }
    return max(abs(p.x) - hx, abs(p.y) - b.y);
}

// 岛体 SDF：把屏幕局部坐标按停靠方向旋到「贴顶坐标系」再算形状。
// extra.w = dock（0=Top 不旋转；1=Left 肩部朝屏幕左；2=Right 肩部朝屏幕右）。
// 左右停靠时岛体 rect 是竖向窄条（36 x 长），half_size 需交换宽高，局部坐标也旋转 90°。
float island_sdf(float2 local, float2 b, float r) {
    if (extra.w < 0.5) {
        return (notch.z > 0.5)
            ? notch_sdf(local, b, r, notch.x, notch.y)
            : rr_sdf(local, b, r);
    }
    // 映射经完整链路数值验证（egui y 顶原点 → bounds 的 h-y 翻转 → shader local → q）：
    // Left  贴左边缘 local.x=-b.x → q=(local.y, -local.x) → q.y=+b.y → 肩部落在贴边侧 ✓
    // Right 贴右边缘 local.x=+b.x → q=(-local.y, local.x) → q.y=+b.y → 肩部落在贴边侧 ✓
    float2 q;
    float2 hb = b.yx;
    if (extra.w < 1.5) {
        q = float2(local.y, -local.x);   // Left
    } else {
        q = float2(-local.y, local.x);   // Right
    }
    return (notch.z > 0.5)
        ? notch_sdf(q, hb, r, notch.x, notch.y)
        : rr_sdf(q, hb, r);
}

float hash21(float2 p) {
    float2 q = frac(p * float2(0.1031, 0.1030));
    q += dot(q, q.yx + float2(33.33, 33.33));
    return frac((q.x + q.y) * q.x);
}

float3 sample_px(float2 px) {
    if (misc.w > 1.5) {
        // Anti-aliased grid (FLAT=2): makes refraction/dispersion/overflow visible.
        float2 q = px / 20.0;
        float dl = min(abs(frac(q.x) - 0.5), abs(frac(q.y) - 0.5)) * 20.0;
        float ln = 1.0 - smoothstep(0.6, 1.8, dl);
        return lerp(float3(0.80,0.84,0.90), float3(0.18,0.22,0.30), ln);
    }
    // px is window-local physical pixels, but y counts from the BOTTOM (D3D11 fullscreen
    // triangle: uv.y=0 at bottom). Flip it against window height (extra.z) so it counts
    // from the top, matching the desktop/window origin in quad.yz.
    // Without this flip the capture is mirrored vertically (island at top -> sample at bottom).
    uint w, h;
    bg.GetDimensions(w, h);
    float2 screen_px = float2(px.x + quad.y, (extra.z - px.y) + quad.z);
    float2 uv = saturate(screen_px / float2(w, h));
    // WGC frames are top-down; D3D11 SRV samples top-left origin -> flip Y.
    // quad.x is the flip flag (set when the live WGC path is active).
    if (quad.x > 0.5) uv.y = 1.0 - uv.y;
    // 采样 BGRA 背景纹理：D3D11 SRV 已把 B8G8R8A8 映射成正确的 RGBA 语义（.r=红），
    // 直接取 .rgb 即可。切勿照搬独立工程的 .bgr 交换——那套 pipeline 手动上传 BGRA
    // 字节才需要交换；WGC CopyResource 路径再交换会把 RGB 翻一次，暖色壁纸变青蓝。
    return bg.SampleLevel(samp, uv, 0.0).rgb;
}

// Per-channel 5-point cross average: R samples +ca side, B samples -ca side.
float3 tap5(float2 px, float2 ca, float blur) {
    float2 off[5] = {
        float2(0,0), float2(blur,0), float2(-blur,0), float2(0,blur), float2(0,-blur)
    };
    float3 s = 0;
    for (int i = 0; i < 5; i++) {
        float2 q = px + off[i];
        s += float3(sample_px(q + ca).r, sample_px(q).g, sample_px(q - ca).b);
    }
    return s / 5.0;
}

// Control hit test: later control overwrites earlier (volume overlay over lyrics).
// Returns idx = MAX_CTL when nothing hit.
struct CtlSel { float d; float2 hs; float r; int idx; float mag; float refr; };
CtlSel pick_ctl(float2 p) {
    CtlSel outc = { 0.0, float2(1,1), 0.0, MAX_CTL, 1.0, 1.0 };
    for (int ci = 0; ci < MAX_CTL; ci++) {
        if (cprm[ci].y < 0.5) continue;
        float4 cr = crect[ci];
        float2 hs = max(cr.zw * 0.5 - float2(0.5,0.5), float2(1,1));
        float rr = max(min(cprm[ci].x - 0.5, min(hs.x, hs.y)), 0.0);
        float dd = rr_sdf(p - cr.xy - cr.zw * 0.5, hs, rr);
        if (dd < 0.0) {
            outc.d = dd; outc.hs = hs; outc.r = rr;
            outc.idx = ci; outc.mag = cprm[ci].z; outc.refr = cprm[ci].w;
        }
    }
    return outc;
}

// Small lens drawn for one control (same material as island body, tighter band).
float4 ctrl_glass(float2 p, float2 local, float2 hs, float radius, float d,
                  float mat, float2 Ld, float3 edge_bg, float mag, float refr) {
    float e = 1.0;
    float gx = rr_sdf(local + float2(e,0), hs, radius) - rr_sdf(local - float2(e,0), hs, radius);
    float gy = rr_sdf(local + float2(0,e), hs, radius) - rr_sdf(local - float2(0,e), hs, radius);
    float2 g = normalize(float2(gx,gy) + float2(1e-6,1e-6));

    float short_side = max(min(hs.x, hs.y), 1.0);
    float band = clamp(short_side * 1.30, 10.0, 72.0);
    float t = saturate(-d / band);
    float k = pow(1.0 - pow(t, 0.30), 2.0);
    float strength = max(misc.z, 0.0) * 0.55 * max(refr, 0.05);
    float cxn = saturate((abs(local.x) - (hs.x - radius)) / max(radius,1));
    float cyn = saturate((abs(local.y) - (hs.y - radius)) / max(radius,1));
    float chroma = max(params.x,0.0) * 0.55 * max(refr,0.05) * lerp(0.60,1.0,cxn*cyn);
    float m = max(mag, 1.0);
    float2 q = p - local * (1.0 - 1.0 / m);
    float2 bp = q - g * k * strength;

    float3 col = tap5(bp, -g * k * chroma, look.x * 0.80 + k * 1.2);
    // Material compensation identical to the island body.
    float lum_c = dot(col, float3(0.299,0.587,0.114));
    col = col * (1.0 + lerp(0.045,-0.035,smoothstep(0.42,0.88,lum_c)));
    float bg_l = dot(edge_bg, float3(0.299,0.587,0.114));
    float scrim = smoothstep(0.48,0.96,bg_l) * 0.16 * mat;
    col = lerp(col, float3(0.30,0.30,0.30), scrim);
    col = col * 1.010 + float3(0.004,0.006,0.010);
    float matdark = (1.0 - mat) * 0.70;
    col = lerp(col, col * 0.58 + float3(0.010,0.014,0.020), float3(matdark,matdark,matdark));
    float topk = 1.0 - smoothstep(0.0, max(hs.y * 2.0, 1.0), local.y + hs.y);
    col += float3(0.06,0.08,0.11) * topk * 0.16;
    float tq = (d + 1.7) / 0.80;
    float bevel = exp(-tq*tq);
    col *= (1.0 - bevel * 0.10);
    float t_in = max(-d, 0.0);
    float bleed = exp(-t_in / clamp(short_side*0.45,5.0,22.0)) * (1.0 - bevel*0.5);
    float lobe = pow(max(dot(g,Ld),0.0),1.5);
    col += float3(0.62,0.86,0.96) * bleed * (lobe*0.15 + 0.055);
    float sigma_c = max(look.z, 0.5) * 0.42;
    float lq = (d + sigma_c*0.65) / sigma_c;
    float rim = exp(-lq*lq);
    float dirw = lerp(0.55,1.0,pow(max(dot(g,Ld),0.0),1.3));
    float line_v = rim * dirw;
    float edge_a = saturate(look.w);
    col = lerp(col, float3(1,1,1), saturate(line_v*edge_a));
    float aa = 1.0 - smoothstep(-1.0,1.0,d);
    return float4(col, saturate(max(aa, line_v*edge_a)));
}

// 液态玻璃「融球水滴」：锚点扁带 + 水滴圆角矩形 smooth-union，带真折射/色散/白边。
// meta.xy=水滴中心(device px,底原点), zw=半宽半高; meta2.x=k, y=锚点y(底原点), z=开关。
float4 metaball_glass(float2 p, float mat, float2 Ld, float3 edge_bg) {
    float e = 1.0;
    float2 dropB = max(meta.zw, float2(1.0,1.0));
    float dropR = min(dropB.x, dropB.y);
    float2 dl = p - meta.xy;
    float dgx = rr_sdf(dl + float2(e,0), dropB, dropR) - rr_sdf(dl - float2(e,0), dropB, dropR);
    float dgy = rr_sdf(dl + float2(0,e), dropB, dropR) - rr_sdf(dl - float2(0,e), dropB, dropR);
    // 融球本体 = 水滴圆角矩形。不再叠一条可见的锚点扁带——之前那条在岛仔里
    // 变成面板顶部突兀的横线，且和水滴离太远长不出颈部。拉丝感交给 app 侧的
    // 速度拉伸（bx/by 随速度放大）承担。
    float d = rr_sdf(dl, dropB, dropR);
    float2 g = normalize(float2(dgx, dgy) + float2(1e-6,1e-6));

    // 折射/色散/白边：用融球自己的 SDF 梯度 g，按短边自适应带深。
    float short_side = max(min(dropB.x, dropB.y), 2.0);
    float band = clamp(short_side * 1.3, 8.0, 60.0);
    float t = saturate(-d / band);
    float kk = pow(1.0 - pow(t, 0.30), 2.0);
    // 融球折射比岛体更强，让它从底下那层玻璃里"凸"出来。
    float strength = max(misc.z, 0.0) * 0.90;
    float chroma = max(params.x, 0.0) * 0.85;
    float2 bp = p - g * kk * strength;
    float3 col = tap5(bp, -g * kk * chroma, look.x * 0.8 + kk * 1.2);
    float lum_c = dot(col, float3(0.299,0.587,0.114));
    col *= (1.0 + lerp(0.045,-0.035,smoothstep(0.42,0.88,lum_c)));
    float bg_l = dot(edge_bg, float3(0.299,0.587,0.114));
    col = lerp(col, float3(0.30,0.30,0.30), smoothstep(0.48,0.96,bg_l)*0.16*mat);
    // 材质压暗加深：融球是一颗"实体玻璃珠"，比底下的岛体玻璃更实。
    float matdark = (1.0-mat)*0.70 + 0.12;
    col = lerp(col, col*0.52 + float3(0.010,0.014,0.020), float3(matdark,matdark,matdark));
    // 内侧暗边(bevel)：沿边缘内 2~5px 压一道暗环，制造厚度与体积感，
    // 这是让融球"浮"在岛体玻璃之上、肉眼可辨的关键。
    float bevel = exp(-pow((d + 3.0)/2.5, 2.0));
    col *= (1.0 - bevel * 0.30);
    // 白边加强：比岛体 rim 更亮更宽一点。
    float sigma_c = max(look.z, 0.5) * 0.52;
    float rim = exp(-pow((d + sigma_c*0.6)/sigma_c, 2.0));
    float dirw = lerp(0.62,1.0,pow(max(dot(g,Ld),0.0),1.3));
    float line_v = rim * dirw;
    float edge_a = saturate(look.w) * 1.15;
    col = lerp(col, float3(1,1,1), saturate(line_v*edge_a));
    float aa = 1.0 - smoothstep(-1.2, 1.2, d);
    return float4(col, saturate(max(aa, line_v*edge_a)));
}

float4 fs(VSOut i) : SV_TARGET {
    float2 tex = float2(extra.y, extra.z);
    float2 p = i.uv * tex;

    float inset = (notch.w > 0.5) ? 0.0 : 3.0;
    float2 half_size = max(bounds.zw * 0.5 - inset, float2(2,2));
    float2 local = p - bounds.xy - bounds.zw * 0.5;
    float radius = max(min(extra.x - inset, min(half_size.x, half_size.y)), 0.0);
    float d = island_sdf(local, half_size, radius);
    float mat = saturate(params.w);

    // Outside the island: paint the desktop back (or stay transparent under DComp).
    // 液态玻璃的投影就画在这里——D3D11 是「先玻璃、后 egui」，egui 侧阴影会盖住
    // 折射玻璃（迁移前 wgpu 顺序相反才没事）。shader 用岛体 SDF 的高斯衰减画外阴影，
    // 形状/位置/方向自动跟随刘海或胶囊、任意停靠方向都对，且只画在 d>0 的外围。
    // 资料包 8 件套同款：shadow = 0.5*exp(-sd²/2σ²)，再用 SDF 把内部裁掉。
    float thresh = (notch.w > 0.5) ? 1.0 : (inset - 0.25);
    if (d > thresh) {
        if (notch.w > 0.5) {
            // DComp 逐像素 alpha：输出预乘的黑色阴影，越靠近岛体越浓、向外衰减到透明。
            float sigma = 5.0;
            float shadow = exp(-pow(max(d - 1.0, 0.0), 2.0) / (2.0 * sigma * sigma));
            shadow *= lightcfg.z;   // 岛体投影开关（0=关，1=开）
            return float4(0, 0, 0, saturate(shadow) * 0.28);
        }
        float3 bgcol = sample_px(p);
        float sh = (1.0 - smoothstep(0.0, 7.0, d - 1.0)) * lerp(0.06, 0.02, mat);
        return float4(bgcol * (1.0 - sh) + float3(0.05,0.06,0.07) * sh, 1.0);
    }

    float aa_w = (notch.w > 0.5) ? max(fwidth(d) * 1.1, 1.4) : 1.2;
    float aa = 1.0 - smoothstep(-aa_w, aa_w, d);

    // Virtual light source follows pointer horizontally; light always from above.
    float2 center = bounds.xy + bounds.zw * 0.5;
    float2 to_ptr = misc.xy - center;
    float plen = length(to_ptr);
    float2 swung = to_ptr / max(plen, 1e-3);
    float2 up_bias = float2(swung.x, min(swung.y, -0.18));
    float follow = saturate(lightcfg.x);
    float2 Ld = normalize(lerp(normalize(float2(-0.45,-0.65)), up_bias,
                               smoothstep(3.0, 48.0, plen) * follow)
                        + float2(1e-6, 1e-6));

    float3 edge_bg = sample_px(p);

    // FLAT diagnostic: show raw captured background (grid seam = alignment check).
    if (misc.w > 0.5) {
        float3 flat_col = sample_px(p);
        CtlSel cs = pick_ctl(p);
        if (cs.idx < MAX_CTL) {
            float4 cr = crect[cs.idx];
            float4 ce = ctrl_glass(p, p - cr.xy - cr.zw*0.5, cs.hs, cs.r, cs.d,
                                   mat, Ld, edge_bg, cs.mag, cs.refr);
            flat_col = lerp(flat_col, ce.rgb, ce.a);
        }
        float a = (notch.w > 0.5) ? aa : 1.0;
        return float4(flat_col * a, a);
    }

    // SDF gradient points outward. 用 island_sdf 保证左右停靠时法线跟着形状一起旋转。
    float e = 1.0;
    float gx = island_sdf(local + float2(e,0), half_size, radius)
             - island_sdf(local - float2(e,0), half_size, radius);
    float gy = island_sdf(local + float2(0,e), half_size, radius)
             - island_sdf(local - float2(0,e), half_size, radius);
    float2 g = normalize(float2(gx, gy) + float2(1e-6, 1e-6));

    // Refraction band: strongest at the edge, decays inward.
    // 角部判定用旋转后的局部坐标，左右停靠（竖向岛体）才能正确识别四个圆角。
    float2 edge_local = local;
    float2 edge_half = half_size;
    if (extra.w > 0.5) {
        edge_local = (extra.w < 1.5) ? float2(local.y, -local.x) : float2(-local.y, local.x);
        edge_half = half_size.yx;
    }
    float band = max(look.y, 1.0);
    float t = saturate(-d / band);
    float k = pow(1.0 - pow(t, 0.30), 2.0);
    float cxn = saturate((abs(edge_local.x) - (edge_half.x - radius)) / max(radius,1));
    float cyn = saturate((abs(edge_local.y) - (edge_half.y - radius)) / max(radius,1));
    float chroma = max(params.x, 0.0) * lerp(0.60, 1.0, cxn * cyn);
    float strength = max(misc.z, 0.0);
    float2 refr = -g * k * strength;
    float2 ca = -g * k * chroma;
    float2 bp = p + refr;

    float3 col = tap5(bp, ca, max(look.x, 0.0) + k * 1.6);
    col = col * 1.004 + float3(0.002, 0.003, 0.006);

    // Top cold light -> glass thickness.
    float topk = 1.0 - smoothstep(0.0, max(bounds.w, 1.0), p.y - bounds.y);
    col += float3(0.05, 0.07, 0.10) * topk * 0.10;

    // Inverse luminance compensation (keeps contrast on bright wallpapers).
    float lum_m = dot(col, float3(0.299, 0.587, 0.114));
    float lift = lerp(0.045, -0.035, smoothstep(0.42, 0.88, lum_m));
    col *= (1.0 + lift);

    // Material: dark variant must stay visibly below background.
    float matdark = (1.0 - mat) * 0.70;
    col = lerp(col, col * 0.58 + float3(0.010,0.014,0.020), float3(matdark,matdark,matdark));
    float matfog = mat * 0.02;
    col = lerp(col, float3(0.96,0.97,1.0), float3(matfog,matfog,matfog));

    // Per-pixel scrim for text legibility (only on light material).
    float bg_l = dot(edge_bg, float3(0.299, 0.587, 0.114));
    float scrim = smoothstep(0.48, 0.96, bg_l) * 0.16 * mat;
    col = lerp(col, float3(0.30,0.30,0.30), scrim);

    // Restore a little saturation washed out by the scrim.
    float lum0 = dot(col, float3(0.299, 0.587, 0.114));
    col = lerp(float3(lum0,lum0,lum0), col, 1.06);

    // Bevel dark band.
    float tq = (d + 2.2) / 0.9;
    float bevel = exp(-tq * tq);
    col *= (1.0 - bevel * 0.08);

    // Internal light bleed (suppressed by bevel) + Beer-Lambert edge absorption.
    float t_in = max(-d, 0.0);
    float bleed = exp(-t_in / 16.0) * (1.0 - bevel * 0.55);
    float path = exp(-t_in / 7.0);
    col *= lerp(float3(1,1,1), float3(0.90,1.0,0.96), path * 0.65);
    float absorb = 1.0 - exp(-t_in / 10.0);
    float3 lcol = lerp(float3(0.88,0.93,1.0), float3(0.52,0.90,0.80), absorb);
    float lobe_a = pow(max(dot(g, Ld), 0.0), 1.6);
    float lobe_b = pow(max(dot(g, -Ld), 0.0), 1.2);
    col += lcol * bleed * (lobe_a * 0.16 + lobe_b * 0.085 + 0.050);
    col += tint.rgb * tint.a * 0.05 * bleed;

    // Sharp rim highlight, directionally modulated (the "liquid" feel).
    float sigma = max(look.z, 0.3) * 0.365;
    float lq = (d + sigma * 0.55) / sigma;
    float rim = exp(-lq * lq);
    float gq = (d + 1.0) / max(3.2 * look.z / 1.5, 0.8);
    float glow = exp(-gq * gq) * 0.04;
    float dirw = lerp(0.40, 1.0, pow(max(dot(g, Ld), 0.0), 1.5));
    float line_v = rim * dirw;
    float edge_a = saturate(look.w);
    float line_w = edge_a * lerp(0.69, 1.02, smoothstep(0.60, 0.92, lum_m));
    col = lerp(col, float3(1,1,1), saturate(line_v * line_w + glow));

    // Per-control inner glass lenses (same capture, multiple small lenses).
    CtlSel cs = pick_ctl(p);
    if (cs.idx < MAX_CTL) {
        float4 cr = crect[cs.idx];
        float4 ce = ctrl_glass(p, p - cr.xy - cr.zw*0.5, cs.hs, cs.r, cs.d,
                               mat, Ld, edge_bg, cs.mag, cs.refr);
        col = lerp(col, ce.rgb, ce.a);
    }

    // Blend edge with the (unrefracted) desktop so no black halo; dither.
    float cov = saturate(max(aa, line_v * edge_a));

    // 液态玻璃融球水滴：作为独立玻璃层浮在岛体之上（不合并进岛体 SDF）。
    if (meta2.z > 0.5) {
        float4 mb = metaball_glass(p, mat, Ld, edge_bg);
        col = lerp(col, mb.rgb, mb.a);
        cov = saturate(max(cov, mb.a));
    }

    float dth = (hash21(p) - 0.5) * (1.7 / 255.0);
    float3 outc = lerp(edge_bg, col, float3(cov,cov,cov)) + float3(dth,dth,dth);
    if (notch.w > 0.5) return float4(outc * cov, cov);
    return float4(outc, 1.0);
}
