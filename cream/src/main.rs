use std::io::{self, IsTerminal, Read};
use std::process::Command;

use cream::{ChunkPathPairs, Output};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut i_stdin = String::new();
    let mut i_stdout = String::new();
    let mut i_stderr = String::new();
    if !io::stdin().is_terminal() {
        io::stdin().read_to_string(&mut i_stdin).unwrap();
        i_stdin = i_stdin.trim_end_matches('\n').to_string();
        // let lines = io::stdin().lines();
        // i_stdin = lines.into_iter().map(|l| l.unwrap()).collect();
    } else {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let Some((command, command_args)) = args.split_first() else {
            eprintln!("usage: cream [--] <command> [args...]");
            std::process::exit(2);
        };
        let mut cmd = Command::new(command);
        cmd.args(command_args);

        println!("Command: {} {}", command, command_args.join(" "));

        let output = match cmd.output() {
            Ok(output) => output,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("cream: command not found: {command}");
                std::process::exit(127);
            }
            Err(e) => return Err(e.into()),
        };

        i_stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        i_stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    }
    let output = Output {
        o_stdin: ChunkPathPairs::new(&i_stdin),
        o_stdout: ChunkPathPairs::new(&i_stdout),
        o_stderr: ChunkPathPairs::new(&i_stderr),
        // o_stdin: Packed::new_with_strip_newlines(i_stdin),
        // o_stdout: Packed::new_with_strip_newlines(i_stdout),
        // o_stderr: Packed::new_with_strip_newlines(i_stderr),
    };
    println!("STDIN\n{}", output.o_stdin);
    println!("STDOUT\n{}", output.o_stdout);
    println!("STDERR\n{}", output.o_stderr);

    Ok(())
}
