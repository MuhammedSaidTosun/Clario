mod commands;
mod pdf;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            commands::ensure_app_dirs(app.handle().clone()).map_err(std::io::Error::other)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            commands::app_dirs,
            commands::ensure_app_dirs,
            commands::log_info,
            commands::pdf_reader::render_pdf_page,
            commands::pdf_reader::extract_pdf_page_text
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
