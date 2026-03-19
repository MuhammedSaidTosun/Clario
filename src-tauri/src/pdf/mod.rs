use pdfium_render::prelude::*;
use png::{BitDepth, ColorType};
use serde::Serialize;
use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

const MIN_ZOOM: f32 = 0.5;
const MAX_ZOOM: f32 = 3.0;
const MIN_DEVICE_PIXEL_RATIO: f32 = 1.0;
const MAX_DEVICE_PIXEL_RATIO: f32 = 4.0;

#[derive(Debug, Serialize)]
pub struct PdfPageRenderResponse {
    pub image_path: String,
    pub page: u16,
    pub page_count: u16,
    pub zoom: f32,
}

#[derive(Debug, Serialize)]
pub struct PdfPageTextSegmentResponse {
    pub text: String,
    pub left_ratio: f32,
    pub top_ratio: f32,
    pub width_ratio: f32,
    pub height_ratio: f32,
}

#[derive(Debug, Serialize)]
pub struct PdfPageTextResponse {
    pub page: u16,
    pub page_count: u16,
    pub page_width_points: f32,
    pub page_height_points: f32,
    pub text: String,
    pub segments: Vec<PdfPageTextSegmentResponse>,
}

pub fn render_pdf_page(
    file_path: &str,
    page: u16,
    zoom: f32,
    device_pixel_ratio: f32,
    cache_root: &Path,
) -> Result<PdfPageRenderResponse, String> {
    let input_path = PathBuf::from(file_path);

    if !input_path.exists() {
        return Err(format!(
            "PDF file was not found at '{}'.",
            input_path.display()
        ));
    }

    if !is_pdf_path(&input_path) {
        return Err("Selected file is not a PDF.".to_string());
    }

    if page == 0 {
        return Err("Page index is 1-based; page must be >= 1.".to_string());
    }

    let normalized_zoom = normalize_zoom(zoom);
    let normalized_device_pixel_ratio = normalize_device_pixel_ratio(device_pixel_ratio);
    let effective_scale = normalized_zoom * normalized_device_pixel_ratio;
    let pdfium = bind_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(&input_path, None)
        .map_err(|error| format!("Failed to load PDF document: {error}"))?;

    let page_count = document.pages().len();

    if page > page_count {
        return Err(format!(
            "Page {page} is out of range. This document has {page_count} page(s)."
        ));
    }

    let output_path = output_path_for_render(
        cache_root,
        &input_path,
        page,
        normalized_zoom,
        normalized_device_pixel_ratio,
    )?;

    if is_valid_cached_render(&output_path) {
        return Ok(PdfPageRenderResponse {
            image_path: output_path.to_string_lossy().into_owned(),
            page,
            page_count,
            zoom: normalized_zoom,
        });
    }

    // Backend command uses 1-based indexing externally; PDFium page APIs are 0-based.
    let page_index = page - 1;
    let pdf_page = document
        .pages()
        .get(page_index)
        .map_err(|error| format!("Failed to read page {page}: {error}"))?;

    let render_config = PdfRenderConfig::new().scale_page_by_factor(effective_scale);
    let bitmap = pdf_page
        .render_with_config(&render_config)
        .map_err(|error| format!("Failed to render page {page}: {error}"))?;

    write_bitmap_as_png(&bitmap, &output_path)?;

    Ok(PdfPageRenderResponse {
        image_path: output_path.to_string_lossy().into_owned(),
        page,
        page_count,
        zoom: normalized_zoom,
    })
}

