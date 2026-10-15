//! Abas baseadas em presets: Visão geral, Cluster e Rede/storage.
//! Para incluir um botão, edite `presets.rs`; nada muda aqui.

use crate::app::presets::{self, Preset};
use crate::app::KubectlUtilApp;
use eframe::egui;

impl KubectlUtilApp {
    pub(super) fn show_overview(&mut self, ui: &mut egui::Ui) {
        ui.label("Acesso rápido");
        ui.horizontal_wrapped(|ui| self.preset_buttons(ui, presets::OVERVIEW));
    }

    pub(super) fn show_cluster(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| self.preset_buttons(ui, presets::CLUSTER));
    }

    pub(super) fn show_network(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| self.preset_buttons(ui, presets::NETWORK));
    }

    /// Um botão por preset; valida as seleções necessárias antes de executar.
    pub(super) fn preset_buttons(&mut self, ui: &mut egui::Ui, list: &[Preset]) {
        for preset in list {
            if ui.button(preset.label).clicked() {
                self.run_preset(preset);
            }
        }
    }
}
