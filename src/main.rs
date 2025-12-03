mod config;
mod transformer;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use config::{AppConfig, ColumnStrategy, TableConfig};
use dialoguer::{theme::ColorfulTheme, Input, Select};
use log::{debug, info, warn};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use transformer::Transformer;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long)]
    input: Option<PathBuf>,

    #[arg(short, long)]
    output: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Run(RunArgs),
    Scan(ScanArgs),
}

#[derive(clap::Args, Debug)]
struct RunArgs {
    #[arg(short, long)]
    input: PathBuf,

    #[arg(short, long)]
    output: PathBuf,

    #[arg(short, long)]
    config: PathBuf,

    #[arg(short, long, default_value_t = 42)]
    seed: u64,
}

#[derive(clap::Args, Debug)]
struct ScanArgs {
    #[arg(short, long)]
    input: PathBuf,

    #[arg(short = 'I', long, default_value_t = false)]
    interactive: bool,
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Run(args)) => {
            let config = AppConfig::load(&args.config)?;
            run_processing(&args.input, &args.output, &config, args.seed)
        }
        Some(Commands::Scan(args)) => process_scan(args),
        None => {
            if let Some(input) = cli.input {
                let output = cli.output.unwrap_or_else(|| {
                    let mut name = input.file_stem().unwrap_or_default().to_os_string();
                    name.push("_anonymized.sql");
                    PathBuf::from(name)
                });
                process_smart_run(input, output)
            } else {
                Err(anyhow!("No input file provided. Use --input or a subcommand."))
            }
        }
    }
}

fn process_scan(args: ScanArgs) -> Result<()> {
    info!("Scanning file: {:?}", args.input);
    let mut config = scan_file(&args.input)?;

    if args.interactive {
        run_interactive_wizard(&mut config)?;
    } else {
        let yaml = serde_yaml::to_string(&config)?;
        println!("{}", yaml);
    }
    Ok(())
}

fn process_smart_run(input: PathBuf, output: PathBuf) -> Result<()> {
    info!("Starting Smart Run...");
    info!("Input: {:?}", input);
    
    println!("Scanning file for schema...");
    let mut config = scan_file(&input)?;
    println!("Found {} tables.", config.tables.len());

    println!("\nProposed Anonymization Plan:");
    for (table, t_conf) in &config.tables {
        println!("Table: {}", table);
        for (col, strat) in &t_conf.columns {
             if matches!(strat, ColumnStrategy::Keep) {
             } else {
                 println!("  - {} -> {:?}", col, strat);
             }
        }
    }

    let theme = ColorfulTheme::default();
    let options = vec![
        "Run (Execute Plan)",
        "Customize Plan",
        "Quit"
    ];
    
    let selection = Select::with_theme(&theme)
        .with_prompt("Ready to proceed?")
        .default(0)
        .items(&options)
        .interact()?;

    match selection {
        0 => {
            println!("Anonymizing to {:?}...", output);
            run_processing(&input, &output, &config, 42)?;
        }
        1 => {
            run_interactive_wizard(&mut config)?;
            println!("Anonymizing to {:?}...", output);
            run_processing(&input, &output, &config, 42)?;
        }
        _ => {
            println!("Bye!");
        }
    }

    Ok(())
}

fn scan_file(path: &Path) -> Result<AppConfig> {
    let input_file = File::open(path)
        .with_context(|| format!("Failed to open input file: {:?}", path))?;
    let reader = BufReader::new(input_file);

    let insert_regex = Regex::new(r"(?i)^INSERT\s+INTO\s+(\S+)\s*\((.*?)\)\s*VALUES")
        .expect("Invalid regex pattern");

    let mut tables_columns: HashMap<String, HashSet<String>> = HashMap::new();

    for line_result in reader.lines() {
        let line = line_result?;
        if let Some(caps) = insert_regex.captures(&line) {
            let table_full_name = caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
            let cols_part = caps.get(2).map(|m| m.as_str()).unwrap_or("");

            let columns: Vec<String> = cols_part
                .split(',')
                .map(|s| s.trim().trim_matches('"').to_string())
                .collect();

            let entry = tables_columns.entry(table_full_name).or_default();
            for col in columns {
                entry.insert(col);
            }
        }
    }

    let mut config = AppConfig {
        tables: HashMap::new(),
    };

    for (table_name, columns) in tables_columns {
        let mut table_config = TableConfig {
            columns: HashMap::new(),
        };
        for col in columns {
            let strategy = guess_strategy(&col);
            table_config.columns.insert(col, strategy);
        }
        config.tables.insert(table_name, table_config);
    }

    Ok(config)
}

