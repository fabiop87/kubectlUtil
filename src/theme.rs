//! Superfícies translúcidas sobre um fundo escuro; sem dependência de blur do sistema.
use gpui_kit::{
    component::{Theme, ThemeMode},
    *,
};

pub const BG: u32 = 0x100B18;
pub const TEXT: u32 = 0xF3EAFF;
pub const MUTED: u32 = 0xBDAACF;
pub const ACCENT: u32 = 0xC9A1FF;

pub fn install(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
    Theme::update(cx, |theme| {
        theme.font_family = "Segoe UI".into();
        theme.mono_font_family = "Consolas".into();
        theme.font_size = px(14.);
        theme.radius = px(10.);
        theme.radius_lg = px(18.);
        theme.colors.background = rgb(BG).into();
        theme.colors.foreground = rgb(TEXT).into();
        theme.colors.muted_foreground = rgb(MUTED).into();
        theme.colors.border = rgb(0x503B68).into();
        theme.colors.input = rgb(0x1C122B).into();
        theme.colors.popover = rgb(0x261A38).into();
        theme.colors.popover_foreground = rgb(TEXT).into();
        theme.colors.button = rgb(0x302040).into();
        theme.colors.button_hover = rgb(0x48315F).into();
        theme.colors.button_foreground = rgb(TEXT).into();
        theme.colors.primary = rgb(ACCENT).into();
        theme.colors.primary_foreground = rgb(BG).into();
        theme.colors.button_primary = rgb(ACCENT).into();
        theme.colors.button_primary_foreground = rgb(BG).into();
        theme.colors.button_primary_hover = rgb(0xDFC4FF).into();
        theme.colors.button_active = rgb(0x563B70).into();
        theme.colors.button_primary_active = rgb(0xB587ED).into();
        theme.colors.accent = rgb(0x48315F).into();
        theme.colors.accent_foreground = rgb(TEXT).into();
        theme.colors.ring = rgb(ACCENT).into();
        theme.colors.selection = rgba(0xC9A1FF55).into();
        theme.colors.list_active = rgb(0x48315F).into();
        theme.colors.list_active_border = rgb(ACCENT).into();
        theme.colors.list_hover = rgb(0x302040).into();
    });
}

pub fn glass() -> Div {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .p_5()
        .rounded(px(18.))
        .border_1()
        .border_color(rgba(0xDDC1FF30))
        .bg(linear_gradient(
            135.,
            linear_color_stop(rgba(0x4C326599), 0.),
            linear_color_stop(rgba(0x251633CC), 1.),
        ))
        .shadow(vec![BoxShadow {
            inset: false,
            color: rgba(0x00000045).into(),
            offset: point(px(0.), px(8.)),
            blur_radius: px(24.),
            spread_radius: px(0.),
        }])
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
        for foreground in [super::TEXT, super::MUTED, super::ACCENT] {
            assert!((luminance(foreground) + 0.05) / (luminance(0x3D294F) + 0.05) >= 4.5);
        }
    }
}
