//! Interface egui. Cada seção tem seu arquivo; aqui ficam o laço principal,
//! a navegação lateral, o painel de saída e o histórico. Cores, fontes e
//! arredondamentos vêm de `crate::theme`; componentes de `widgets.rs`.

use super::{KubectlUtilApp, MockScenario, Section};
use crate::app::util::file_stamp;
use crate::theme::{self, Palette};
use eframe::egui;
use std::sync::atomic::Ordering;
use std::time::Duration;

mod aws;
mod connection;
mod parameter_store;
mod sections;
mod widgets;

use self::widgets::{card_frame, chip, danger_button, palette_of};

impl eframe::App for KubectlUtilApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // let p = theme::palette(ctx.style().visuals.dark_mode);
        let p = theme::palette(self.dark_mode);

        // Evita que áreas não cobertas pelos painéis fiquem pretas ao alternar o Mock.
        let screen_rect = ctx.screen_rect();
        ctx.layer_painter(egui::LayerId::background())
            .rect_filled(screen_rect, 0.0, p.bg);

        while let Ok(result) = self.rx.try_recv() {
            self.handle_result(result);
        }
        if ctx.input(|input| input.viewport().minimized == Some(true)) {
            self.params.hide_value();
        }
        self.poll_port_forwards();
        // Só precisa acordar periodicamente para vigiar processos de port-forward reais;
        // resultados de comandos acordam a interface sozinhos (ver `launch`).
        if self.port_forwards.has_processes() {
            ctx.request_repaint_after(Duration::from_millis(500));
        }

        self.show_navigation(ctx, &p);

        egui::TopBottomPanel::bottom("output_panel")
            .resizable(true)
            .default_height(350.0)
            .min_height(200.0)
            .frame(
                egui::Frame::new()
                    .fill(p.bg)
                    .inner_margin(egui::Margin::symmetric(24, 14)),
            )
            .show(ctx, |ui| self.show_output_panel(ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(p.bg)
                    .inner_margin(egui::Margin::symmetric(24, 20)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("central_scroll_area")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(self.section.label())
                                .size(20.0)
                                .strong()
                                .color(p.text),
                        );
                        ui.label(
                            egui::RichText::new("Execute somente as consultas necessárias.")
                                .color(p.text),
                        );
                        //ui.add_space(10.0);

                        if self.section != Section::Connection {
                            self.show_context_bar(ui);
                            //ui.add_space(5.0);
                        }

                        match self.section {
                            Section::Overview => self.show_overview(ui),
                            Section::Connection => self.show_connection(ui),
                            Section::Cluster => self.show_cluster(ui),
                            Section::Network => self.show_network(ui),
                            Section::Aws => self.show_aws(ui),
                            Section::Parameters => self.show_parameters(ui),
                        }

                        //ui.add_space(5.0);
                        self.show_history(ui);
                        //ui.add_space(6.0);
                    });
            });
    }
}

