use crate::backend::Backend;
use anyhow::Result;
use clap::Args;
use serde::{Deserialize, Serialize};
use std::fs;

const DEFAULT_SYSTEM_PROMPT: &str = "You are an AI coding assistent.

You are pair programming with a USER to solve their coding task. You decide which files are important for the task.

You are an agent - please keep going until the user's query is completely resolved, before ending your turn and yielding back to the user. Only terminate your turn when you are sure that the problem is solved. Autonomously resolve the query to the best of your ability before coming back to the user.
keep it simple and precise.

Your main goal is to follow the USER's instructions at each message.

<communication>
When using markdown in assistant messages, use backticks to format file, directory, function, and class names. Use \\( and \\) for inline math, \\[ and \\] for block math.
</communication>


<tool_calling>
You have tools at your disposal to solve the coding task. Follow these rules regarding tool calls:
1. ALWAYS follow the tool call schema exactly as specified and make sure to provide all necessary parameters.
4. If you need additional information that you can get via tool calls, prefer that over asking the user.
6. Only use the standard tool call format and the available tools.
7. If you are not sure about file content or codebase structure pertaining to the user's request, use your tools to read files and gather the relevant information: do NOT guess or make up an answer.
8. You can autonomously read as many files as you need to clarify your own questions and completely resolve the user's query, not just one.
9. Batch those shell command calls (e.g. for compiling/linting) together with edit calls where appropriate.
</tool_calling>


<maximize_context_understanding>
Be THOROUGH when gathering information. Make sure you have the FULL picture before replying. Use additional tool calls or clarifying questions as needed.
TRACE every symbol back to its definitions and usages so you fully understand it.
Look past the first seemingly relevant result. EXPLORE alternative implementations, edge cases, and varied search terms until you have COMPREHENSIVE coverage of the topic.
</maximize_context_understanding>

Answer the user's request using the relevant tool(s), if they are available. Check that all the required parameters for each tool call are provided or can reasonably be inferred from context. IF there are no relevant tools or there are missing values for required parameters, ask the user to supply these values; otherwise proceed with the tool calls. If the user provides a specific value for a parameter (for example provided in quotes), make sure to use that value EXACTLY. DO NOT make up values for or ask about optional parameters. Carefully analyze descriptive terms in the request as they may indicate required parameter values that should be included even if not explicitly quoted.

To search for code, use the ripgrep `rg -n` command.";

/// Represents a layer of configuration, either from a file or from the command line.
/// All fields are optional.
#[derive(Args, Deserialize, Debug, Default)]
#[serde(default)]
pub struct ConfigLayer {
    /// The backend to use.
    #[arg(long, value_enum)]
    pub backend: Option<Backend>,

    /// The model to use for the agent.
    #[arg(long)]
    pub model: Option<String>,

    /// The system prompt to use.
    #[arg(long)]
    pub system_prompt: Option<String>,

    /// The timeout for API requests in seconds.
    #[arg(long)]
    pub timeout_seconds: Option<u64>,

    /// The maximum number of tool-use iterations.
    #[arg(long)]
    pub max_iterations: Option<u8>,

    /// The maximum number of lines to read from a file.
    #[arg(long)]
    pub max_read_lines: Option<u64>,

    /// Command prefixes that the agent is allowed to execute.
    #[arg(long, value_delimiter = ',')]
    pub allowed_command_prefixes: Vec<String>,

    /// Paths to ignore when listing or reading files.
    #[arg(long, value_delimiter = ',')]
    pub ignored_paths: Vec<String>,

    /// Paths that the agent is allowed to access.
    #[arg(long, value_delimiter = ',')]
    pub accessible_paths: Vec<String>,

    /// Enable or disable the terminal bell.
    #[arg(long)]
    pub terminal_bell: Option<bool>,

    /// Show the system prompt before starting the conversation.
    #[arg(long)]
    pub show_system_prompt: Option<bool>,

    /// Show detailed arguments and output for tool calls.
    #[arg(long)]
    pub debug_tool_calls: Option<bool>,

    /// Automatically execute all tool calls.
    #[arg(long)]
    pub auto_execute: Option<bool>,

    /// Print API messages before sending them.
    #[arg(long)]
    pub print_messages: Option<bool>,

    /// The base URL for the API client.
    #[arg(long)]
    pub base_url: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(default)]
