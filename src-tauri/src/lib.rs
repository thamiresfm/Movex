mod clipboard;
mod config;
mod core;
mod input;
mod network;
mod screen;
mod transfer;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar Movex");
}
