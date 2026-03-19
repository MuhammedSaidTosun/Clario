use crate::pdf::{
    extract_pdf_page_text as extract_pdf_page_text_for_page,
    render_pdf_page as render_pdf_page_to_cache, PdfPageRenderResponse, PdfPageTextResponse,
};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const CACHE_DIR_NAME: &str = "cache";

#[tauri::command]
pub fn render_pdf_page(
    app: AppHandle,
    file_path: String,
    page: u16,
    zoom: f32,
    device_pixel_ratio: f32,
) -> Result<PdfPageRenderResponse, String> {
    super::ensure_app_dirs(app.clone())?;

    let cache_dir = app_data_dir(&app)?.join(CACHE_DIR_NAME);

    render_pdf_page_to_cache(&file_path, page, zoom, device_pixel_ratio, &cache_dir)
}

#[tauri::command]
pub fn extract_pdf_page_text(
    app: AppHandle,
    file_path: String,
    page: u16,
) -> Result<PdfPageTextResponse, String> {
    super::ensure_app_dirs(app)?;
    extract_pdf_page_text_for_page(&file_path, page)
}

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|error| format!("failed to resolve app_data_dir: {error}"))
}
