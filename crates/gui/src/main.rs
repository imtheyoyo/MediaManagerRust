#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

struct Application;

impl eframe::App for Application {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        ui.heading("MediaManagerRust");
        ui.label("Médiathèque films et séries — squelette de l'interface.");
    }
}

fn main() -> eframe::Result {
    eframe::run_native(
        "MediaManagerRust",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(Application))),
    )
}
