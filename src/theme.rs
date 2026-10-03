use ratatui::style::Color;

pub struct Theme;

impl Theme {
    // Brand & Accents
    pub const ACCENT: Color = Color::Rgb(56, 189, 248); // Sky Cyan #38bdf8

    // Typography
    pub const TEXT_PRIMARY: Color = Color::Rgb(241, 245, 249); // Crisp White #f1f5f9
    pub const TEXT_MUTED: Color = Color::Rgb(148, 163, 184); // Slate Gray #94a3b8

    // Borders & UI Chrome
    pub const BORDER: Color = Color::Rgb(71, 85, 105); // Slate 600 #475569
    pub const BG_CARD: Color = Color::Rgb(15, 23, 42); // Deep Navy Slate #0f172a
    pub const BG_SELECTED: Color = Color::Rgb(30, 58, 138); // Deep Indigo #1e3a8a

    // Status Indicators
    pub const SUCCESS: Color = Color::Rgb(74, 222, 128); // Soft Emerald #4ade80
    pub const WARNING: Color = Color::Rgb(251, 191, 36); // Warm Amber #fbbf24
    pub const DANGER: Color = Color::Rgb(248, 113, 113); // Coral Red #f87171
}
