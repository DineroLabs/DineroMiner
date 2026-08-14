pub mod address;
pub mod commands;
pub mod settings;
pub mod stats;
pub mod supervisor;
pub mod worksource;

use commands::MinerState;
use std::sync::Mutex;

pub fn run() {
    tauri::Builder::default()
        .manage(MinerState(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            commands::validate_address_cmd,
            commands::get_settings,
            commands::save_settings,
            commands::start_mining,
            commands::stop_mining,
        ])
        .run(tauri::generate_context!())
        .expect("error while running DineroMiner");
}
