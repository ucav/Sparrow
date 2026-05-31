// Shell completions for Sparrow
// Generate with: sparrow completions <shell>
// Or: cargo run -- completions bash

pub fn generate(shell: &str) -> String {
    match shell {
        "bash" => include_str!("../completions/sparrow.bash").to_string(),
        "zsh" => include_str!("../completions/_sparrow").to_string(),
        "fish" => include_str!("../completions/sparrow.fish").to_string(),
        "powershell" => include_str!("../completions/_sparrow.ps1").to_string(),
        _ => "Unknown shell. Supported: bash, zsh, fish, powershell".to_string(),
    }
}
