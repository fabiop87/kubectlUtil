//! Interface egui. Cada seção tem seu arquivo; aqui ficam o laço principal,
//! a navegação lateral, o painel de saída e o histórico.

use super::{KubectlUtilApp, MockScenario, Section};
use crate::app::util::file_stamp;
use eframe::egui;
use std::sync::atomic::Ordering;
use std::time::Duration;

mod aws;
mod connection;
mod parameter_store;
mod sections;
mod widgets;

impl eframe::App for KubectlUtilApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let screen_rect = ctx.screen_rect();
        let background = ctx.style().visuals.panel_fill;
        ctx.layer_painter(egui::LayerId::background()).rect_filled(screen_rect, 0.0, background);

        while let Ok(result) = self.rx.try_recv() {
            self.handle_result(result);
        }
        self.poll_port_forwards();
        // Só precisa acordar periodicamente para vigiar processos de port-forward reais;
        // resultados de comandos acordam a interface sozinhos (ver `launch`).
        if self.port_forwards.has_processes() {
            ctx.request_repaint_after(Duration::from_millis(500));
        }

        self.show_navigation(ctx);

        egui::TopBottomPanel::bottom("output_panel")
            .resizable(true)
            .default_height(350.0)
            .min_height(200.0)
            .show(ctx, |ui| self.show_output_panel(ui));

        let central_fill = ctx.style().visuals.panel_fill;
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).fill(central_fill))
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.heading(self.section.label());
                ui.label("Execute somente as consultas necessárias.");
                ui.add_space(12.0);

                if self.section != Section::Connection {
                    self.show_context_bar(ui);
                }
                ui.add_space(10.0);

                match self.section {
                    Section::Overview => self.show_overview(ui),
                    Section::Connection => self.show_connection(ui),
                    Section::Cluster => self.show_cluster(ui),
                    Section::Network => self.show_network(ui),
                    Section::Aws => self.show_aws(ui),
                    Section::Parameters => self.show_parameters(ui),
                }

                ui.add_space(8.0);
                self.show_history(ui);
            });
    }
}

impl KubectlUtilApp {
    fn show_navigation(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("navigation").exact_width(190.0).resizable(false).show(ctx, |ui| {
            ui.add_space(12.0);
            ui.heading("◈ kubectlUtil");
            ui.label(egui::RichText::new("Kubernetes & AWS").small().color(egui::Color32::GRAY));
            ui.add_space(10.0);

            let was_mock = self.mock_mode;
            ui.add_enabled_ui(!self.busy, |ui| {
                ui.checkbox(&mut self.mock_mode, "Modo Mock (sem rede)");
            });
            if self.mock_mode {
                ui.label(
                    egui::RichText::new("Nenhum comando real será executado.")
                        .small()
                        .color(egui::Color32::DARK_GREEN),
                );
                egui::ComboBox::from_label("Teste")
                    .selected_text(self.mock_scenario.label())
                    .width(180.0)
                    .show_ui(ui, |ui| {
                        for scenario in MockScenario::ALL {
                            ui.selectable_value(&mut self.mock_scenario, scenario, scenario.label());
                        }
                    });
            }
            if was_mock != self.mock_mode {
                self.on_mock_toggled();
            }

            ui.add_space(14.0);
            for section in [
                Section::Connection,
                Section::Overview,
                Section::Cluster,
                Section::Network,
                Section::Aws,
                Section::Parameters,
            ] {
                self.nav_button(ui, section);
            }
            ui.add_space(20.0);
            ui.separator();
            ui.label(egui::RichText::new("As consultas são feitas sob demanda.").small().color(egui::Color32::GRAY));
        });
    }

    fn nav_button(&mut self, ui: &mut egui::Ui, section: Section) {
        if ui.selectable_label(self.section == section, section.label()).clicked() {
            self.section = section;
        }
    }

    fn show_context_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let context = if self.context.value.is_empty() { "não selecionado" } else { self.context.value.as_str() };
            ui.label(format!("Contexto: {context}"));
            ui.separator();
            let namespace =
                if self.namespace.value.is_empty() { "não selecionado" } else { self.namespace.value.as_str() };
            ui.label(format!("Namespace: {namespace}"));
            if ui.button("Abrir conexão").clicked() {
                self.section = Section::Connection;
            }
        });
        ui.separator();
    }

    fn show_output_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(&self.output_title);
            ui.separator();
            let status = if self.busy { "Executando…" } else { self.status.as_str() };
            ui.label(status);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Limpar").clicked() {
                    self.output.clear();
                }
                if ui.button("Copiar").clicked() {
                    ui.ctx().copy_text(self.output.clone());
                    self.status = "Saída copiada.".into();
                }
                if ui.button("Salvar .log").clicked() {
                    self.save_output_log();
                }
                if self.busy && ui.button("Cancelar").clicked() {
                    self.cancel.store(true, Ordering::Relaxed);
                    self.status = "Cancelando…".into();
                }
            });
        });
        ui.add_space(6.0);

        egui::ScrollArea::vertical()
            .id_salt("output_scroll_area")
            .max_height(f32::INFINITY)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.add_space(4.0);
                // `&str` é um buffer somente leitura: o texto continua selecionável e copiável.
                let mut text: &str = self.output.as_str();
                ui.add(
                    egui::TextEdit::multiline(&mut text)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .desired_rows(16),
                );
            });
    }

    fn save_output_log(&mut self) {
        let file_name = format!("kubectlUtil-output-{}.log", file_stamp());
        self.status = match std::fs::write(&file_name, &self.output) {
            Ok(()) => format!("Saída salva em {file_name}"),
            Err(e) => format!("Não foi possível salvar o log: {e}"),
        };
    }

    fn show_history(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(format!("Histórico local de operações ({} entradas)", self.history.len()))
            .default_open(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Copiar histórico").clicked() {
                        ui.ctx().copy_text(self.history.join("\n"));
                        self.status = "Histórico copiado.".into();
                    }
                    if ui.button("Limpar histórico").clicked() {
                        self.history.clear();
                    }
                });
                egui::ScrollArea::vertical().id_salt("history_scroll_area").max_height(100.0).show(ui, |ui| {
                    for item in self.history.iter().rev().take(50) {
                        ui.monospace(item);
                    }
                });
            });
    }
}