impl KubectlUtilApp {
    fn show_navigation(&mut self, ctx: &egui::Context, p: &Palette) {
        egui::SidePanel::left("navigation")
            .exact_width(220.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(p.surface)
                    .inner_margin(egui::Margin::same(16)),
            )
            .show(ctx, |ui| {
                //ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("◈ kubectlUtil")
                        .size(21.0)
                        .strong()
                        .color(p.accent),
                );
                ui.label(
                    egui::RichText::new("Kubernetes & AWS")
                        .small()
                        .color(p.muted),
                );
                //ui.add_space(10.0);

                self.show_mock_controls(ui, p);

                //ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("SEÇÕES")
                        .small()
                        .strong()
                        .color(p.muted),
                );
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
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
                });

                // Rodapé fixo na base do painel.
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.label(
                        egui::RichText::new("As consultas são feitas sob demanda.")
                            .small()
                            .color(p.muted),
                    );
                    //ui.add_space(6.0);
                    self.show_theme_toggle(ctx, ui);
                });
            });
    }

    fn show_mock_controls(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let was_mock = self.mock_mode;
        egui::Frame::new()
            .fill(p.surface_alt)
            .corner_radius(egui::CornerRadius::same(10))
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.add_enabled_ui(!self.busy, |ui| {
                    ui.checkbox(&mut self.mock_mode, "Modo Mock (sem rede)");
                });
                if self.mock_mode {
                    ui.label(
                        egui::RichText::new("Nenhum comando real será executado.")
                            .small()
                            .color(p.success),
                    );
                    ui.label(egui::RichText::new("Cenário").small().color(p.muted));
                    egui::ComboBox::from_id_salt("mock_scenario")
                        .selected_text(self.mock_scenario.label())
                        .width(150.0)
                        .show_ui(ui, |ui| {
                            for scenario in MockScenario::ALL {
                                ui.selectable_value(
                                    &mut self.mock_scenario,
                                    scenario,
                                    scenario.label(),
                                );
                            }
                        });
                }
            });
        if was_mock != self.mock_mode {
            self.on_mock_toggled();
        }
    }

    fn show_theme_toggle(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let label = if self.dark_mode {
            "☀  Tema claro"
        } else {
            "🌙  Tema escuro"
        };
        if ui
            .add_sized([ui.available_width(), 32.0], egui::Button::new(label))
            .clicked()
        {
            self.dark_mode = !self.dark_mode;
            theme::set_dark(ctx, self.dark_mode);
        }
    }

    fn nav_button(&mut self, ui: &mut egui::Ui, section: Section) {
        let selected = self.section == section;
        let p = theme::palette(self.dark_mode);

        let mut text = egui::RichText::new(section.label()).color(p.text);

        if selected {
            text = text.strong();
        }
        if selected {
            text = text.strong();
        }
        let mut button = egui::Button::new(text)
            .min_size(egui::vec2(0.0, 30.0))
            .selected(selected);
        if !selected {
            // Itens inativos sem moldura; o destaque aparece ao passar o mouse.
            button = button
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE);
        }
        if ui.add(button).clicked() {
            if self.section == Section::Parameters && section != Section::Parameters {
                self.params.hide_value();
            }
            self.section = section;
        }
    }

    fn show_context_bar(&mut self, ui: &mut egui::Ui) {
        let context_set = !self.context.value.is_empty();
        let namespace_set = !self.namespace.value.is_empty();
        let context = if context_set {
            self.context.value.clone()
        } else {
            "não selecionado".to_string()
        };
        let namespace = if namespace_set {
            self.namespace.value.clone()
        } else {
            "não selecionado".to_string()
        };
        ui.horizontal_wrapped(|ui| {
            chip(ui, "Contexto", &context, context_set);
            chip(ui, "Namespace", &namespace, namespace_set);
            if ui.button("Abrir conexão").clicked() {
                if self.section == Section::Parameters {
                    self.params.hide_value();
                }
                self.section = Section::Connection;
            }
        });
    }

    fn show_output_panel(&mut self, ui: &mut egui::Ui) {
        let p = palette_of(ui);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(self.output_title.as_str())
                    .size(16.0)
                    .strong()
                    .color(p.text),
            );
            //ui.add_space(6.0);
            if self.busy {
                ui.spinner();
                ui.label("Executando…");
            } else {
                ui.label(egui::RichText::new(self.status.as_str()).color(p.muted));
            }
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
                if self.busy && danger_button(ui, "Cancelar").clicked() {
                    self.cancel.store(true, Ordering::Relaxed);
                    self.status = "Cancelando…".into();
                }
            });
        });
        //ui.add_space(6.0);

        // Console escuro: legível nos dois temas.
        egui::Frame::new()
            .fill(p.console_bg)
            .corner_radius(egui::CornerRadius::same(10))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.visuals_mut().override_text_color = Some(p.console_text);
                ui.visuals_mut().selection.bg_fill = egui::Color32::from_rgb(55, 65, 140);
                egui::ScrollArea::vertical()
                    .id_salt("output_scroll_area")
                    .max_height(f32::INFINITY)
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        // `&str` é um buffer somente leitura: o texto continua selecionável e copiável.
                        let mut text: &str = self.output.as_str();
                        ui.add(
                            egui::TextEdit::multiline(&mut text)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .desired_rows(16)
                                .frame(false),
                        );
                    });
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
        let p = palette_of(ui);
        card_frame(&p).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            egui::CollapsingHeader::new(
                egui::RichText::new(format!(
                    "Histórico local de operações ({} entradas)",
                    self.history.len()
                ))
                .size(12.0)
                .strong()
                .color(p.text),
            )
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
                egui::ScrollArea::vertical()
                    .id_salt("history_scroll_area")
                    .max_height(120.0)
                    .show(ui, |ui| {
                        for item in self.history.iter().rev().take(50) {
                            ui.monospace(item);
                        }
                    });
            });
        });
    }
}
