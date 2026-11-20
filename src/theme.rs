//! Tokens visuais: console escuro e plano. A cor só aparece onde carrega significado
//! (verde = ativo, âmbar = simulado/segredo, vermelho = produção/destrutivo).
use gpui_kit::{
    component::{Theme, ThemeMode},
    *,
};

pub const BG: u32 = 0x11171A; // janela
pub const SURFACE: u32 = 0x182024; // painéis e navegação
pub const RAISED: u32 = 0x202B30; // hover, seleção
pub const WELL: u32 = 0x0C1114; // console, listas, valores
pub const LINE: u32 = 0x2F3C42; // bordas e divisores
pub const TEXT: u32 = 0xE8E4D8;
pub const MUTED: u32 = 0x98A5A3;
pub const ACCENT: u32 = 0x4FD1B2;
pub const WARN: u32 = 0xF0B04A;
pub const DANGER: u32 = 0xF06A5E;

pub const MONO: &str = "Consolas";

pub fn install(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
    Theme::update(cx, |theme| {
        theme.font_family = "Bahnschrift".into();
        theme.mono_font_family = MONO.into();
        theme.font_size = px(14.);
        theme.radius = px(6.);
        theme.radius_lg = px(8.);
        theme.colors.background = rgb(BG).into();
        theme.colors.foreground = rgb(TEXT).into();
        theme.colors.muted_foreground = rgb(MUTED).into();
        theme.colors.border = rgb(LINE).into();
        theme.colors.input = rgb(0x141C20).into();
        theme.colors.popover = rgb(0x1E282D).into();
        theme.colors.popover_foreground = rgb(TEXT).into();
        theme.colors.button = rgb(0x232F35).into();
        theme.colors.button_hover = rgb(0x2C3A41).into();
        theme.colors.button_foreground = rgb(TEXT).into();
        theme.colors.button_active = rgb(0x34444C).into();
        theme.colors.primary = rgb(ACCENT).into();
        theme.colors.primary_foreground = rgb(BG).into();
        theme.colors.button_primary = rgb(ACCENT).into();
        theme.colors.button_primary_foreground = rgb(BG).into();
        theme.colors.button_primary_hover = rgb(0x73E0C5).into();
        theme.colors.button_primary_active = rgb(0x3BB99B).into();
        theme.colors.accent = rgb(0x2C3A41).into();
        theme.colors.accent_foreground = rgb(TEXT).into();
        theme.colors.ring = rgb(ACCENT).into();
        theme.colors.selection = rgba(0x4FD1B255).into();
        theme.colors.list_active = rgb(0x24323A).into();
        theme.colors.list_active_border = rgb(ACCENT).into();
        theme.colors.list_hover = rgb(0x1E292E).into();
    });
}

/// Painel plano: uma borda, sem gradiente nem sombra.
pub fn panel() -> Div {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .rounded(px(8.))
        .border_1()
        .border_color(rgb(LINE))
        .bg(rgb(SURFACE))
}

/// Poço escuro para dados em monoespaçada (console, listas, valores).
pub fn well() -> Div {
    div()
        .p_3()
        .rounded(px(6.))
        .border_1()
        .border_color(rgb(LINE))
        .bg(rgb(WELL))
        .font_family(MONO)
}

#[cfg(test)]
mod tests {
    #[test]
    fn dark_palette_keeps_text_contrast() {
        let luminance = |hex: u32| {
            let c = |shift| {
                let v = ((hex >> shift) & 255u32) as f64 / 255.;
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * c(16) + 0.7152 * c(8) + 0.0722 * c(0)
        };
        // O fundo mais claro sobre o qual o texto aparece é RAISED.
        for foreground in [
            super::TEXT,
            super::MUTED,
            super::ACCENT,
            super::WARN,
            super::DANGER,
        ] {
            assert!(
                (luminance(foreground) + 0.05) / (luminance(super::RAISED) + 0.05) >= 4.5
            );
        }
    }
}