fn guess_strategy(col_name: &str) -> ColumnStrategy {
    let lower = col_name.to_lowercase();

    if lower == "id" || lower.ends_with("_id") || lower.ends_with("uuid") || lower.ends_with("guid") {
        return ColumnStrategy::Keep;
    }

    if lower.contains("date") || lower.contains("time") || lower.ends_with("_at") {
        return ColumnStrategy::Keep;
    }

    if lower.contains("amount") 
        || lower.contains("price") 
        || lower.contains("sum") 
        || lower.contains("total") 
        || lower.contains("balance") 
        || lower.contains("cost") 
        || lower.contains("currency") {
        return ColumnStrategy::Keep;
    }

    if lower.contains("email") {
        return ColumnStrategy::Email;
    }
    if lower.contains("phone") || lower.contains("mobile") {
        return ColumnStrategy::Phone;
    }
    if lower == "first_name" || lower == "firstname" {
        return ColumnStrategy::FirstName;
    }
    if lower == "last_name" || lower == "lastname" || lower == "surname" {
        return ColumnStrategy::LastName;
    }
    if lower.contains("name") && !lower.contains("user") && !lower.contains("file") && !lower.contains("domain") {
        return ColumnStrategy::FullName;
    }
    if lower.contains("address") || lower.contains("city") || lower.contains("street") {
        return ColumnStrategy::Fixed("ANONYMIZED ADDRESS".to_string());
    }
    if lower.contains("password") || lower.contains("token") || lower.contains("secret") || lower.contains("key") {
         return ColumnStrategy::Fixed("REDACTED_SECRET".to_string());
    }
    if lower.contains("description") || lower.contains("comment") || lower.contains("note") {
        return ColumnStrategy::Mask;
    }

    ColumnStrategy::Keep
}

fn run_processing(input: &Path, output: &Path, config: &AppConfig, seed: u64) -> Result<()> {
    let transformer = Transformer::new(seed);

    let input_file = File::open(input)
        .with_context(|| format!("Failed to open input file: {:?}", input))?;
    let reader = BufReader::new(input_file);

    let output_file = File::create(output)
        .with_context(|| format!("Failed to create output file: {:?}", output))?;
    let mut writer = BufWriter::new(output_file);

    let insert_regex = Regex::new(r"(?i)^INSERT\s+INTO\s+(\S+)\s*\((.*?)\)\s*VALUES\s*\((.*)\);")
        .expect("Invalid regex pattern");

    let mut processed_lines = 0;
    let mut anonymized_count = 0;

    for line_result in reader.lines() {
        let line = line_result.context("Error reading line from input")?;
        processed_lines += 1;

        if processed_lines % 100_000 == 0 {
            info!("Processed {} lines...", processed_lines);
        }

        if let Some(caps) = insert_regex.captures(&line) {
            let table_full_name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            
            let table_key = if config.tables.contains_key(table_full_name) {
                Some(table_full_name)
            } else {
                table_full_name.split('.').last().and_then(|name| {
                    if config.tables.contains_key(name) {
                        Some(name)
                    } else {
                        None
                    }
                })
            };

            if let Some(key) = table_key {
                if let Some(table_config) = config.tables.get(key) {
                    let cols_part = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                    let vals_part = caps.get(3).map(|m| m.as_str()).unwrap_or("");

                    let columns: Vec<String> = cols_part
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').to_string())
                        .collect();

                    let values = Transformer::parse_values(vals_part);

                    if columns.len() != values.len() {
                        warn!("Column count mismatch. Skipping line {}", processed_lines);
                        writeln!(writer, "{}", line)?;
                        continue;
                    }

                    let mut new_values = Vec::with_capacity(values.len());

                    for (i, col_name) in columns.iter().enumerate() {
                        let strategy = table_config
                            .columns
                            .get(col_name)
                            .unwrap_or(&ColumnStrategy::Keep);
                        
                        let original_val = &values[i];
                        let new_val = transformer.transform(original_val, strategy);
                        new_values.push(new_val);
                    }

                    let new_values_str = new_values.join(", ");
                    writeln!(
                        writer,
                        "INSERT INTO {} ({}) VALUES ({});",
                        table_full_name, cols_part, new_values_str
                    )?;
                    anonymized_count += 1;
                } else {
                    writeln!(writer, "{}", line)?;
                }
            } else {
                writeln!(writer, "{}", line)?;
            }
        } else {
            writeln!(writer, "{}", line)?;
        }
    }

    writer.flush().context("Failed to flush output buffer")?;
    info!("Done! Processed {} lines. Anonymized {} statements.", processed_lines, anonymized_count);
    Ok(())
}

