//! Aba "Parameter Store" (somente leitura).

use super::widgets::{card, palette_of, primary_button};
use crate::app::KubectlUtilApp;
use eframe::egui;

impl KubectlUtilApp {
    pub(super) fn show_parameters(&mut self, ui: &mut egui::Ui) {
        self.show_parameter_list(ui);
        ui.add_space(12.0);
        self.show_parameter_value(ui);

        let muted = palette_of(ui).muted;
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(
                "Os resultados ficam em cache em memória por perfil, região e caminho até atualizar, trocar a consulta ou fechar o aplicativo. SecureString só é descriptografado quando você solicita o valor.",
            )
            .small()
            .color(muted),
        );
    }

    fn show_parameter_list(&mut self, ui: &mut egui::Ui) {
        let muted = palette_of(ui).muted;
        card(
            ui,
            "AWS Systems Manager — Parameter Store",
            Some("Somente leitura. O aplicativo não cria, altera nem exclui parâmetros."),
            |ui| {
                ui.label(
                    egui::RichText::new(format!("Perfil: {}    Região: {}", self.profile.value, self.region))
                        .color(muted),
                );
                ui.horizontal_wrapped(|ui| {
                    ui.label("Caminho");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.params.path)
                            .desired_width(300.0)
                            .hint_text("/producao/minha-aplicacao/"),
                    );
                    if primary_button(ui, !self.busy, "Carregar (usar cache)").clicked() {
                        self.load_parameters(false);
                    }
                    if ui.add_enabled(!self.busy, egui::Button::new("Atualizar da AWS")).clicked() {
                        self.load_parameters(true);
                    }
                    if ui.add_enabled(!self.busy, egui::Button::new("Limpar cache")).clicked() {
                        self.clear_parameter_cache();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Filtrar nomes");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.params.filter)
                            .desired_width(280.0)
                            .hint_text("Digite parte do nome"),
                    );
                    ui.label(egui::RichText::new(format!("{} parâmetros em memória", self.params.entries.len())).color(muted));
                });

                egui::ScrollArea::vertical()
                    .id_salt("parameter_store_scroll_area")
                    .max_height(220.0)
                    .show(ui, |ui| {
                        egui::Grid::new("ssm_parameters_grid").striped(true).spacing([18.0, 6.0]).show(ui, |ui| {
                            ui.strong("Nome");
                            ui.strong("Tipo");
                            ui.strong("Última modificação");
                            ui.end_row();
                            let filter = self.params.filter.to_lowercase();
                            let visible: Vec<_> = self
                                .params
                                .entries
                                .iter()
                                .filter(|p| p.name.to_lowercase().contains(&filter))
                                .cloned()
                                .collect();
                            for p in visible {
                                if ui.selectable_label(self.params.selected == p.name, &p.name).clicked() {
                                    self.params.select(p.name.clone());
                                }
                                ui.label(&p.kind);
                                ui.label(&p.modified);
                                ui.end_row();
                            }
                        });
                    });
            },
        );
    }

    fn show_parameter_value(&mut self, ui: &mut egui::Ui) {
        let muted = palette_of(ui).muted;
        card(
            ui,
            "Valor do parâmetro",
            Some("Consulta individual e sob demanda; o valor não vai ao histórico nem ao log geral."),
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    let selected = if self.params.selected.is_empty() {
                        "Nenhum parâmetro selecionado".to_string()
                    } else {
                        self.params.selected.clone()
                    };
                    ui.label(egui::RichText::new(selected).strong());
                    let can_reveal = !self.busy && !self.params.selected.is_empty();
                    if primary_button(ui, can_reveal, "Consultar valor").clicked() {
                        self.reveal_selected_parameter();
                    }
                    if self.params.revealed_visible {
                        if ui.button("Ocultar valor").clicked() {
                            // Além de ocultar, apaga o segredo da memória do aplicativo.
                            self.params.hide_value();
                        }
                        if ui.button("Copiar valor").clicked() {
                            ui.ctx().copy_text(self.params.revealed.clone());
                            self.status = "Valor copiado para a área de transferência; limpe-a se necessário.".into();
                        }
                    }
                });

                if self.params.revealed_visible {
                    // `&str` é somente leitura, mas continua selecionável.
                    let mut value: &str = self.params.revealed.as_str();
                    ui.add(
                        egui::TextEdit::multiline(&mut value)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .desired_rows(3),
                    );
                } else if !self.params.selected.is_empty() {
                    ui.label(
                        egui::RichText::new("Valor oculto. Clique em ‘Consultar valor’ para fazer uma consulta AWS individual.")
                            .color(muted),
                    );
                }
            },
        );
    }
}
