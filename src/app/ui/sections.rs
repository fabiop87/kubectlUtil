//! Abas baseadas em presets: Visão geral, Cluster e Rede/storage.
//! Para incluir um botão, edite `presets.rs`; nada muda aqui.

use super::widgets::card;
use crate::app::KubectlUtilApp;
use crate::app::presets::{self, Preset};
use eframe::egui;

impl KubectlUtilApp {
    pub(super) fn show_overview(&mut self, ui: &mut egui::Ui) {
        card(
            ui,
            "Acesso rápido",
            Some("As consultas mais usadas, uma por clique."),
            |ui| {
                ui.horizontal_wrapped(|ui| self.preset_buttons(ui, presets::OVERVIEW));
            },
        );
    }

    pub(super) fn show_cluster(&mut self, ui: &mut egui::Ui) {
        card(
            ui,
            "Consultas do cluster",
            Some("Nodes, workloads, eventos, Services e imagens."),
            |ui| {
                ui.horizontal_wrapped(|ui| self.preset_buttons(ui, presets::CLUSTER));
            },
        );
    }

    pub(super) fn show_network(&mut self, ui: &mut egui::Ui) {
        card(
            ui,
            "Rede, storage e backups",
            Some("Ingress, endpoints, volumes, ConfigMaps e Velero."),
            |ui| {
                ui.horizontal_wrapped(|ui| self.preset_buttons(ui, presets::NETWORK));
            },
        );
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