fn run_interactive_wizard(config: &mut AppConfig) -> Result<()> {
    let theme = ColorfulTheme::default();
    println!("GhostDB Interactive Config Wizard");
    
    loop {
        let mut table_names: Vec<String> = config.tables.keys().cloned().collect();
        table_names.sort();
        
        let mut choices = table_names.clone();
        choices.push("Save and Proceed".to_string());

        let selection = Select::with_theme(&theme)
            .with_prompt("Select a table to configure")
            .default(0)
            .items(&choices)
            .interact()?;

        if selection == table_names.len() {
            break;
        }

        let table_name = &table_names[selection];
        configure_table(table_name, config.tables.get_mut(table_name).unwrap())?;
    }

    Ok(())
}

fn configure_table(table_name: &str, table_config: &mut TableConfig) -> Result<()> {
    let theme = ColorfulTheme::default();
    
    loop {
        let mut col_names: Vec<String> = table_config.columns.keys().cloned().collect();
        col_names.sort();

        let display_items: Vec<String> = col_names.iter().map(|c| {
            let strategy = table_config.columns.get(c).unwrap();
            format!("{} [{:?}]", c, strategy)
        }).collect();

        let mut choices = display_items.clone();
        choices.push("Back to Tables".to_string());

        let selection = Select::with_theme(&theme)
            .with_prompt(format!("Configure columns for table '{}'", table_name))
            .default(0)
            .items(&choices)
            .interact()?;

        if selection == col_names.len() {
            break;
        }

        let col_name = &col_names[selection];
        let new_strategy = select_strategy(col_name)?;
        table_config.columns.insert(col_name.clone(), new_strategy);
    }
    Ok(())
}

fn select_strategy(col_name: &str) -> Result<ColumnStrategy> {
    let strategies = vec![
        ("Keep (Original Value)", ColumnStrategy::Keep),
        ("Email (fake@example.com)", ColumnStrategy::Email),
        ("First Name (Alice)", ColumnStrategy::FirstName),
        ("Last Name (Smith)", ColumnStrategy::LastName),
        ("Full Name (Alice Smith)", ColumnStrategy::FullName),
        ("Phone (+1-555...)", ColumnStrategy::Phone),
        ("Mask (a***@e***.com)", ColumnStrategy::Mask),
        ("Fixed Value...", ColumnStrategy::Fixed("".to_string())),
    ];

    let items: Vec<&str> = strategies.iter().map(|(n, _)| *n).collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Select strategy for column '{}'", col_name))
        .items(&items)
        .interact()?;

    let (_, strategy) = &strategies[selection];
    
    match strategy {
        ColumnStrategy::Fixed(_) => {
            let val: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Enter the fixed value")
                .interact_text()?;
            Ok(ColumnStrategy::Fixed(val))
        }
        _ => Ok(strategy.clone()),
    }
}
