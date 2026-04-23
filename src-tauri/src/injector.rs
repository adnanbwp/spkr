use enigo::{Enigo, Keyboard, Settings};

pub fn inject_text(text: &str) -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("Failed to initialize input injector: {e}"))?;
    enigo
        .text(text)
        .map_err(|e| format!("Text injection failed: {e}"))
}
