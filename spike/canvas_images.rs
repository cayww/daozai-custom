use super::{
    appearance_assets,
    cover::{open_plain_file, COVER_LIMIT},
    fail, Result, COVER_FILE,
};
use crate::plugin_canvas::{valid_image_path, Fit};
use base64::{engine::general_purpose::STANDARD, Engine};
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    io::{Cursor, Read},
    path::Path,
    sync::{Arc, Mutex, OnceLock, Weak},
};

pub(super) const IMAGE_LIMIT: u64 = 512 * 1024;
const SIDE_LIMIT: u32 = 1024;
const HISTORY_BUDGET: usize = 32 * 1024 * 1024;
const ACTIVE_BUDGET: usize = 64 * 1024 * 1024;
const ACTIVE_RESERVE: usize = 8 * 1024 * 1024;
const TEXTURE_LIMIT: usize = 128;
const PIXEL_BYTES: usize = std::mem::size_of::<egui::Color32>();
pub(super) type CanvasImages = BTreeMap<String, CanvasImage>;

#[derive(Clone)]
pub(super) struct CanvasImage {
    bytes: Arc<[u8]>,
    size: Arc<OnceLock<[usize; 2]>>,
}

struct CachedTexture {
    image: Weak<OnceLock<[usize; 2]>>,
    raster_size: [usize; 2],
    used_pass: u64,
    texture: egui::TextureHandle,
}

#[derive(Default)]
struct TextureCache {
    entries: VecDeque<CachedTexture>,
    bytes: usize,
    pass: u64,
    active_bytes: usize,
}

impl TextureCache {
    fn for_context(ctx: &egui::Context) -> Arc<Mutex<Self>> {
        let mut install_cleanup = false;
        let cache = ctx.data_mut(|data| {
            Arc::clone(data.get_temp_mut_or_insert_with(
                egui::Id::new("plugin-canvas-textures"),
                || {
                    install_cleanup = true;
                    Arc::<Mutex<Self>>::default()
                },
            ))
        });
        if install_cleanup {
            let cache = Arc::downgrade(&cache);
            ctx.on_end_pass(
                "plugin-canvas-textures",
                Arc::new(move |ctx| {
                    if ctx.viewport_id() == egui::ViewportId::ROOT {
                        if let Some(cache) = cache.upgrade() {
                            let pass = ctx.cumulative_pass_nr();
                            cache
                                .lock()
                                .unwrap_or_else(|poison| poison.into_inner())
                                .finish_pass(pass);
                        }
                    }
                }),
            );
        }
        cache
    }

    fn trim_to(&mut self, bytes: usize, entries: usize, pass: u64) {
        while self.bytes > bytes || self.entries.len() > entries {
            let Some(oldest) = self.entries.back() else {
                break;
            };
            if oldest.used_pass == pass {
                break;
            }
            self.bytes -= self.entries.pop_back().unwrap().texture.byte_size();
        }
    }

    fn find(&mut self, image: &Arc<OnceLock<[usize; 2]>>, size: [usize; 2]) -> Option<egui::TextureId> {
        let index = self.entries.iter().position(|entry| {
            entry.image.as_ptr() == Arc::as_ptr(image)
                && if entry.used_pass == self.pass {
                    entry.raster_size[0] >= size[0] && entry.raster_size[1] >= size[1]
                } else {
                    entry.raster_size == size && entry.texture.byte_size() <= ACTIVE_BUDGET - self.active_bytes
                }
        })?;
        let mut entry = self.entries.remove(index).unwrap();
        if entry.used_pass != self.pass {
            self.active_bytes += entry.texture.byte_size();
            entry.used_pass = self.pass;
        }
        let id = entry.texture.id();
        self.entries.push_front(entry);
        Some(id)
    }

    fn finish_pass(&mut self, pass: u64) {
        self.entries.retain(|entry| {
            let keep = entry.used_pass == pass || entry.image.strong_count() != 0;
            if !keep {
                self.bytes -= entry.texture.byte_size();
            }
            keep
        });
        // 本轮仍绘制的图片可以超过历史预算，避免每帧重新解码和上传。
        self.trim_to(HISTORY_BUDGET, TEXTURE_LIMIT, pass);
    }
}