pub struct Config {
    pub backend: Backend,
    pub model: String,
    pub system_prompt: Option<String>,
    pub timeout_seconds: u64,
    pub max_iterations: u8,
    pub max_read_lines: u64,
    pub allowed_command_prefixes: Vec<String>,
    pub ignored_paths: Vec<String>,
    pub accessible_paths: Vec<String>,
    pub terminal_bell: bool,
    pub show_system_prompt: bool,
    pub debug_tool_calls: bool,
    pub auto_execute: bool,
    pub print_messages: bool,
    pub base_url: String,
}
impl Config {
    /// Merges a configuration layer into the current configuration.
    /// Values in the layer take precedence.
    pub fn merge(&mut self, layer: &ConfigLayer) {
        if let Some(backend) = &layer.backend {
            self.backend = backend.clone();
            // Only update base_url if it wasn't explicitly provided in the same layer.
            if layer.base_url.is_none() {
                self.base_url = self.backend.config().base_url.to_string();
            }
        }

        if let Some(model) = &layer.model {
            self.model = model.clone();
        }
        if let Some(system_prompt) = &layer.system_prompt {
            // Convert empty or whitespace-only strings to None
            if system_prompt.trim().is_empty() {
                self.system_prompt = None;
            } else {
                self.system_prompt = Some(system_prompt.clone());
            }
        }
        if let Some(timeout_seconds) = layer.timeout_seconds {
            self.timeout_seconds = timeout_seconds;
        }
        if let Some(max_iterations) = layer.max_iterations {
            self.max_iterations = max_iterations;
        }
        if let Some(max_read_lines) = layer.max_read_lines {
            self.max_read_lines = max_read_lines;
        }
        if !layer.allowed_command_prefixes.is_empty() {
            self.allowed_command_prefixes = layer.allowed_command_prefixes.clone();
        }
        if !layer.ignored_paths.is_empty() {
            self.ignored_paths = layer.ignored_paths.clone();
        }
        if !layer.accessible_paths.is_empty() {
            self.accessible_paths = layer.accessible_paths.clone();
        }
        if let Some(terminal_bell) = layer.terminal_bell {
            self.terminal_bell = terminal_bell;
        }
        if let Some(show_system_prompt) = layer.show_system_prompt {
            self.show_system_prompt = show_system_prompt;
        }
        if let Some(debug_tool_calls) = layer.debug_tool_calls {
            self.debug_tool_calls = debug_tool_calls;
        }
        if let Some(auto_execute) = layer.auto_execute {
            self.auto_execute = auto_execute;
        }
        if let Some(print_messages) = layer.print_messages {
            self.print_messages = print_messages;
        }
        if let Some(base_url) = &layer.base_url {
            self.base_url = base_url.clone();
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        let backend = Backend::default();
        Self {
            backend: backend.clone(),
            model: "openai/gpt-4.1-mini".to_string(),
            system_prompt: Some(DEFAULT_SYSTEM_PROMPT.to_string()),
            timeout_seconds: 120,
            max_iterations: 50,
            max_read_lines: 1000,
            allowed_command_prefixes: vec![
                "ls".to_string(),
                "cat".to_string(),
                "echo".to_string(),
                "pwd".to_string(),
                "rg".to_string(),
                "git diff".to_string(),
            ],
            ignored_paths: vec![".git".to_string()],
            accessible_paths: vec![".".to_string()],
            terminal_bell: true,
            show_system_prompt: false,
            debug_tool_calls: false,
            auto_execute: false,
            print_messages: false,
            base_url: backend.config().base_url.to_string(),
        }
    }
}

/// Loads configuration from defaults, a configuration file, and CLI arguments.
/// The layers are applied in order, with later layers taking precedence.
///
/// 1. `Config::default()` is used as the base.
/// 2. The `config.toml` file is loaded and merged.
/// 3. The `cli_layer` from command-line arguments is merged.
///
/// The function will also create or update the `config.toml` file to include any
/// newly available default settings, making them discoverable to the user.
pub fn load(cli_layer: &ConfigLayer) -> Result<Config> {
    let xdg_dirs = xdg::BaseDirectories::new();
    let config_path = xdg_dirs.place_config_file("alors/config.toml")?;

    // Load file layer, or use a default if it doesn't exist or fails to parse.
    let file_layer: ConfigLayer = if config_path.exists() {
        let config_string = fs::read_to_string(&config_path)?;
        toml::from_str(&config_string).unwrap_or_default()
    } else {
        ConfigLayer::default()
    };

    // Determine the state of the config as it should be on disk.
    let mut config_for_disk = Config::default();
    config_for_disk.merge(&file_layer);

    // If the on-disk representation is out of date or doesn't exist, write it.
    let new_disk_toml = toml::to_string_pretty(&config_for_disk)?;
    let old_disk_toml = fs::read_to_string(&config_path).unwrap_or_default();

    if new_disk_toml != old_disk_toml {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&config_path, new_disk_toml)?;
        if old_disk_toml.is_empty() {
            println!("Created default config at: {}", config_path.display());
        }
    }

    // Start with the on-disk config state and merge the final CLI layer.
    let mut final_config = config_for_disk;
    final_config.merge(cli_layer);

    Ok(final_config)
}
