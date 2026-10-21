//! Google Material Design 3 (M3 Dark) palette for Open AG Patcher.
//! Follows Google's official dark theme guidelines:
//! - High-contrast accessible text
//! - Semantic tonal containers and subtle surface elevations
//! - Cohesive Google Blue primary accent
//! - Zero tacky emojis; clean typography and minimal geometric glyphs.

use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::BorderType;

// Google Material 3 Dark Palette
pub const MD_PRIMARY: Color = Color::Rgb(168, 199, 250);        // Google Blue (#A8C7FA)
pub const MD_ON_PRIMARY: Color = Color::Rgb(4, 30, 73);          // Deep Blue text on primary
pub const MD_PRIMARY_CONTAINER: Color = Color::Rgb(20, 50, 95);  // Selected button/tab container
pub const MD_SECONDARY: Color = Color::Rgb(194, 231, 255);      // Light Cyan (#C2E7FF)
pub const MD_TERTIARY: Color = Color::Rgb(208, 188, 255);       // Soft Lavender (#D0BCFF)

// Surfaces & Outlines
pub const MD_SURFACE: Color = Color::Rgb(20, 21, 24);           // Background surface (#141518)
pub const MD_CONTAINER: Color = Color::Rgb(30, 31, 37);         // Card container (#1E1F25)
pub const MD_CONTAINER_HIGH: Color = Color::Rgb(44, 46, 54);    // Active/Selected row (#2C2E36)
pub const MD_OUTLINE: Color = Color::Rgb(68, 71, 78);           // Subtle border (#44474E)
pub const MD_OUTLINE_FOCUS: Color = Color::Rgb(168, 199, 250);  // Focused border (Google Blue)

// Typography (On-Surface)
pub const MD_TEXT_HIGH: Color = Color::Rgb(230, 230, 230);      // High emphasis (#E6E6E6)
pub const MD_TEXT_MED: Color = Color::Rgb(196, 199, 197);       // Medium emphasis (#C4C7C5)
pub const MD_TEXT_LOW: Color = Color::Rgb(142, 145, 143);       // Low emphasis/Muted (#8E918F)

// Semantic Colors
pub const MD_SUCCESS: Color = Color::Rgb(129, 201, 149);        // Google Green (#81C995)
pub const MD_WARNING: Color = Color::Rgb(253, 214, 99);         // Google Amber (#FDD663)
pub const MD_ERROR: Color = Color::Rgb(242, 139, 130);          // Google Red (#F28B82)
pub const MD_INFO: Color = Color::Rgb(138, 180, 248);           // Google Light Blue (#8AB4F8)

// Borders
pub const BORDER_ROUNDED: BorderType = BorderType::Rounded;

// Legacy color aliases mapped to Material 3
pub const COLOR_BORDER: Color = MD_OUTLINE;
pub const COLOR_BORDER_FOCUS: Color = MD_OUTLINE_FOCUS;
pub const COLOR_TITLE: Color = MD_PRIMARY;
pub const COLOR_TEXT: Color = MD_TEXT_HIGH;
pub const COLOR_SUBTEXT: Color = MD_TEXT_MED;
pub const COLOR_MUTED: Color = MD_TEXT_LOW;

pub const COLOR_GREEN: Color = MD_SUCCESS;
pub const COLOR_PEACH: Color = MD_WARNING;
pub const COLOR_RED: Color = MD_ERROR;
pub const COLOR_CYAN: Color = MD_SECONDARY;
pub const COLOR_BLUE: Color = MD_PRIMARY;
pub const COLOR_SEL_BG: Color = MD_CONTAINER_HIGH;
pub const COLOR_BTN_BG_FOCUSED: Color = MD_PRIMARY_CONTAINER;

// Helper Styles
pub fn style_title() -> Style {
    Style::default().fg(MD_PRIMARY).add_modifier(Modifier::BOLD)
}

pub fn style_border_normal() -> Style {
    Style::default().fg(MD_OUTLINE)
}

pub fn style_border_active() -> Style {
    Style::default().fg(MD_OUTLINE_FOCUS).add_modifier(Modifier::BOLD)
}

pub fn style_patched() -> Style {
    Style::default().fg(MD_SUCCESS).add_modifier(Modifier::BOLD)
}

pub fn style_unpatched() -> Style {
    Style::default().fg(MD_WARNING).add_modifier(Modifier::BOLD)
}

pub fn style_missing() -> Style {
    Style::default().fg(MD_TEXT_LOW)
}

pub fn style_tab_active() -> Style {
    Style::default()
        .fg(MD_PRIMARY)
        .bg(MD_PRIMARY_CONTAINER)
        .add_modifier(Modifier::BOLD)
}

pub fn style_tab_inactive() -> Style {
    Style::default().fg(MD_TEXT_MED)
}

pub fn style_button_normal() -> Style {
    Style::default().fg(MD_TEXT_MED)
}

pub fn style_button_focused() -> Style {
    Style::default()
        .fg(MD_PRIMARY)
        .bg(MD_PRIMARY_CONTAINER)
        .add_modifier(Modifier::BOLD)
}
