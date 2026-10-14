use eframe::egui;
use super::*;

impl eframe::App for KubectlUtilApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Evita que áreas não cobertas pelos painéis fiquem pretas ao alternar o Mock.
        let screen_rect = ctx.screen_rect();
        let background = ctx.style().visuals.panel_fill;
        ctx.layer_painter(egui::LayerId::background()).rect_filled(
            screen_rect,
            0.0,
            background,
        );
        while let Ok(result) = self.rx.try_recv() {
            self.busy = false;
            self.status = if result.success { "Concluído".into() } else { "Com erro — veja a saída".into() };
            if result.title == "Revelar SecureString" || result.title == "Revelar parâmetro" {
                // Never put parameter values into the general output panel or operation history.
                self.revealed_parameter_value = if result.success { result.output.clone() } else { String::new() };
                self.parameter_value_visible = result.success;
                self.output_title = "Parameter Store — resultado seguro".into();
                self.output = if result.success { "Valor carregado na área segura do Parameter Store. O conteúdo não foi adicionado ao histórico nem ao log geral.".into() } else { result.output.clone() };
                self.history.push(format!("{} | consulta de valor de parâmetro | {}", timestamp(), if result.success { "OK" } else { "ERRO" }));
                if self.history.len() > 200 { self.history.remove(0); }
                continue;
            }
            self.output_title = result.title.clone();
            self.output = result.output.clone();
            self.history.push(format!("{} | {} | {}", timestamp(), result.title, if result.success { "OK" } else { "ERRO" }));
            if self.history.len() > 200 { self.history.remove(0); }
            if result.title == "Explorador de Pods" { self.explorer_output = result.output.clone(); }
            if result.title == "Diagnóstico de Pods" {
                self.output = diagnose_pods(&result.output);
                self.explorer_output = result.output.clone();
            }

            let lines: Vec<String> = result.output.lines().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect();
            match result.title.as_str() {
                "Contextos Kubernetes" => {
                    self.contexts = lines;
                    if self.context.is_empty() {
                        self.context = self.output.lines().next().unwrap_or("").trim().to_string();
                    }
                }
                "Namespaces" => {
                    self.namespaces = lines;
                    if !self.namespaces.contains(&self.namespace) { self.namespace.clear(); }
                }
                "Pods" => {
                    self.pods = lines;
                    self.pod.clear();
                }
                "Services" => {
                    self.services = lines;
                    self.service.clear();
                }
                "Perfis AWS" => {
                    self.profiles = lines;
                    if !self.profiles.contains(&self.profile) && !self.profiles.is_empty() {
                        self.profile = self.profiles[0].clone();
                    }
                }
                "Listar parâmetros SSM" => {
                    if !result.success && (result.output.contains("UnrecognizedClientException") || result.output.to_lowercase().contains("security token included in the request is invalid")) {
                        self.status = format!("Credenciais AWS inválidas ou expiradas para o perfil '{}'. Atualize a sessão e tente novamente. Se usa AWS SSO, execute `aws sso login --profile {}` no terminal; se usa credenciais temporárias, renove também AWS_SESSION_TOKEN. Confira o perfil e a região.", self.profile, self.profile);
                    } else if result.success {
                        let parsed = serde_json::from_str::<serde_json::Value>(&result.output);
                        match parsed {
                            Ok(json) => {
                                let entries: Vec<ParameterEntry> = json["Parameters"].as_array().into_iter().flatten().map(|p| ParameterEntry {
                                    name: p["Name"].as_str().unwrap_or("").to_string(),
                                    kind: p["Type"].as_str().unwrap_or("Unknown").to_string(),
                                    modified: p["LastModifiedDate"].as_str().unwrap_or("—").to_string(),
                                }).filter(|p| !p.name.is_empty()).collect();
                                let key = self.pending_parameter_key.clone();
                                self.parameter_cache.insert(key.clone(), entries.clone());
                                self.loaded_parameter_key = key;
                                self.parameters = entries;
                                self.selected_parameter.clear();
                                self.revealed_parameter_value.clear();
                                self.parameter_value_visible = false;
                                self.status = format!("{} parâmetros carregados. Valores SecureString não foram descriptografados.", self.parameters.len());
                            }
                            Err(e) => self.status = format!("Não foi possível interpretar o JSON do Parameter Store: {e}"),
                        }
                    }
                }
                _ => {}
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(150));

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
                ui.label(egui::RichText::new("Nenhum comando real será executado.").small().color(egui::Color32::DARK_GREEN));
                egui::ComboBox::from_label("Teste")
                    .selected_text(&self.mock_scenario)
                    .width(180.0)
                    .show_ui(ui, |ui| {
                        for scenario in ["Sucesso (dados padrão)", "Erro", "Timeout", "Lista vazia", "Sem permissões"] {
                            ui.selectable_value(&mut self.mock_scenario, scenario.to_string(), scenario);
                        }
                    });
            }
            if was_mock != self.mock_mode {
                self.status = if self.mock_mode { "Modo Mock ativado: operações externas bloqueadas." } else { "Modo real ativado: ações podem executar comandos AWS CLI/kubectl." }.into();
                if self.mock_mode {
                    for pf in &mut self.port_forwards {
                        if let Some(child) = pf.child.as_mut() {
                            let _ = child.kill();
                            let _ = child.wait();
                        }
                    }
                    self.port_forwards.clear();
                    self.refresh_contexts();
                } else {
                    self.port_forwards.retain(|pf| pf.child.is_some());
                }
            }
            ui.add_space(14.0);
            self.nav_button(ui, Section::Connection);
            self.nav_button(ui, Section::Overview);
            self.nav_button(ui, Section::Cluster);
            self.nav_button(ui, Section::Network);
            self.nav_button(ui, Section::Aws);
            self.nav_button(ui, Section::Parameters);
            ui.add_space(20.0);
            ui.separator();
            ui.label(egui::RichText::new("As consultas são feitas sob demanda.").small().color(egui::Color32::GRAY));
        });

        egui::TopBottomPanel::bottom("output_panel")
            .resizable(true)
            .default_height(350.0)
            .min_height(200.0)
            .show(ctx, |ui| self.show_output_panel(ui));

        // Garante o fundo do painel central conforme o tema do egui.
        let central_fill = ctx.style().visuals.panel_fill;
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).fill(central_fill))
            .show(ctx, |ui| {
            ui.add_space(6.0);
            ui.heading(self.section.label());
            ui.label("Execute somente as consultas necessárias.");
            ui.add_space(12.0);

            
            if self.section != Section::Connection {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!(
                        "Contexto: {}",
                        if self.context.is_empty() {
                            "não selecionado"
                        } else {
                            &self.context
                        }
                    ));

                    ui.separator();

                    ui.label(format!(
                        "Namespace: {}",
                        if self.namespace.is_empty() {
                            "não selecionado"
                        } else {
                            &self.namespace
                        }
                    ));

                    if ui.button("Abrir conexão").clicked() {
                        self.section = Section::Connection;
                    }
                });

                ui.separator();
            }

            
            if self.section == Section::Connection {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.heading("Conexão Kubernetes");

                    ui.horizontal_wrapped(|ui| {
                        // Contexto
                        Self::field_combo(
                            ui,
                            "Contexto",
                            &mut self.context,
                            &mut self.context_filter,
                            &self.contexts,
                            300.0,
                        );

                        if ui.button("Atualizar").clicked() {
                            self.refresh_contexts();
                        }

                        if ui.button("Usar contexto").clicked() {
                            let context = self.context.clone();

                            if !context.is_empty() {
                                self.launch(
                                    "Alterar contexto",
                                    "kubectl",
                                    vec![
                                        "config".into(),
                                        "use-context".into(),
                                        context,
                                    ],
                                );
                            } else {
                                self.status = "Selecione um contexto.".into();
                            }
                        }
                    });

                    ui.horizontal_wrapped(|ui| {
                        // Namespace: mais estreito
                        Self::field_combo(
                            ui,
                            "Namespace",
                            &mut self.namespace,
                            &mut self.namespace_filter,
                            &self.namespaces,
                            180.0,
                        );

                        if ui.button("Carregar namespaces").clicked() {
                            self.refresh_namespaces();
                        }

                        if ui.button("Pods").clicked() {
                            self.refresh_workloads();
                        }

                        if ui.button("Services").clicked() {
                            self.load_services();
                        }
                    });

                    ui.horizontal_wrapped(|ui| {
                        ui.label("Pod");

                        egui::ComboBox::from_id_salt("pod")
                            .close_behavior(egui::PopupCloseBehavior::IgnoreClicks)
                            .selected_text(if self.pod.is_empty() { "Selecionar Pod…" } else { &self.pod })
                            .width(300.0).height(400.0)
                            .show_ui(ui, |ui| {
                                ui.set_min_width(300.0);

                                // Pesquisa fixa no topo
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.pod_filter)
                                        .hint_text("Pesquisar Pods...")
                                        .desired_width(280.0),
                                );

                                ui.separator();

                                // Apenas a lista de opções terá rolagem
                                egui::ScrollArea::vertical()
                                    .id_salt("pod_options_scroll")
                                    .max_height(200.0)
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        let needle = self.pod_filter.trim().to_lowercase();

                                        let matches: Vec<String> = self
                                            .pods
                                            .iter()
                                            .filter(|pod| pod.to_lowercase().contains(&needle))
                                            .cloned()
                                            .collect();

                                        if matches.is_empty() {
                                            ui.label("Nenhum Pod encontrado.");
                                        }

                                        for item in matches {
                                            if ui
                                                .add_sized(
                                                    [280.0, 34.0],
                                                    egui::Button::new(&item)
                                                        .selected(self.pod == item),
                                                )
                                                .clicked()
                                            {
                                                self.pod = item;
                                                ui.close_menu();
                                            }
                                        }
                                    });
                            });

                        ui.label("Service");
                        egui::ComboBox::from_id_salt("service")
                        .close_behavior(egui::PopupCloseBehavior::IgnoreClicks)
                        .selected_text(if self.service.is_empty() {
                            "Selecionar Service…"
                        } else {
                            &self.service
                        })
                        .width(300.0)
                        .show_ui(ui, |ui| {
                            ui.set_min_width(300.0);

                            // Pesquisa fixa no topo
                            ui.add(
                                egui::TextEdit::singleline(&mut self.service_filter)
                                    .hint_text("Pesquisar Services...")
                                    .desired_width(280.0),
                            );

                            ui.separator();

                            // Apenas a lista de opções terá rolagem
                            egui::ScrollArea::vertical()
                                .id_salt("service_options_scroll")
                                .max_height(200.0)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    let needle = self.service_filter.trim().to_lowercase();

                                    let matches: Vec<String> = self
                                        .services
                                        .iter()
                                        .filter(|service| service.to_lowercase().contains(&needle))
                                        .cloned()
                                        .collect();

                                    if matches.is_empty() {
                                        ui.label("Nenhum Service encontrado.");
                                    }

                                    for item in matches {
                                        if ui
                                            .add_sized(
                                                [280.0, 34.0],
                                                egui::Button::new(&item)
                                                    .selected(self.service == item),
                                            )
                                            .clicked()
                                        {
                                            self.service = item;
                                            ui.close_menu();
                                        }
                                    }
                                });
                        });
                    });
                });

                ui.add_space(10.0);
            }


            ui.add_space(10.0);
            match self.section {
                Section::Overview => {
                    ui.label("Acesso rápido");
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Ver cluster-info").clicked() { self.k("Informações do cluster", &["cluster-info"]); }
                        if ui.button("Ver nodes").clicked() { self.k("Nodes", &["get", "nodes", "-o", "wide"]); }
                        if ui.button("Eventos").clicked() {
                            let ns = self.current_namespace();
                            if !ns.is_empty() { self.output_action("Eventos", vec!["get".into(),"events".into(),"-n".into(),ns,"--sort-by=.metadata.creationTimestamp".into()]); }
                            else { self.status = "Selecione um namespace.".into(); }
                        }
                    });
                }
                Section::Connection => {
                    ui.heading("Explorador e diagnóstico");
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Carregar/atualizar Pods (1 consulta)").clicked() { self.load_pod_explorer(); }
                        if ui.button("Diagnóstico rápido (1 consulta)").clicked() { self.run_pod_diagnostics(); }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Filtrar lista carregada:");
                        ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("nome, status, node…").desired_width(260.0));
                    });
                    if !self.explorer_output.is_empty() {
                        egui::ScrollArea::vertical()
                                .id_salt("workloads_pods_scroll_area")
                                .max_height(150.0)
                                .show(ui, |ui| {
                            for line in self.explorer_output.lines().filter(|l| l.to_lowercase().contains(&self.filter.to_lowercase())) {
                                ui.monospace(line);
                            }
                        });
                    }
                    ui.separator();
                    ui.heading("Troubleshooting");
                    ui.horizontal_wrapped(|ui| {
                        let ns = self.current_namespace();
                        let pod = self.current_pod();
                        if ui.button("Logs (últimas 100 linhas)").clicked() {
                            if !ns.is_empty() && !pod.is_empty() { self.output_action("Logs do Pod", vec!["logs".into(),"-n".into(),ns,pod,"--tail=100".into()]); }
                            else { self.status = "Selecione namespace e Pod.".into(); }
                        }
                        let ns = self.current_namespace(); let pod = self.current_pod();
                        if ui.button("Logs anteriores (--previous)").clicked() {
                            if !ns.is_empty() && !pod.is_empty() { self.output_action("Logs anteriores do Pod", vec!["logs".into(), "-n".into(), ns, pod, "--previous".into(), "--tail=100".into()]); }
                            else { self.status = "Selecione namespace e Pod.".into(); }
                        }
                        let ns = self.current_namespace(); let pod = self.current_pod();
                        if ui.button("Descrever Pod").clicked() {
                            if !ns.is_empty() && !pod.is_empty() { self.output_action("Describe Pod", vec!["describe".into(),"pod".into(),"-n".into(),ns,pod]); }
                            else { self.status = "Selecione namespace e Pod.".into(); }
                        }
                        let ns = self.current_namespace(); let pod = self.current_pod();
                        if ui.button("YAML do Pod").clicked() {
                            if !ns.is_empty() && !pod.is_empty() { self.output_action("YAML do Pod", vec!["get".into(),"pod".into(),"-n".into(),ns,pod,"-o".into(),"yaml".into()]); }
                            else { self.status = "Selecione namespace e Pod.".into(); }
                        }
                        let ns = self.current_namespace();
                        if ui.button("Top Pods").clicked() {
                            if !ns.is_empty() { self.output_action("Top Pods", vec!["top".into(),"pods".into(),"-n".into(),ns]); }
                            else { self.status = "Selecione namespace.".into(); }
                        }
                        let ns = self.current_namespace(); let pod = self.current_pod();
                        if ui.button("Bash no Pod").clicked() {
                            if !ns.is_empty() && !pod.is_empty() {
                                self.status = "Bash interativo requer terminal externo; use o comando abaixo.".into();
                                self.output = format!("kubectl exec -it -n {ns} {pod} -- bash\n\nExecute em um terminal separado.");
                            } else { self.status = "Selecione namespace e Pod.".into(); }
                        }
                    });
                    ui.separator();
                    ui.heading("Port Forward");
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Porta local"); ui.add(egui::TextEdit::singleline(&mut self.local_port).desired_width(75.0));
                        ui.label("Porta remota"); ui.add(egui::TextEdit::singleline(&mut self.remote_port).desired_width(75.0));
                    });
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Iniciar Port Forward do Pod").clicked() {
                            let (ns,pod,lp,rp) = (self.current_namespace(),self.current_pod(),self.local_port.clone(),self.remote_port.clone());
                            if !ns.is_empty() && !pod.is_empty() && valid_port(&lp) && valid_port(&rp) {
                                self.start_port_forward(pod);
                            } else { self.status = "Selecione namespace/Pod e informe portas válidas (1–65535).".into(); }
                        }
                        if ui.button("Iniciar Port Forward do Service").clicked() {
                            let (ns,svc,lp,rp) = (self.current_namespace(),self.current_service(),self.local_port.clone(),self.remote_port.clone());
                            if !ns.is_empty() && !svc.is_empty() && valid_port(&lp) && valid_port(&rp) {
                                self.start_port_forward(format!("svc/{svc}"));
                            } else { self.status = "Selecione namespace/Service e informe portas válidas.".into(); }
                        }
                    });
                    egui::Frame::group(ui.style())
                        .fill(egui::Color32::from_rgb(18, 75, 45))
                        .stroke(egui::Stroke::new(
                            2.0_f32,
                            egui::Color32::from_rgb(20, 20, 20),
                        ))
                        .inner_margin(egui::Margin::same(10))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("●")
                                        .size(20.0)
                                        .strong()
                                        .color(egui::Color32::from_rgb(50, 255, 120)),
                                );

                                ui.vertical(|ui| {
                                    ui.heading(
                                        egui::RichText::new(format!(
                                            "{} PORT FORWARD(S) ATIVO(S)",
                                            self.port_forwards.len()
                                        ))
                                        .strong()
                                        .color(egui::Color32::LIGHT_GREEN),
                                    );

                                    ui.label("Conexões em execução — localhost");
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
                                                    "{}:{} → {} ({})",
                                                    "localhost",
                                                    pf.local_port,
                                                    pf.remote_port,
                                                    pf.target
                                                ))
                                                .strong()
                                                .color(egui::Color32::WHITE),
                                            );

                                            ui.label(format!("Namespace: {}", pf.namespace));

                                            if ui.button(format!("■ Parar #{}", index + 1)).clicked() {
                                                stop_index = Some(index);
                                            }
                                        });
                                    });
                            }

                            if let Some(index) = stop_index {
                                self.stop_port_forward(index);
                            }
                        });
                }
                Section::Cluster => {
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Nodes (wide)").clicked() { self.k("Nodes", &["get","nodes","-o","wide"]); }
                        if ui.button("Top Nodes").clicked() { self.k("Top Nodes", &["top","nodes"]); }
                        if ui.button("Workloads").clicked() {
                            let ns=self.current_namespace();
                            if !ns.is_empty() { self.output_action("Workloads", vec!["get".into(),"deployments,daemonsets,statefulsets,jobs".into(),"-n".into(),ns]); }
                            else { self.status="Selecione namespace.".into(); }
                        }
                        if ui.button("Cluster info").clicked() { self.k("Informações do cluster", &["cluster-info"]); }
                        if ui.button("Eventos").clicked() {
                            let ns=self.current_namespace();
                            if !ns.is_empty() { self.output_action("Eventos", vec!["get".into(),"events".into(),"-n".into(),ns,"--sort-by=.metadata.creationTimestamp".into()]); }
                            else { self.status="Selecione namespace.".into(); }
                        }
                        if ui.button("Services").clicked() {
                            let ns=self.current_namespace();
                            if !ns.is_empty() { self.output_action("Services", vec!["get".into(),"svc".into(),"-n".into(),ns]); }
                            else { self.status="Selecione namespace.".into(); }
                        }
                        if ui.button("Imagens dos Pods").clicked() {
                            let ns=self.current_namespace();
                            if !ns.is_empty() { self.output_action("Imagens", vec!["get".into(),"pods".into(),"-n".into(),ns,"-o".into(),"jsonpath={range .items[*]}{.metadata.name}{\"\\t\"}{range .spec.containers[*]}{.image}{\" \"}{end}{\"\\n\"}{end}".into()]); }
                            else { self.status="Selecione namespace.".into(); }
                        }
                    });
                }
                Section::Network => {
                    ui.horizontal_wrapped(|ui| {
                        for (label, args) in [
                            ("Ingresses", vec!["get","ingress"]),
                            ("Detalhes de Ingress", vec!["describe","ingress"]),
                            ("Endpoints", vec!["get","endpoints"]),
                            ("PVC e PV", vec!["get","pvc,pv"]),
                            ("ConfigMaps", vec!["get","configmaps"]),
                            ("Backups Velero", vec!["get","backups"]),
                        ] {
                            if ui.button(label).clicked() {
                                let ns=self.current_namespace();
                                if !ns.is_empty() {
                                    let mut a:Vec<String> = args.iter().map(|s|s.to_string()).collect();
                                    a.extend(["-n".into(),ns]);
                                    self.output_action(label, a);
                                } else { self.status="Selecione namespace.".into(); }
                            }
                        }
                    });
                }
                Section::Aws => {
                    ui.heading("AWS & EKS");
                    ui.horizontal_wrapped(|ui| {
                        Self::field_combo(ui, "Perfil", &mut self.profile, &mut self.profile_filter, &self.profiles, 250.0);
                        if ui.button("Carregar perfis").clicked() { self.load_profiles(); }
                        ui.add_space(10.0);
                        if ui.button("Login AWS SSO").clicked() { self.perform_login(); }
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Região");
                        ui.add(egui::TextEdit::singleline(&mut self.region).desired_width(130.0));
                        ui.label("Cluster EKS");
                        ui.add(egui::TextEdit::singleline(&mut self.cluster).desired_width(220.0));
                    });
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        let p=self.profile.clone();
                        if ui.button("AWS configure list").clicked() {
                            self.aws("Configuração AWS", vec!["configure".into(),"list".into(),"--profile".into(),p]);
                        }
                        let p=self.profile.clone();
                        if ui.button("Identidade STS").clicked() {
                            self.aws("Identidade STS", vec!["sts".into(),"get-caller-identity".into(),"--profile".into(),p]);
                        }
                        let p=self.profile.clone(); let r=self.region.clone();
                        if ui.button("Listar clusters EKS").clicked() {
                            self.aws("Clusters EKS", vec!["eks".into(),"list-clusters".into(),"--profile".into(),p,"--region".into(),r]);
                        }
                        let p=self.profile.clone(); let r=self.region.clone(); let c=self.cluster.clone();
                        if ui.button("Atualizar kubeconfig").clicked() {
                            if c.trim().is_empty() { self.status="Informe o nome do cluster EKS.".into(); }
                            else { self.aws("Atualizar kubeconfig", vec!["eks".into(),"update-kubeconfig".into(),"--name".into(),c,"--profile".into(),p,"--region".into(),r]); }
                        }
                    });
                    ui.label("Login SSO: para manter a autenticação interativa, abra um terminal e execute:");
                    ui.horizontal(|ui| {
                        let sso = format!("aws sso login --profile {}", self.profile);
                        ui.add(egui::TextEdit::singleline(&mut sso.clone()).desired_width(390.0).interactive(false));
                        if ui.button("Copiar comando SSO").clicked() { ui.ctx().copy_text(sso); self.status="Comando SSO copiado.".into(); }
                    });
                    ui.label(egui::RichText::new("O login SSO não é iniciado dentro da interface nesta versão.").small().color(egui::Color32::GRAY));
                }
                Section::Parameters => {
                    ui.heading("AWS Systems Manager — Parameter Store");
                    ui.label("Somente leitura. O aplicativo não cria, altera nem exclui parâmetros.");
                    ui.label(format!("Perfil: {}    Região: {}", self.profile, self.region));
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Caminho");
                        ui.add(egui::TextEdit::singleline(&mut self.parameter_path).desired_width(300.0).hint_text("/producao/minha-aplicacao/"));
                        if ui.add_enabled(!self.busy, egui::Button::new("Carregar (usar cache)" )).clicked() { self.load_parameters(false); }
                        if ui.add_enabled(!self.busy, egui::Button::new("Atualizar da AWS")).clicked() { self.load_parameters(true); }
                        if ui.add_enabled(!self.busy, egui::Button::new("Limpar cache")).clicked() {self.clear_parameter_cache();}
                    });
                    ui.horizontal(|ui| {
                        ui.label("Filtrar nomes");
                        ui.add(egui::TextEdit::singleline(&mut self.parameter_filter).desired_width(280.0).hint_text("Digite parte do nome"));
                        ui.label(format!("{} parâmetros em memória", self.parameters.len()));
                    });
                        egui::ScrollArea::vertical()
                            .id_salt("parameter_store_scroll_area")
                            .max_height(220.0)
                            .show(ui, |ui| {
                        egui::Grid::new("ssm_parameters_grid").striped(true).show(ui, |ui| {
                            ui.strong("Nome"); ui.strong("Tipo"); ui.strong("Última modificação"); ui.end_row();
                            let filter = self.parameter_filter.to_lowercase();
                            let visible_parameters: Vec<ParameterEntry> = self.parameters.iter()
                                .filter(|p| p.name.to_lowercase().contains(&filter)).cloned().collect();
                            for p in visible_parameters {
                                if ui.selectable_label(self.selected_parameter == p.name, &p.name).clicked() {
                                    self.selected_parameter = p.name.clone();
                                    self.revealed_parameter_value.clear();
                                    self.parameter_value_visible = false;
                                }
                                ui.label(&p.kind);
                                ui.label(&p.modified);
                                ui.end_row();
                            }
                        });
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label(if self.selected_parameter.is_empty() { "Nenhum parâmetro selecionado".to_string() } else { self.selected_parameter.clone() });
                        if ui.add_enabled(!self.busy && !self.selected_parameter.is_empty(), egui::Button::new("Consultar valor")).clicked() { self.reveal_selected_parameter(); }
                        if self.parameter_value_visible {
                            if ui.button("Ocultar valor").clicked() { self.parameter_value_visible = false; }
                            if ui.button("Copiar valor").clicked() { ui.ctx().copy_text(self.revealed_parameter_value.clone()); self.status = "Valor copiado para a área de transferência; limpe-a se necessário.".into(); }
                        }
                    });
                    if self.parameter_value_visible {
                        ui.label("Valor consultado sob demanda. Não é salvo no histórico nem no arquivo de log geral.");
                        ui.add(egui::TextEdit::multiline(&mut self.revealed_parameter_value).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY).desired_rows(3).interactive(false));
                    } else if !self.selected_parameter.is_empty() {
                        ui.label("Valor oculto. Clique em ‘Consultar valor’ para fazer uma consulta AWS individual.");
                    }
                    ui.label(egui::RichText::new("Os resultados ficam em cache em memória por perfil, região e caminho até atualizar, trocar a consulta ou fechar o aplicativo. SecureString só é descriptografado quando você solicita o valor.").small().color(egui::Color32::GRAY));
                }
            }
            ui.add_space(8.0);
            egui::CollapsingHeader::new(format!("Histórico local de operações ({} entradas)", self.history.len())).default_open(false).show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Copiar histórico").clicked() { ui.ctx().copy_text(self.history.join("\n")); self.status = "Histórico copiado.".into(); }
                    if ui.button("Limpar histórico").clicked() { self.history.clear(); }
                });
                egui::ScrollArea::vertical()
                    .id_salt("history_scroll_area")
                    .max_height(100.0)
                    .show(ui, |ui| {
                    for item in self.history.iter().rev().take(50) { ui.monospace(item); }
                });
            });
        });
    }
}