pub fn extract_pdf_page_text(file_path: &str, page: u16) -> Result<PdfPageTextResponse, String> {
    let input_path = PathBuf::from(file_path);

    if !input_path.exists() {
        return Err(format!(
            "PDF file was not found at '{}'.",
            input_path.display()
        ));
    }

    if !is_pdf_path(&input_path) {
        return Err("Selected file is not a PDF.".to_string());
    }

    if page == 0 {
        return Err("Page index is 1-based; page must be >= 1.".to_string());
    }

    let pdfium = bind_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(&input_path, None)
        .map_err(|error| format!("Failed to load PDF document: {error}"))?;

    let page_count = document.pages().len();

    if page > page_count {
        return Err(format!(
            "Page {page} is out of range. This document has {page_count} page(s)."
        ));
    }

    // Backend command uses 1-based indexing externally; PDFium page APIs are 0-based.
    let page_index = page - 1;
    let pdf_page = document
        .pages()
        .get(page_index)
        .map_err(|error| format!("Failed to read page {page}: {error}"))?;

    let page_width_points = normalize_page_dimension(pdf_page.width().value);
    let page_height_points = normalize_page_dimension(pdf_page.height().value);

    let text_page = pdf_page
        .text()
        .map_err(|error| format!("Failed to extract text from page {page}: {error}"))?;
    let page_text = normalize_extracted_text(text_page.all());
    let mut segments: Vec<PdfPageTextSegmentResponse> = Vec::new();

    for segment in text_page.segments().iter() {
        let segment_text = normalize_extracted_text(segment.text());

        if segment_text.trim().is_empty() {
            continue;
        }

        let bounds = segment.bounds();
        let (left_ratio, top_ratio, width_ratio, height_ratio) = rect_to_normalized_ratios(
            bounds,
            page_width_points,
            page_height_points,
        );

        if width_ratio <= 0.0 || height_ratio <= 0.0 {
            continue;
        }

        segments.push(PdfPageTextSegmentResponse {
            text: segment_text,
            left_ratio,
            top_ratio,
            width_ratio,
            height_ratio,
        });
    }

    Ok(PdfPageTextResponse {
        page,
        page_count,
        page_width_points,
        page_height_points,
        text: page_text,
        segments,
    })
}

fn bind_pdfium() -> Result<Pdfium, String> {
    if let Ok(raw_path) = env::var("PDFIUM_DYNAMIC_LIB_PATH") {
        let trimmed = raw_path.trim();

        if !trimmed.is_empty() {
            let env_path = PathBuf::from(trimmed);
            let candidate = if env_path.is_dir() {
                env_path.join(Pdfium::pdfium_platform_library_name())
            } else {
                env_path
            };

            match Pdfium::bind_to_library(&candidate) {
                Ok(bindings) => return Ok(Pdfium::new(bindings)),
                Err(env_error) => {
                    if let Ok(bindings) = Pdfium::bind_to_system_library() {
                        return Ok(Pdfium::new(bindings));
                    }

                    return Err(format!(
                        "PDF engine unavailable. Could not load library from PDFIUM_DYNAMIC_LIB_PATH ('{}'): {env_error}. Also failed to load system PDFium library.",
                        candidate.display()
                    ));
                }
            }
        }
    }

    let local_candidate = Pdfium::pdfium_platform_library_name_at_path("./");

    match Pdfium::bind_to_library(&local_candidate) {
        Ok(bindings) => Ok(Pdfium::new(bindings)),
        Err(local_error) => match Pdfium::bind_to_system_library() {
            Ok(bindings) => Ok(Pdfium::new(bindings)),
            Err(system_error) => Err(format!(
                "PDF engine unavailable. Tried local library '{}' and system library. Local error: {local_error}. System error: {system_error}.",
                local_candidate.display()
            )),
        },
    }
}

fn output_path_for_render(
    cache_root: &Path,
    input_path: &Path,
    page: u16,
    zoom: f32,
    device_pixel_ratio: f32,
) -> Result<PathBuf, String> {
    let file_identity = file_identity_tag(input_path);
    let render_dir = cache_root
        .join("pdf-renders")
        .join(format!("{}-{}", file_stem_safe(input_path), file_identity));

    fs::create_dir_all(&render_dir).map_err(|error| {
        format!(
            "Failed to create render cache directory '{}': {error}",
            render_dir.display()
        )
    })?;

    let zoom_tag = (zoom * 100.0).round() as u16;
    let dpr_tag = device_pixel_ratio_tag(device_pixel_ratio);
    let output_name = format!("page-{page}-z{zoom_tag}-dpr{dpr_tag}.png");

    Ok(render_dir.join(output_name))
}

fn is_valid_cached_render(output_path: &Path) -> bool {
    let metadata = match fs::metadata(output_path) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };

    metadata.is_file() && metadata.len() > 0
}

