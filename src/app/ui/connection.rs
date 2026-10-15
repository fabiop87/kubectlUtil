//! Aba "Conexão Kubernetes": seleção de contexto/namespace/Pod/Service,
//! explorador de Pods, troubleshooting e Port Forward.

use super::widgets::field_combo;
use crate::app::presets;
use crate::app::KubectlUtilApp;
use eframe::egui;

impl KubectlUtilApp {
    pub(super) fn show_connection(&mut self, ui: &mut egui::Ui) {
        self.show_connection_selectors(ui);
        ui.add_space(10.0);
        self.show_pod_explorer(ui);
        ui.separator();
        self.show_troubleshooting(ui);
        ui.separator();
        self.show_port_forward(ui);
    }

    fn show_connection_selectors(&mut self, ui: &mut egui::Ui) {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.heading("Conexão Kubernetes");

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
        });
    }

    fn show_pod_explorer(&mut self, ui: &mut egui::Ui) {
        ui.heading("Explorador e diagnóstico");
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
    }

    fn show_troubleshooting(&mut self, ui: &mut egui::Ui) {
        ui.heading("Troubleshooting");
        ui.horizontal_wrapped(|ui| {
            self.preset_buttons(ui, presets::POD_TOOLS);
            if ui.button("Bash no Pod").clicked() {
                self.show_exec_hint();
            }
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
        ui.heading("Port Forward");
        ui.horizontal_wrapped(|ui| {
            ui.label("Porta local");
            ui.add(egui::TextEdit::singleline(&mut self.local_port).desired_width(75.0));
            ui.label("Porta remota");
            ui.add(egui::TextEdit::singleline(&mut self.remote_port).desired_width(75.0));
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button("Iniciar Port Forward do Pod").clicked() {
                let pod = self.pod.trimmed().to_string();
                if pod.is_empty() {
                    self.status = "Selecione um Pod.".into();
                } else {
                    self.start_port_forward(pod);
                }
            }
            if ui.button("Iniciar Port Forward do Service").clicked() {
                let service = self.service.trimmed().to_string();
                if service.is_empty() {
                    self.status = "Selecione um Service.".into();
                } else {
                    self.start_port_forward(format!("svc/{service}"));
                }
            }
        });

        if !self.port_forwards.is_empty() {
            self.show_active_port_forwards(ui);
        }
    }

    fn show_active_port_forwards(&mut self, ui: &mut egui::Ui) {
        egui::Frame::group(ui.style())
            .fill(egui::Color32::from_rgb(18, 75, 45))
            .stroke(egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(20, 20, 20)))
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                let mut stop_all = false;
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("●")
                            .size(20.0)
                            .strong()
                            .color(egui::Color32::from_rgb(50, 255, 120)),
                    );

                    ui.vertical(|ui| {
                        ui.heading(
                            egui::RichText::new(format!("{} PORT FORWARD(S) ATIVO(S)", self.port_forwards.len()))
                                .strong()
                                .color(egui::Color32::LIGHT_GREEN),
                        );
                        ui.label("Conexões em execução — localhost");
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("■ Parar todos").clicked() {
                            stop_all = true;
                        }
                    });
                });

                ui.separator();

                let mut stop_index: Option<usize> = None;

                for (index, pf) in self.port_forwards.iter().enumerate() {
                    egui::Frame::group(ui.style())
                        .fill(egui::Color32::from_rgb(25, 55, 40))
                        .inner_margin(egui::Margin::same(6))
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "localhost:{} → {} ({})",
                                        pf.local_port, pf.remote_port, pf.target
                                    ))
                                    .strong()
                                    .color(egui::Color32::WHITE),
                                );

                                ui.label(format!("Namespace: {}", pf.namespace));
                                if pf.backend.is_simulated() {
                                    ui.label("[MOCK]");
                                }

                                if ui.button(format!("■ Parar #{}", index + 1)).clicked() {
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
