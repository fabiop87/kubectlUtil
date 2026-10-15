//! Componentes de interface reutilizáveis.

use crate::app::Selectable;
use eframe::egui;

/// Combo com campo de pesquisa fixo no topo e lista rolável de opções.
pub(super) fn field_combo(ui: &mut egui::Ui, label: &str, selectable: &mut Selectable, width: f32) {
    ui.label(label);

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
