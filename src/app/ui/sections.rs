use super::*;
use crate::app::presets::Preset;

impl AppView {
    pub(super) fn presets(
        &self,
        title: &'static str,
        list: &'static [Preset],
        cx: &Context<Self>,
    ) -> Div {
        card(
            title,
            "Execute uma consulta para visualizar o resultado no console.",
        )
        .child(self.preset_buttons(list, cx))
    }
    pub(super) fn preset_buttons(&self, list: &'static [Preset], cx: &Context<Self>) -> Div {
        row().children(list.iter().enumerate().map(|(i, preset)| {
            Button::new(("preset", i))
                .label(preset.label)
                .disabled(self.model.busy)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.model.run_preset(preset);
                    cx.notify();
                }))
        }))
    }
}
