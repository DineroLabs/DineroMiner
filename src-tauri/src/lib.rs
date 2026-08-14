pub mod settings;
pub mod supervisor;
pub mod stats;
pub mod worksource;
pub mod address;
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running DineroMiner");
}
