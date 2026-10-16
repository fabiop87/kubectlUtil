//! Componentes de interface reutilizáveis (cartão, chip, botões, combo).

use crate::app::Selectable;
use crate::theme::{self, Palette};
use eframe::egui;

pub(super) fn palette_of(ui: &egui::Ui) -> Palette {
    theme::palette(ui.visuals().dark_mode)
}

/// Moldura de cartão: fundo de superfície, borda fina e cantos arredondados.
pub(super) fn card_frame(p: &Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(p.surface)
        .stroke(egui::Stroke::new(1.0_f32, p.border))
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(egui::Margin::same(16))
}

/// Cartão com título (e subtítulo opcional) que ocupa toda a largura disponível.
pub(super) fn card<R>(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: Option<&str>,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let p = palette_of(ui);
    card_frame(&p)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(egui::RichText::new(title).size(16.0).strong());
            if let Some(subtitle) = subtitle {
                ui.label(egui::RichText::new(subtitle).small().color(p.muted));
            }
            ui.add_space(6.0);
            add_contents(ui)
        })
        .inner
}

/// Pílula informativa: `rótulo valor`. `set = false` esmaece o valor (ex.: "não selecionado").
pub(super) fn chip(ui: &mut egui::Ui, label: &str, value: &str, set: bool) {
    let p = palette_of(ui);
    let (fill, value_color) = if set { (p.accent_soft, p.accent_dark) } else { (p.surface_alt, p.muted) };
    egui::Frame::new()
        .fill(fill)
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.label(egui::RichText::new(label).small().color(p.muted));
                ui.label(egui::RichText::new(value).strong().color(value_color));
            });
        });
}

/// Botão de ação principal (cor de destaque).
pub(super) fn primary_button(ui: &mut egui::Ui, enabled: bool, text: &str) -> egui::Response {
    let p = palette_of(ui);
    let button = egui::Button::new(egui::RichText::new(text).strong().color(p.on_accent)).fill(p.accent);
    ui.add_enabled(enabled, button)
}

/// Botão para ações que interrompem algo (parar, cancelar).
pub(super) fn danger_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let p = palette_of(ui);
    let button = egui::Button::new(egui::RichText::new(text).strong().color(p.on_danger)).fill(p.danger);
    ui.add(button)
}

/// Combo com campo de pesquisa fixo no topo e lista rolável de opções.
pub(super) fn field_combo(ui: &mut egui::Ui, label: &str, selectable: &mut Selectable, width: f32) {
    let muted = palette_of(ui).muted;
    ui.label(egui::RichText::new(label).color(muted));

    // Desestruturar permite emprestar cada campo separadamente dentro dos closures.
    let Selectable { value, items, filter } = selectable;

    egui::ComboBox::from_id_salt(label)
        .close_behavior(egui::PopupCloseBehavior::IgnoreClicks)
        .selected_text(if value.is_empty() { "Selecionar…" } else { value.as_str() })
        .width(width)
        .show_ui(ui, |ui| {
            ui.set_min_width(width);

            // Pesquisa fixa no topo
            ui.add(
                egui::TextEdit::singleline(filter)
                    .hint_text("Pesquisar / filtrar...")
                    .desired_width((width - 20.0).max(100.0)),
            );

            ui.separator();

            // Apenas a lista de opções terá rolagem
            egui::ScrollArea::vertical()
                .id_salt(("field_combo_options", label))
                .max_height(200.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let needle = filter.trim().to_lowercase();
                    let mut found = false;

                    for item in items.iter().filter(|item| item.to_lowercase().contains(&needle)) {
                        found = true;

                        if ui
                            .add_sized(
                                [(width - 20.0).max(100.0), 34.0],
                                egui::Button::new(item).selected(*value == *item),
                            )
                            .clicked()
                        {
                            *value = item.clone();
                            ui.close_menu();
                        }
                    }

                    if !found {
                        ui.label("Nenhum resultado para esse filtro.");
                    }
                });
        });
}
