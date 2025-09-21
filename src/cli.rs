use anyhow::Result;
use std::io::{self, Write};
use tokio::sync::mpsc;

use crate::network::{NetworkCommand, TrainDBNode};

pub struct InteractiveCLI {
    command_sender: mpsc::UnboundedSender<NetworkCommand>,
}

impl InteractiveCLI {
    pub fn new(command_sender: mpsc::UnboundedSender<NetworkCommand>) -> Self {
        Self { command_sender }
    }
    
    pub async fn run(&self) -> Result<()> {
        println!("TrainDB Interactive CLI");
        println!("Commands: set <key> <value>, get <key>, delete <key>, list, quit");
        println!("Type 'help' for more information");
        
        loop {
            print!("traindb> ");
            io::stdout().flush()?;
            
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();
            
            if input.is_empty() {
                continue;
            }
            
            let parts: Vec<&str> = input.split_whitespace().collect();
            
            match parts[0] {
                "set" => {
                    if parts.len() != 3 {
                        println!("Usage: set <key> <value>");
                        continue;
                    }
                    let key = parts[1].to_string();
                    let value = parts[2].to_string();
                    
                    if let Err(e) = self.command_sender.send(NetworkCommand::SetKey { key, value }) {
                        eprintln!("Failed to send command: {}", e);
                    } else {
                        println!("Key set successfully");
                    }
                }
                "get" => {
                    if parts.len() != 2 {
                        println!("Usage: get <key>");
                        continue;
                    }
                    let key = parts[1].to_string();
                    
                    if let Err(e) = self.command_sender.send(NetworkCommand::GetKey { key }) {
                        eprintln!("Failed to send command: {}", e);
                    }
                }
                "delete" => {
                    if parts.len() != 2 {
                        println!("Usage: delete <key>");
                        continue;
                    }
                    let key = parts[1].to_string();
                    
                    if let Err(e) = self.command_sender.send(NetworkCommand::DeleteKey { key }) {
                        eprintln!("Failed to send command: {}", e);
                    } else {
                        println!("Key deleted successfully");
                    }
                }
                "list" => {
                    if let Err(e) = self.command_sender.send(NetworkCommand::ListKeys) {
                        eprintln!("Failed to send command: {}", e);
                    }
                }
                "help" => {
                    self.show_help();
                }
                "quit" | "exit" => {
                    println!("Goodbye!");
                    break;
                }
                _ => {
                    println!("Unknown command: {}. Type 'help' for available commands.", parts[0]);
                }
            }
        }
        
        Ok(())
    }
    
    fn show_help(&self) {
        println!("Available commands:");
        println!("  set <key> <value>  - Set a key-value pair");
        println!("  get <key>          - Get the value for a key");
        println!("  delete <key>       - Delete a key");
        println!("  list               - List all keys");
        println!("  help               - Show this help message");
        println!("  quit/exit          - Exit the CLI");
    }
}