impl CanvasImage {
    pub(super) fn checked(name: &str, bytes: Vec<u8>) -> Result<Self> {
        let image = Self {
            bytes: bytes.into(),
            size: Arc::new(OnceLock::new()),
        };
        image.decode(name)?;
        Ok(image)
    }

    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn decode(&self, name: &str) -> Result<egui::ColorImage> {
        let image = if Path::new(name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
        {
            appearance_assets::load_static_svg(
                std::str::from_utf8(&self.bytes)?,
                IMAGE_LIMIT as usize,
                SIDE_LIMIT,
            )?
        } else {
            let mut reader =
                image::ImageReader::with_format(Cursor::new(&self.bytes), image::ImageFormat::Png);
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(SIDE_LIMIT);
            limits.max_image_height = Some(SIDE_LIMIT);
            limits.max_alloc = Some(16 * 1024 * 1024);
            reader.limits(limits);
            let rgba = reader.decode()?.into_rgba8();
            egui::ColorImage::from_rgba_unmultiplied(
                [rgba.width() as usize, rgba.height() as usize],
                rgba.as_raw(),
            )
        };
        let _ = self.size.set(image.size());
        Ok(image)
    }

    fn rasterize(&self, name: &str, source: [usize; 2], target: [usize; 2]) -> Result<egui::ColorImage> {
        if source == target {
            return self.decode(name);
        }
        if Path::new(name).extension().is_some_and(|ext| ext.eq_ignore_ascii_case("svg")) {
            let hint = egui::load::SizeHint::Size { width: target[0] as u32, height: target[1] as u32, maintain_aspect_ratio: true };
            return egui_extras::image::load_svg_bytes_with_size(&self.bytes, hint, &Default::default())
                .map_err(|error| fail(&error));
        }
        let image = self.decode(name)?;
        let rgba = image::Rgba32FImage::from_fn(source[0] as u32, source[1] as u32, |x, y| {
            let base = y as usize * source[0] * 4 + x as usize * 4;
            let raw = image.as_raw();
            let color = egui::Color32::from_rgba_premultiplied(
                raw[base], raw[base + 1], raw[base + 2], raw[base + 3],
            );
            image::Rgba(egui::Rgba::from(color).to_array())
        });
        let smaller = image::imageops::resize(&rgba, target[0] as u32, target[1] as u32,
            image::imageops::FilterType::Triangle);
        let mut pixels = Vec::with_capacity(target[0] * target[1] * 4);
        for p in smaller.pixels() {
            let [r, g, b, a] = p.0;
            pixels.extend_from_slice(&[
                (r * 255.0).round() as u8, (g * 255.0).round() as u8,
                (b * 255.0).round() as u8, (a * 255.0).round() as u8,
            ]);
        }
        Ok(egui::ColorImage::from_rgba_premultiplied(target, &pixels))
    }

    pub(super) fn texture(
        &self,
        ctx: &egui::Context,
        name: &str,
        display_pixels: egui::Vec2,
        fit: Fit,
    ) -> Option<(egui::TextureId, egui::Vec2)> {
        let source = *self.size.get()?;
        let native = egui::vec2(source[0] as f32, source[1] as f32);
        let ratio = display_pixels / native;
        let scale = match fit {
            Fit::Contain => ratio.min_elem(),
            Fit::Cover => ratio.max_elem(),
        };
        let longest = source[0].max(source[1]);
        // 把相近的显示尺寸归为一档，避免动画与 DPI 取整反复上传。
        let edge = ((longest as f32 * scale).ceil() as usize).clamp(1, longest)
            .next_power_of_two().min(longest);
        let size_for = |edge: usize| source.map(|side| (side * edge).div_ceil(longest));
        let mut raster_size = size_for(edge);
        // 岛上画布共用根视口。嵌套视口结束绘制时，也不能回收根视口本轮已用的纹理。
        let pass = ctx.cumulative_pass_nr_for(egui::ViewportId::ROOT);
        let cache = TextureCache::for_context(ctx);
        let mut cache = cache.lock().unwrap_or_else(|poison| poison.into_inner());
        if cache.pass != pass {
            cache.pass = pass;
            cache.active_bytes = 0;
        }
        let available = ACTIVE_BUDGET - cache.active_bytes;
        let raster_bytes = |[width, height]: [usize; 2]| width * height * PIXEL_BYTES;
        let bytes = raster_bytes(raster_size);
        if bytes > available.saturating_sub(ACTIVE_RESERVE) {
            // 极密画布继续绘制。为同一帧后续纹理保留空间，按剩余预算缩小图片。
            let pixels = (available / (4096 * PIXEL_BYTES)).clamp(1, 512);
            let mut edge = edge;
            while raster_bytes(raster_size) > available.min(pixels * PIXEL_BYTES) && edge > 1 {
                edge /= 2;
                raster_size = size_for(edge);
            }
        }
        if let Some(id) = cache.find(&self.size, raster_size) {
            return Some((id, native));
        }
        if raster_bytes(raster_size) > available {
            return None;
        }

        let image = self.rasterize(name, source, raster_size).ok()?;
        let bytes = raster_bytes(image.size());
        if bytes > available {
            return None;
        }
        cache.trim_to(ACTIVE_BUDGET - bytes, usize::MAX, pass);
        let texture = ctx.load_texture(name, image, egui::TextureOptions::LINEAR);
        let id = texture.id();
        cache.bytes += bytes;
        cache.active_bytes += bytes;
        cache.entries.push_front(CachedTexture {
            image: Arc::downgrade(&self.size),
            raster_size,
            used_pass: pass,
            texture,
        });
        Some((id, native))
    }
}

impl Serialize for CanvasImage {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&STANDARD.encode(&self.bytes))
    }
}

