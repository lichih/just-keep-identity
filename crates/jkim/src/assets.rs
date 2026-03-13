use termimad::{MadSkin, crossterm::style::Color};
use console::style;

pub enum AssetId {
    GuideCompletions,
    GuideStatus,
    GuideMan,
}

impl AssetId {
    pub fn content(&self) -> &'static str {
        match self {
            AssetId::GuideCompletions => include_str!("../assets/guide_completions.md"),
            AssetId::GuideStatus => include_str!("../assets/guide_status.md"),
            AssetId::GuideMan => include_str!("../assets/guide_man.md"),
        }
    }

    pub fn render(&self) {
        let mut skin = MadSkin::default();
        skin.bold.set_fg(Color::Yellow);
        skin.italic.set_fg(Color::Cyan);
        skin.code_block.set_bg(Color::AnsiValue(236)); // Dark grey

        // Ensure the terminal area is ready for rendering
        let (width, _) = termimad::terminal_size();
        let content = self.content();

        // Print a subtle separator to stderr to avoid polluting stdout
        eprintln!("\n{}", style("─".repeat(width as usize)).dim());

        // Use write_text_on to specify stderr as the output for human instructions
        let mut stderr = std::io::stderr();
        let _ = skin.write_text_on(&mut stderr, content);

        eprintln!("{}", style("─".repeat(width as usize)).dim());
    }
}
