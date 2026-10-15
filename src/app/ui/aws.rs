//! Aba "AWS & EKS".

use super::widgets::field_combo;
use crate::action::Action;
use crate::app::KubectlUtilApp;
use eframe::egui;

impl KubectlUtilApp {
    pub(super) fn show_aws(&mut self, ui: &mut egui::Ui) {
        ui.heading("AWS & EKS");
        ui.horizontal_wrapped(|ui| {
            field_combo(ui, "Perfil", &mut self.profile, 250.0);
            if ui.button("Carregar perfis").clicked() {
                self.load_profiles();
            }
            ui.add_space(10.0);
            if ui
                .add_enabled(!self.busy, egui::Button::new("Login AWS SSO"))
                .on_hover_text("Executa `aws sso login` e aguarda a autorização no navegador (até 5 minutos).")
                .clicked()
            {
                self.perform_login();
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Região");
            ui.add(egui::TextEdit::singleline(&mut self.region).desired_width(130.0));
            ui.label("Cluster EKS");
            ui.add(egui::TextEdit::singleline(&mut self.cluster).desired_width(220.0));
        });
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("AWS configure list").clicked() {
                let args = self.aws_args(&["configure", "list"], false);
                self.aws(Action::AwsConfigure, args);
            }
            if ui.button("Identidade STS").clicked() {
                let args = self.aws_args(&["sts", "get-caller-identity"], false);
                self.aws(Action::AwsIdentity, args);
            }
            if ui.button("Listar clusters EKS").clicked() {
                let args = self.aws_args(&["eks", "list-clusters"], true);
                self.aws(Action::EksClusters, args);
            }
            if ui.button("Atualizar kubeconfig").clicked() {
                let cluster = self.cluster.trim().to_string();
                if cluster.is_empty() {
                    self.status = "Informe o nome do cluster EKS.".into();
                } else {
                    let args = self.aws_args(&["eks", "update-kubeconfig", "--name", &cluster], true);
                    self.aws(Action::UpdateKubeconfig, args);
                }
            }
        });

        ui.label("Prefere autenticar fora do aplicativo? Execute em um terminal:");
        ui.horizontal(|ui| {
            let sso = format!("aws sso login --profile {}", self.profile.trimmed());
            ui.add(egui::TextEdit::singleline(&mut sso.clone()).desired_width(390.0).interactive(false));
            if ui.button("Copiar comando SSO").clicked() {
                ui.ctx().copy_text(sso);
                self.status = "Comando SSO copiado.".into();
            }
        });
        ui.label(
            egui::RichText::new(
                "Durante o login pelo botão acima as demais ações ficam em espera; use Cancelar no painel de saída para interromper.",
            )
            .small()
            .color(egui::Color32::GRAY),
        );
    }
}