impl<'de> Deserialize<'de> for CanvasImage {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        use serde::de::Error;
        let encoded = String::deserialize(deserializer)?;
        if encoded.len() as u64 > IMAGE_LIMIT.div_ceil(3) * 4 {
            return Err(Error::custom("插件画布图片超过 512 KiB"));
        }
        let bytes = STANDARD.decode(encoded).map_err(Error::custom)?;
        Ok(Self {
            bytes: bytes.into(),
            size: Arc::new(OnceLock::new()),
        })
    }
}

fn is_image(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("png") || ext.eq_ignore_ascii_case("svg"))
}

pub(super) fn names(base: &Path) -> Result<Vec<String>> {
    let mut names = Vec::new();
    let mut folders = vec![base.to_owned()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(folder)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() && path.canonicalize()? == path {
                folders.push(path);
            } else if is_image(&path) && path.is_file() {
                let name = path
                    .strip_prefix(base)?
                    .to_string_lossy()
                    .replace('\\', "/");
                if !valid_image_path(&name) {
                    return Err(fail(&format!("插件画布图片路径无效：{name}")));
                }
                names.push(name);
            }
        }
    }
    Ok(names)
}

pub(super) fn read(base: &Path) -> Result<CanvasImages> {
    decode_all(names(base)?, |name, remaining| {
        let mut bytes = Vec::new();
        open_plain_file(&base.join(name), "插件画布图片必须是包内普通文件")?
            .take(remaining + 1)
            .read_to_end(&mut bytes)?;
        Ok(bytes)
    })
}

pub(super) fn decode_all(
    mut names: Vec<String>,
    mut load: impl FnMut(&str, u64) -> Result<Vec<u8>>,
) -> Result<CanvasImages> {
    let mut remaining = IMAGE_LIMIT;
    let mut images = BTreeMap::new();
    // 封面是集市的独立资源，先给专用画布图片留出额度。
    names.sort_by_key(|name| name == COVER_FILE);
    for name in names {
        if !valid_image_path(&name) {
            return Err(fail(&format!("插件画布图片路径无效：{name}")));
        }
        let limit = if name == COVER_FILE {
            COVER_LIMIT
        } else {
            remaining
        };
        let bytes = load(&name, limit)?;
        if bytes.len() as u64 > limit {
            return Err(fail("插件画布图片合计超过 512 KiB"));
        }
        if name == COVER_FILE && bytes.len() as u64 > remaining {
            continue;
        }
        let image = match CanvasImage::checked(&name, bytes) {
            Ok(image) => image,
            // 合规的集市封面可能超过画布图元的尺寸。
            Err(_) if name == COVER_FILE => continue,
            Err(error) => return Err(error),
        };
        remaining -= image.bytes().len() as u64;
        images.insert(name, image);
    }
    Ok(images)
}

