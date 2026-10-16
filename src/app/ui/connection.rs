//! Aba "Conexão Kubernetes": seleção de contexto/namespace/Pod/Service,
//! explorador de Pods, troubleshooting e Port Forward.

use super::widgets::{card, danger_button, field_combo, palette_of, primary_button};
use crate::app::presets;
use crate::app::KubectlUtilApp;
use eframe::egui;

impl KubectlUtilApp {
    pub(super) fn show_connection(&mut self, ui: &mut egui::Ui) {
        self.show_connection_selectors(ui);
        ui.add_space(12.0);
        self.show_pod_explorer(ui);
        ui.add_space(12.0);
        self.show_troubleshooting(ui);
        ui.add_space(12.0);
        self.show_port_forward(ui);
    }

    fn show_connection_selectors(&mut self, ui: &mut egui::Ui) {
        card(
            ui,
            "Conexão Kubernetes",
            Some("O aplicativo usa o contexto e o namespace escolhidos aqui em todas as consultas."),
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    let previous_context = self.context.value.clone();
                    field_combo(ui, "Contexto", &mut self.context, 300.0);
                    if self.context.value != previous_context {
                        self.on_context_changed();
                    }

                    if ui.button("Atualizar").clicked() {
                        self.refresh_contexts();
                    }

                    if ui
                        .button("Gravar no kubeconfig")
                        .on_hover_text(
                            "Executa `kubectl config use-context`. O aplicativo já usa o contexto selecionado em cada consulta; \
                             isto só altera o padrão dos seus outros terminais.",
                        )
                        .clicked()
                    {
                        self.use_selected_context();
                    }
                });

                ui.horizontal_wrapped(|ui| {
                    field_combo(ui, "Namespace", &mut self.namespace, 180.0);

                    if ui.button("Carregar namespaces").clicked() {
                        self.refresh_namespaces();
                    }
                    if ui.button("Pods").clicked() {
                        self.load_pods();
                    }
                    if ui.button("Services").clicked() {
                        self.load_services();
                    }
                });

                ui.horizontal_wrapped(|ui| {
                    field_combo(ui, "Pod", &mut self.pod, 300.0);
                    field_combo(ui, "Service", &mut self.service, 300.0);
                });
            },
        );
    }

    fn show_pod_explorer(&mut self, ui: &mut egui::Ui) {
        card(
            ui,
            "Explorador e diagnóstico",
            Some("Uma consulta alimenta a lista, o filtro e o diagnóstico rápido."),
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Carregar/atualizar Pods (1 consulta)").clicked() {
                        self.load_pod_explorer();
                    }
                    if ui.button("Diagnóstico rápido (1 consulta)").clicked() {
                        self.run_pod_diagnostics();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Filtrar lista carregada:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.filter)
                            .hint_text("nome, status, node…")
                            .desired_width(260.0),
                    );
                });
                if !self.explorer_output.is_empty() {
                    let needle = self.filter.to_lowercase();
                    egui::ScrollArea::vertical()
                        .id_salt("workloads_pods_scroll_area")
                        .max_height(150.0)
                        .show(ui, |ui| {
                            for line in self.explorer_output.lines().filter(|l| l.to_lowercase().contains(&needle)) {
                                ui.monospace(line);
                            }
                        });
                }
            },
        );
    }

    fn show_troubleshooting(&mut self, ui: &mut egui::Ui) {
        card(ui, "Troubleshooting", Some("Ferramentas para o Pod selecionado."), |ui| {
            ui.horizontal_wrapped(|ui| {
                self.preset_buttons(ui, presets::POD_TOOLS);
                if ui.button("Bash no Pod").clicked() {
                    self.show_exec_hint();
                }
            });
        });
    }

    /// Bash interativo precisa de um terminal: mostra o comando pronto para copiar.
    fn show_exec_hint(&mut self) {
        let namespace = self.namespace.trimmed().to_string();
        let pod = self.pod.trimmed().to_string();
        if namespace.is_empty() || pod.is_empty() {
            self.status = "Selecione namespace e Pod.".into();
            return;
        }
        let context = self.context.trimmed();
        let context_flag = if context.is_empty() { String::new() } else { format!(" --context {context}") };
        self.status = "Bash interativo requer terminal externo; use o comando abaixo.".into();
        self.output_title = "Bash no Pod".into();
        self.output = format!(
            "kubectl exec -it{context_flag} -n {namespace} {pod} -- bash\n\nExecute em um terminal separado."
        );
    }

    fn show_port_forward(&mut self, ui: &mut egui::Ui) {
        card(
            ui,
            "Port Forward",
            Some("Cada encaminhamento usa uma porta local exclusiva."),
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Porta local");
                    ui.add(egui::TextEdit::singleline(&mut self.local_port).desired_width(75.0));
                    ui.label("Porta remota");
                    ui.add(egui::TextEdit::singleline(&mut self.remote_port).desired_width(75.0));
                });
                ui.horizontal_wrapped(|ui| {
                    if primary_button(ui, true, "Iniciar Port Forward do Pod").clicked() {
                        let pod = self.pod.trimmed().to_string();
                        if pod.is_empty() {
                            self.status = "Selecione um Pod.".into();
                        } else {
                            self.start_port_forward(pod);
                        }
                    }
                    if primary_button(ui, true, "Iniciar Port Forward do Service").clicked() {
                        let service = self.service.trimmed().to_string();
                        if service.is_empty() {
                            self.status = "Selecione um Service.".into();
                        } else {
                            self.start_port_forward(format!("svc/{service}"));
                        }
                    }
                });

                if !self.port_forwards.is_empty() {
                    ui.add_space(6.0);
                    self.show_active_port_forwards(ui);
                }
            },
        );
    }

    fn show_active_port_forwards(&mut self, ui: &mut egui::Ui) {
        let p = palette_of(ui);
        egui::Frame::new()
            .fill(p.success_soft)
            .stroke(egui::Stroke::new(1.0, p.success))
            .corner_radius(egui::CornerRadius::same(10))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                let mut stop_all = false;

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("●").size(16.0).color(p.success));
                    ui.label(
                        egui::RichText::new(format!("{} port forward(s) ativo(s)", self.port_forwards.len()))
                            .strong()
                            .color(p.success),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if danger_button(ui, "■ Parar todos").clicked() {
                            stop_all = true;
                        }
                    });
                });
                ui.add_space(4.0);

                let mut stop_index: Option<usize> = None;

                for (index, pf) in self.port_forwards.iter().enumerate() {
                    egui::Frame::new()
                        .fill(p.surface)
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("localhost:{} → {}", pf.local_port, pf.remote_port))
                                        .strong()
                                        .monospace(),
                                );
                                ui.label(egui::RichText::new(format!("{} · {}", pf.target, pf.namespace)).color(p.muted));
                                if pf.backend.is_simulated() {
                                    ui.label(egui::RichText::new("MOCK").small().strong().color(p.accent_dark));
                                }
                                if danger_button(ui, &format!("■ Parar #{}", index + 1)).clicked() {
                                    stop_index = Some(index);
                                }
                            });
                        });
                }

                if stop_all {
                    self.stop_all_port_forwards();
                } else if let Some(index) = stop_index {
                    self.stop_port_forward(index);
                }
            });
    }
}
