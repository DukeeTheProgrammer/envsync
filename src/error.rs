use colored::*;

pub fn print_error(err: &anyhow::Error) {
    eprintln!("{} {}", "error:".red().bold(), err);

    for cause in err.chain().skip(1) {
        eprintln!("  {} {}", "caused by:".dimmed(), cause);
    }
}

#[allow(dead_code)]
pub fn print_success(msg: &str) {
    println!("{} {}", "success:".green().bold(), msg);
}

#[allow(dead_code)]
pub fn print_warning(msg: &str) {
    println!("{} {}", "warning:".yellow().bold(), msg);
}

#[allow(dead_code)]
pub fn print_info(msg: &str) {
    println!("{} {}", "info:".blue().bold(), msg);
}