pub(super) fn valid_snapshot(images: &CanvasImages) -> bool {
    images
        .values()
        .map(|image| image.bytes.len() as u64)
        .sum::<u64>()
        <= IMAGE_LIMIT
        && images
            .iter()
            .all(|(name, image)| valid_image_path(name) && image.decode(name).is_ok())
}

#[cfg(all(test, feature="__spike_tests_off__"))]
mod tests {
    use super::*;

    fn solid_svg(side: u32, color: &str) -> Vec<u8> {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {side} {side}" width="{side}" height="{side}"><rect width="{side}" height="{side}" fill="{color}"/></svg>"#
        )
        .into_bytes()
    }

    #[test]
    fn canvas_texture_budget_keeps_used_images_alive_until_the_next_pass() {
        let ctx = egui::Context::default();
        let names: Vec<_> = (0..17).map(|i| format!("image-{i}.svg")).collect();
        let images = decode_all(names.clone(), |_, _| Ok(solid_svg(1024, "#e03030"))).unwrap();
        let mut ids = Vec::new();
        let full = egui::Vec2::splat(1024.0);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            for name in &names {
                ids.push(images[name].texture(ctx, name, full, Fit::Contain).unwrap().0);
            }
        });
        let [first, second] = [ids[0], ids[1]];
        let tex_manager = ctx.tex_manager();
        let textures = tex_manager.read();
        let used: usize = ids.iter().map(|id| textures.meta(*id).unwrap().bytes_used()).sum();
        assert!(used <= ACTIVE_BUDGET);
        assert!(textures.meta(ids[16]).unwrap().size[0] < 1024);
        drop(textures);

        let out = ctx.run(egui::RawInput::default(), |ctx| {
            assert_eq!(images[&names[0]].texture(ctx, &names[0], full, Fit::Contain).unwrap().0, first);
            assert!(images[&names[16]].texture(ctx, &names[16], full, Fit::Contain).is_some());
        });
        assert!(
            out.textures_delta.free.contains(&second),
            "换轮播图时释放最久没用的纹理"
        );
        assert!(!out.textures_delta.free.contains(&first));
    }

    #[test]
    fn canvas_texture_cache_reuses_active_images_and_trims_when_hidden() {
        let ctx = egui::Context::default();
        let names: Vec<_> = (0..9).map(|i| format!("image-{i}.svg")).collect();
        let images = decode_all(names.clone(), |_, _| Ok(solid_svg(1024, "#e03030"))).unwrap();
        let mut drawn = Vec::new();
        for _ in 0..2 {
            let mut frame = Vec::new();
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                frame = names
                    .iter()
                    .map(|name| images[name].texture(ctx, name, egui::Vec2::splat(1024.0), Fit::Contain).unwrap().0)
                    .collect();
            });
            if drawn.is_empty() {
                drawn = frame;
            } else {
                assert_eq!(frame, drawn, "超过历史预算的可见图片仍应复用");
            }
        }
        let hidden = ctx.run(egui::RawInput::default(), |_| {});
        assert!(hidden.textures_delta.free.contains(&drawn[0]));
        assert_eq!(
            drawn
                .iter()
                .filter(|id| ctx.tex_manager().read().meta(**id).is_some())
                .count(),
            8
        );
    }

    #[test]
    fn canvas_texture_cache_uses_the_current_context_and_image_revision() {
        let draw = |ctx: &egui::Context, image: &CanvasImage| {
            let mut id = None;
            let out = ctx.run(egui::RawInput::default(), |ctx| {
                id = image.texture(ctx, "same.svg", egui::Vec2::splat(2.0), Fit::Contain).map(|texture| texture.0);
            });
            (id.unwrap(), out)
        };
        let first_ctx = egui::Context::default();
        let second_ctx = egui::Context::default();
        let original = CanvasImage::checked("same.svg", solid_svg(2, "#ff0000")).unwrap();
        let replacement = CanvasImage::checked("same.svg", solid_svg(2, "#0000ff")).unwrap();
        let (first_id, _) = draw(&first_ctx, &original);
        let (second_id, second_out) = draw(&second_ctx, &original);
        assert!(second_out
            .textures_delta
            .set
            .iter()
            .any(|(id, _)| *id == second_id));
        assert_eq!(draw(&first_ctx, &original.clone()).0, first_id);
        let (new_id, replaced) = draw(&first_ctx, &replacement);
        assert_ne!(new_id, first_id);
        let img = &replaced
            .textures_delta
            .set
            .iter()
            .find(|(id, _)| *id == new_id)
            .unwrap()
            .1
            .image;
        let egui::ImageData::Color(img) = img else {
            panic!("应上传新资源")
        };
        assert_eq!(img.pixels[0], egui::Color32::BLUE);
        drop(original);
        let released = first_ctx.run(egui::RawInput::default(), |_| {});
        assert!(released.textures_delta.free.contains(&first_id));
        assert!(!released.textures_delta.free.contains(&new_id));
    }

    #[test]
    fn canvas_texture_uses_draw_size_and_keeps_both_sizes_in_one_pass() {
        let ctx = egui::Context::default();
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 513"><rect width="1024" height="513" fill="#e03030"/></svg>"##;
        let image = CanvasImage::checked("banner.svg", svg.as_bytes().to_vec()).unwrap();
        let mut ids = Vec::new();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            for fit in [Fit::Contain, Fit::Cover, Fit::Contain] {
                let (id, native) = image.texture(ctx, "banner.svg", egui::Vec2::splat(24.0), fit).unwrap();
                assert_eq!(native, egui::vec2(1024.0, 513.0));
                ids.push(id);
            }
        });
        assert_eq!(ids[1], ids[2]);
        assert_ne!(ids[0], ids[1]);
        for (&id, size) in ids.iter().zip([[32, 17], [64, 33], [64, 33]]) {
            let (_, delta) = out.textures_delta.set.iter().find(|(texture, _)| *texture == id).unwrap();
            assert_eq!(delta.image.size(), size);
            assert!(!out.textures_delta.free.contains(&id));
        }
        let reused = ctx.run(egui::RawInput::default(), |ctx| {
            let (id, _) = image.texture(ctx, "banner.svg", egui::Vec2::splat(24.0), Fit::Cover).unwrap();
            assert_eq!(id, ids[1]);
        });
        assert!(!reused.textures_delta.set.iter().any(|(id, _)| ids.contains(id)));
    }

    #[test]
    fn canvas_png_downsampling_preserves_transparent_edges() {
        let png = image::RgbaImage::from_fn(4, 4, |x, _| {
            if x < 2 { image::Rgba([255, 0, 0, 255]) } else { image::Rgba([0, 255, 0, 0]) }
        });
        let mut bytes = Cursor::new(Vec::new());
        png.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        let image = CanvasImage::checked("edge.png", bytes.into_inner()).unwrap();
        let raster = image.rasterize("edge.png", [4, 4], [1, 1]).unwrap();
        let [r, g, b, a] = raster.pixels[0].to_array();
        assert_eq!([g, b], [0, 0]);
        assert!((120..=136).contains(&a));
        assert!(r > a);
    }

    #[test]
    fn canvas_images_check_dimensions_and_total_bytes() {
        let png = |width| {
            let mut output = Cursor::new(Vec::new());
            image::DynamicImage::new_rgba8(width, 1)
                .write_to(&mut output, image::ImageFormat::Png)
                .unwrap();
            output.into_inner()
        };
        assert!(CanvasImage::checked("image.png", png(1024)).is_ok());
        assert!(CanvasImage::checked("image.png", png(1025)).is_err());

        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="#000"/></svg>"##;
        let mut bytes = svg.to_vec();
        bytes.resize(IMAGE_LIMIT as usize / 2 + 1, b' ');
        assert!(decode_all(vec!["first.svg".into()], |_, _| Ok(bytes.clone())).is_ok());
        assert!(
            decode_all(vec!["first.svg".into(), "second.svg".into()], |_, _| Ok(
                bytes.clone()
            ))
            .is_err()
        );
        assert!(decode_all(vec!["../other.svg".into()], |_, _| panic!(
            "越界路径不得交给读取器"
        ))
        .is_err());
    }
}