fn write_bitmap_as_png(bitmap: &PdfBitmap<'_>, output_path: &Path) -> Result<(), String> {
    let rgba_image = bitmap.as_image().into_rgba8();
    let (width, height) = rgba_image.dimensions();
    let rgba_bytes = rgba_image.into_raw();

    let output_file = fs::File::create(output_path).map_err(|error| {
        format!(
            "Failed to create rendered page image file '{}': {error}",
            output_path.display()
        )
    })?;
    let writer = BufWriter::new(output_file);

    let mut encoder = png::Encoder::new(writer, width, height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);

    let mut png_writer = encoder
        .write_header()
        .map_err(|error| format!("Failed to create PNG header: {error}"))?;

    png_writer
        .write_image_data(&rgba_bytes)
        .map_err(|error| format!("Failed to write PNG image data: {error}"))
}

fn file_stem_safe(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("document");

    let mut output = String::new();

    for character in stem.chars() {
        if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
            output.push(character);
        } else {
            output.push('_');
        }
    }

    if output.is_empty() {
        "document".to_string()
    } else {
        output
    }
}

fn file_identity_tag(path: &Path) -> String {
    let canonical_path = match path.canonicalize() {
        Ok(path) => path,
        Err(_) => path.to_path_buf(),
    };
    let identity_source = canonical_path.to_string_lossy();
    let mut hasher = DefaultHasher::new();
    identity_source.hash(&mut hasher);

    format!("{:016x}", hasher.finish())
}

fn is_pdf_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}

fn normalize_extracted_text(raw_text: String) -> String {
    raw_text
        .replace('\0', "")
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}

fn normalize_page_dimension(value: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        1.0
    }
}

fn rect_to_normalized_ratios(
    bounds: PdfRect,
    page_width_points: f32,
    page_height_points: f32,
) -> (f32, f32, f32, f32) {
    let safe_width = normalize_page_dimension(page_width_points);
    let safe_height = normalize_page_dimension(page_height_points);

    let raw_left = bounds.left().value / safe_width;
    let raw_right = bounds.right().value / safe_width;
    let left_ratio = clamp_unit(raw_left.min(raw_right));
    let right_ratio = clamp_unit(raw_left.max(raw_right));
    let width_ratio = (right_ratio - left_ratio).max(0.0);

    // PDF coordinate system uses bottom-left origin. Convert to top-left ratio space.
    let raw_top_from_top = 1.0 - (bounds.top().value / safe_height);
    let raw_bottom_from_top = 1.0 - (bounds.bottom().value / safe_height);
    let top_ratio = clamp_unit(raw_top_from_top.min(raw_bottom_from_top));
    let bottom_ratio = clamp_unit(raw_top_from_top.max(raw_bottom_from_top));
    let height_ratio = (bottom_ratio - top_ratio).max(0.0);

    (left_ratio, top_ratio, width_ratio, height_ratio)
}

fn clamp_unit(value: f32) -> f32 {
    if !value.is_finite() {
        return 0.0;
    }

    value.clamp(0.0, 1.0)
}

fn normalize_zoom(zoom: f32) -> f32 {
    if !zoom.is_finite() {
        return 1.0;
    }

    zoom.clamp(MIN_ZOOM, MAX_ZOOM)
}

fn normalize_device_pixel_ratio(device_pixel_ratio: f32) -> f32 {
    if !device_pixel_ratio.is_finite() {
        return 1.0;
    }

    device_pixel_ratio.clamp(MIN_DEVICE_PIXEL_RATIO, MAX_DEVICE_PIXEL_RATIO)
}

fn device_pixel_ratio_tag(device_pixel_ratio: f32) -> String {
    let rounded = (device_pixel_ratio * 100.0).round() / 100.0;

    if (rounded - rounded.round()).abs() < f32::EPSILON {
        return format!("{}", rounded.round() as u16);
    }

    let mut tag = format!("{rounded:.2}");

    while tag.ends_with('0') {
        tag.pop();
    }

    if tag.ends_with('.') {
        tag.pop();
    }

    tag.replace('.', "_")
}
