use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{mpsc, Arc},
    thread,
    time::{Duration, Instant},
};

use agentgrid_protocol::{
    BrowserPayload, CommandPayload, DesktopPayload, DockerPayload, FilePayload, GitPayload,
    HttpRequestPayload, JobPayload, JobResult, PluginPayload, SessionPayload,
};
use base64::{engine::general_purpose, Engine as _};
use serde_json::{json, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExecutorError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("unsupported action")]
    Unsupported,
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

pub type Result<T> = std::result::Result<T, ExecutorError>;
pub type LogCallback = Arc<dyn Fn(&str, &str) + Send + Sync + 'static>;

pub fn execute(payload: &JobPayload) -> Result<JobResult> {
    match payload {
        JobPayload::HttpRequest(request) => execute_http(request),
        JobPayload::Command(command) => execute_command(command),
        JobPayload::File(file) => execute_file(file),
        JobPayload::Git(git) => execute_git(git),
        JobPayload::Docker(docker) => execute_docker(docker),
        JobPayload::Browser(browser) => execute_browser(browser),
        JobPayload::Desktop(desktop) => execute_desktop(desktop),
        JobPayload::Session(session) => execute_session(session),
        JobPayload::AgentMessage(_) => Err(ExecutorError::Unsupported),
        JobPayload::Plugin(plugin) => execute_plugin(plugin),
        JobPayload::Custom { .. } => Err(ExecutorError::Unsupported),
    }
}

pub fn execute_with_cancel<F>(payload: &JobPayload, should_stop: F) -> Result<JobResult>
where
    F: Fn() -> bool + Send + 'static,
{
    execute_with_cancel_and_logs(payload, should_stop, Arc::new(|_, _| {}))
}

pub fn execute_with_cancel_and_logs<F>(
    payload: &JobPayload,
    should_stop: F,
    log: LogCallback,
) -> Result<JobResult>
where
    F: Fn() -> bool + Send + 'static,
{
    match payload {
        JobPayload::Command(command) => {
            execute_command_with_cancel_and_logs(command, should_stop, log)
        }
        JobPayload::Session(session) => {
            execute_session_with_cancel_and_logs(session, should_stop, log)
        }
        JobPayload::Browser(BrowserPayload::Automate {
            url,
            actions,
            screenshot_path,
            timeout_seconds,
        }) => execute_browser_automation_with_cancel(
            url,
            actions,
            screenshot_path.as_deref(),
            *timeout_seconds,
            should_stop,
            log,
        ),
        JobPayload::Docker(DockerPayload::Run {
            image,
            args,
            timeout_seconds,
        }) => {
            let mut docker_args = vec!["run".to_string(), "--rm".to_string(), image.clone()];
            docker_args.extend(args.clone());
            run_command_result_with_cancel(
                "docker.run".to_string(),
                "docker",
                &docker_args,
                None,
                *timeout_seconds,
                should_stop,
                log,
            )
            .map(docker_result_from_command_like)
        }
        JobPayload::Plugin(plugin) => execute_plugin(plugin),
        other => execute(other),
    }
}

pub fn execute_desktop(payload: &DesktopPayload) -> Result<JobResult> {
    match payload {
        DesktopPayload::Screenshot {
            path,
            timeout_seconds,
        } => execute_desktop_screenshot(path.as_deref(), *timeout_seconds),
        DesktopPayload::Click {
            x,
            y,
            button,
            timeout_seconds,
        } => execute_desktop_click(*x, *y, button, *timeout_seconds),
        DesktopPayload::TypeText {
            text,
            interval_ms,
            timeout_seconds,
        } => execute_desktop_type_text(text, *interval_ms, *timeout_seconds),
        DesktopPayload::Key {
            key,
            modifiers,
            timeout_seconds,
        } => execute_desktop_key(key, modifiers, *timeout_seconds),
    }
}

fn execute_desktop_screenshot(path: Option<&str>, timeout_seconds: u64) -> Result<JobResult> {
    let started = Instant::now();
    let path = path
        .map(ToString::to_string)
        .unwrap_or_else(default_screenshot_path);
    if cfg!(windows) {
        let escaped_path = path.replace('\'', "''");
        let script = format!(
            r#"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$screen = [System.Windows.Forms.Screen]::PrimaryScreen
if ($null -eq $screen) {{ throw "No primary screen is available in this Windows session." }}
$bounds = $screen.Bounds
$bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
$dir = Split-Path -Parent '{escaped_path}'
if ($dir -and -not (Test-Path $dir)) {{ New-Item -ItemType Directory -Force -Path $dir | Out-Null }}
$bitmap.Save('{escaped_path}', [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bitmap.Dispose()
[Console]::Out.WriteLine((@{{ path = '{escaped_path}'; width = $bounds.Width; height = $bounds.Height }} | ConvertTo-Json -Compress))
"#
        );
        let args = vec![
            "-NoProfile".to_string(),
            "-ExecutionPolicy".to_string(),
            "Bypass".to_string(),
            "-Command".to_string(),
            script,
        ];
        let result = run_command_result(
            "desktop.screenshot".to_string(),
            "powershell.exe",
            &args,
            None,
            timeout_seconds.max(5),
        )?;
        let JobResult::GitResult {
            exit_code,
            stdout,
            stderr,
            ..
        } = result
        else {
            return Ok(result);
        };
        if exit_code.unwrap_or(1) != 0 {
            return Ok(JobResult::Error {
                code: "desktop_screenshot_failed".to_string(),
                message: if stderr.trim().is_empty() {
                    stdout
                } else {
                    stderr
                },
                retryable: false,
            });
        }
        let parsed = serde_json::from_str::<Value>(&stdout).unwrap_or_else(|_| json!({}));
        let bytes = fs::read(&path)?;
        let size_bytes = bytes.len() as u64;
        Ok(JobResult::DesktopResult {
            operation: "screenshot".to_string(),
            path: Some(path.clone()),
            content_base64: Some(general_purpose::STANDARD.encode(&bytes)),
            width: parsed.get("width").and_then(Value::as_u64),
            height: parsed.get("height").and_then(Value::as_u64),
            bytes: size_bytes,
            message: "screenshot captured from interactive desktop session".to_string(),
            duration_ms: started.elapsed().as_millis() as u64,
        })
    } else {
        Ok(JobResult::Error {
            code: "desktop_screenshot_unsupported".to_string(),
            message: "desktop screenshot v1 is currently implemented for Windows interactive sessions only".to_string(),
            retryable: false,
        })
    }
}

fn execute_desktop_click(x: i32, y: i32, button: &str, timeout_seconds: u64) -> Result<JobResult> {
    let started = Instant::now();
    if !cfg!(windows) {
        return Ok(unsupported_desktop_result("click"));
    }
    let down_up = match button.to_ascii_lowercase().as_str() {
        "left" => (0x0002_u32, 0x0004_u32),
        "right" => (0x0008_u32, 0x0010_u32),
        "middle" => (0x0020_u32, 0x0040_u32),
        other => {
            return Ok(JobResult::Error {
                code: "desktop_click_invalid_button".to_string(),
                message: format!("unsupported desktop click button: {other}"),
                retryable: false,
            });
        }
    };
    let script = format!(
        r#"
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class AgentGridMouse {{
  [DllImport("user32.dll")]
  public static extern bool SetCursorPos(int X, int Y);
  [DllImport("user32.dll")]
  public static extern void mouse_event(uint dwFlags, uint dx, uint dy, uint dwData, UIntPtr dwExtraInfo);
}}
"@
[AgentGridMouse]::SetCursorPos({x}, {y}) | Out-Null
Start-Sleep -Milliseconds 60
[AgentGridMouse]::mouse_event({down}, 0, 0, 0, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 80
[AgentGridMouse]::mouse_event({up}, 0, 0, 0, [UIntPtr]::Zero)
[Console]::Out.WriteLine((@{{ operation = "click"; x = {x}; y = {y}; button = "{button}" }} | ConvertTo-Json -Compress))
"#,
        down = down_up.0,
        up = down_up.1,
        button = json_escape_string(button)
    );
    run_desktop_powershell("click", script, timeout_seconds, started)
}

fn execute_desktop_type_text(
    text: &str,
    interval_ms: Option<u64>,
    timeout_seconds: u64,
) -> Result<JobResult> {
    let started = Instant::now();
    if !cfg!(windows) {
        return Ok(unsupported_desktop_result("type_text"));
    }
    let escaped_text = powershell_single_quote(text);
    let interval = interval_ms.unwrap_or(0);
    let script = if interval == 0 {
        format!(
            r#"
Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.SendKeys]::SendWait('{text}')
[Console]::Out.WriteLine((@{{ operation = "type_text"; chars = {chars}; interval_ms = 0 }} | ConvertTo-Json -Compress))
"#,
            text = sendkeys_escape(text),
            chars = text.chars().count()
        )
    } else {
        format!(
            r#"
Add-Type -AssemblyName System.Windows.Forms
$text = '{text}'
foreach ($ch in $text.ToCharArray()) {{
  [System.Windows.Forms.SendKeys]::SendWait([string]$ch)
  Start-Sleep -Milliseconds {interval}
}}
[Console]::Out.WriteLine((@{{ operation = "type_text"; chars = $text.Length; interval_ms = {interval} }} | ConvertTo-Json -Compress))
"#,
            text = escaped_text,
            interval = interval
        )
    };
    run_desktop_powershell("type_text", script, timeout_seconds, started)
}

fn execute_desktop_key(key: &str, modifiers: &[String], timeout_seconds: u64) -> Result<JobResult> {
    let started = Instant::now();
    if !cfg!(windows) {
        return Ok(unsupported_desktop_result("key"));
    }
    let send_key = sendkeys_key_combo(key, modifiers);
    let script = format!(
        r#"
Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.SendKeys]::SendWait('{send_key}')
[Console]::Out.WriteLine((@{{ operation = "key"; key = "{key}"; modifiers = @({modifiers}) }} | ConvertTo-Json -Compress))
"#,
        send_key = powershell_single_quote(&send_key),
        key = json_escape_string(key),
        modifiers = modifiers
            .iter()
            .map(|item| format!("\"{}\"", json_escape_string(item)))
            .collect::<Vec<_>>()
            .join(", ")
    );
    run_desktop_powershell("key", script, timeout_seconds, started)
}

fn run_desktop_powershell(
    operation: &str,
    script: String,
    timeout_seconds: u64,
    started: Instant,
) -> Result<JobResult> {
    let args = vec![
        "-NoProfile".to_string(),
        "-ExecutionPolicy".to_string(),
        "Bypass".to_string(),
        "-Command".to_string(),
        script,
    ];
    let result = run_command_result(
        format!("desktop.{operation}"),
        "powershell.exe",
        &args,
        None,
        timeout_seconds.max(5),
    )?;
    let JobResult::GitResult {
        exit_code,
        stdout,
        stderr,
        ..
    } = result
    else {
        return Ok(result);
    };
    if exit_code.unwrap_or(1) != 0 {
        return Ok(JobResult::Error {
            code: format!("desktop_{operation}_failed"),
            message: if stderr.trim().is_empty() {
                stdout
            } else {
                stderr
            },
            retryable: false,
        });
    }
    Ok(JobResult::DesktopResult {
        operation: operation.to_string(),
        path: None,
        content_base64: None,
        width: None,
        height: None,
        bytes: 0,
        message: stdout.trim().to_string(),
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

fn default_screenshot_path() -> String {
    let file_name = format!(
        "agentgrid-screenshot-{}.png",
        chrono::Utc::now().timestamp_millis()
    );
    std::env::temp_dir().join(file_name).display().to_string()
}

fn unsupported_desktop_result(operation: &str) -> JobResult {
    JobResult::Error {
        code: format!("desktop_{operation}_unsupported"),
        message: format!(
            "desktop {operation} v1 is currently implemented for Windows interactive sessions only"
        ),
        retryable: false,
    }
}

fn powershell_single_quote(value: &str) -> String {
    value.replace('\'', "''")
}

fn json_escape_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn sendkeys_escape(value: &str) -> String {
    powershell_single_quote(
        &value
            .chars()
            .map(|ch| match ch {
                '+' | '^' | '%' | '~' | '(' | ')' | '{' | '}' | '[' | ']' => {
                    format!("{{{ch}}}")
                }
                '\n' => "{ENTER}".to_string(),
                '\r' => String::new(),
                _ => ch.to_string(),
            })
            .collect::<String>(),
    )
}

fn sendkeys_key_combo(key: &str, modifiers: &[String]) -> String {
    let mut prefix = String::new();
    for modifier in modifiers {
        match modifier.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => prefix.push('^'),
            "alt" => prefix.push('%'),
            "shift" => prefix.push('+'),
            _ => {}
        }
    }
    let normalized_key = match key.to_ascii_lowercase().as_str() {
        "enter" | "return" => "ENTER".to_string(),
        "esc" | "escape" => "ESC".to_string(),
        "tab" => "TAB".to_string(),
        "backspace" => "BACKSPACE".to_string(),
        "delete" | "del" => "DEL".to_string(),
        "up" => "UP".to_string(),
        "down" => "DOWN".to_string(),
        "left" => "LEFT".to_string(),
        "right" => "RIGHT".to_string(),
        "space" => " ".to_string(),
        other if other.len() == 1 => other.to_string(),
        other => other.to_ascii_uppercase(),
    };
    if normalized_key.len() == 1 {
        format!("{prefix}{normalized_key}")
    } else {
        format!("{prefix}{{{normalized_key}}}")
    }
}

pub fn execute_plugin(payload: &PluginPayload) -> Result<JobResult> {
    let started = Instant::now();
    let plugin_dir = std::env::var("AGENTGRID_PLUGIN_DIR")
        .unwrap_or_else(|_| "/opt/agentgrid/plugins".to_string());
    let executable = Path::new(&plugin_dir).join(&payload.plugin_id);
    if !executable.exists() {
        return Err(ExecutorError::InvalidInput(format!(
            "plugin executable not found: {}",
            executable.display()
        )));
    }
    let mut child = Command::new(executable)
        .arg(&payload.action)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        let input = serde_json::to_vec(&json!({
            "api_version": "agentgrid.plugin/v1",
            "kind": "WorkerPluginRequest",
            "plugin_id": payload.plugin_id,
            "action": payload.action,
            "input": payload.input
        }))
        .map_err(|error| ExecutorError::InvalidInput(error.to_string()))?;
        stdin.write_all(&input)?;
        drop(stdin);
    }
    let result = wait_child_output(
        format!("plugin.{}", payload.plugin_id),
        child,
        payload.timeout_seconds,
        started,
    )?;
    let JobResult::CommandResult {
        exit_code,
        stdout,
        stderr,
        duration_ms,
    } = result
    else {
        return Err(ExecutorError::Unsupported);
    };
    if exit_code != Some(0) {
        return Ok(JobResult::Error {
            code: "plugin_failed".to_string(),
            message: stderr,
            retryable: false,
        });
    }
    let output = serde_json::from_str(&stdout).unwrap_or_else(|_| json!({ "stdout": stdout }));
    Ok(JobResult::PluginResult {
        plugin_id: payload.plugin_id.clone(),
        action: payload.action.clone(),
        output,
        duration_ms,
    })
}

pub fn execute_http(payload: &HttpRequestPayload) -> Result<JobResult> {
    let started = Instant::now();
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(
            payload.timeout_seconds.max(1),
        ))
        .build()?;
    let method = payload.method.parse().unwrap_or(reqwest::Method::GET);
    let mut request = client.request(method, &payload.url);
    for (key, value) in &payload.headers {
        request = request.header(key, value);
    }
    if let Some(body) = &payload.body {
        request = request.json(body);
    }
    let response = request.send()?;
    let status_code = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .map(|(key, value)| {
            (
                key.to_string(),
                value.to_str().unwrap_or("<non-utf8>").to_string(),
            )
        })
        .collect();
    let mut body = response.text()?;
    if body.len() > payload.max_response_bytes as usize {
        body.truncate(payload.max_response_bytes as usize);
    }

    Ok(JobResult::HttpResponse {
        status_code,
        headers,
        body: serde_json::from_str(&body).unwrap_or(Value::String(body)),
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

pub fn execute_command(payload: &CommandPayload) -> Result<JobResult> {
    run_command_result(
        "command".to_string(),
        &payload.program,
        &payload.args,
        payload.working_dir.as_deref(),
        payload.timeout_seconds,
    )
}

pub fn execute_command_with_cancel<F>(payload: &CommandPayload, should_stop: F) -> Result<JobResult>
where
    F: Fn() -> bool + Send + 'static,
{
    execute_command_with_cancel_and_logs(payload, should_stop, Arc::new(|_, _| {}))
}

pub fn execute_command_with_cancel_and_logs<F>(
    payload: &CommandPayload,
    should_stop: F,
    log: LogCallback,
) -> Result<JobResult>
where
    F: Fn() -> bool + Send + 'static,
{
    run_command_result_with_cancel(
        "command".to_string(),
        &payload.program,
        &payload.args,
        payload.working_dir.as_deref(),
        payload.timeout_seconds,
        should_stop,
        log,
    )
}

fn run_command_result(
    operation: String,
    program: &str,
    args: &[String],
    working_dir: Option<&str>,
    timeout_seconds: u64,
) -> Result<JobResult> {
    let started = Instant::now();
    let mut command = Command::new(program);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(working_dir) = working_dir {
        command.current_dir(working_dir);
    }
    let child = command.spawn()?;
    wait_child_output(operation, child, timeout_seconds, started)
}

fn run_command_result_with_cancel<F>(
    operation: String,
    program: &str,
    args: &[String],
    working_dir: Option<&str>,
    timeout_seconds: u64,
    should_stop: F,
    log: LogCallback,
) -> Result<JobResult>
where
    F: Fn() -> bool + Send + 'static,
{
    let started = Instant::now();
    let mut command = Command::new(program);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(working_dir) = working_dir {
        command.current_dir(working_dir);
    }
    let child = command.spawn()?;
    wait_child_output_with_cancel(operation, child, timeout_seconds, started, should_stop, log)
}

fn wait_child_output(
    operation: String,
    child: Child,
    timeout_seconds: u64,
    started: Instant,
) -> Result<JobResult> {
    let timeout = Duration::from_secs(timeout_seconds.max(1));
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let output = child.wait_with_output();
        let _ = tx.send(output);
    });
    let output = match rx.recv_timeout(timeout) {
        Ok(output) => output?,
        Err(_) => {
            return Ok(JobResult::Error {
                code: "command_timeout".to_string(),
                message: format!("command exceeded {} seconds", timeout_seconds.max(1)),
                retryable: false,
            });
        }
    };

    if operation == "command" || operation.starts_with("plugin.") {
        return Ok(JobResult::CommandResult {
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            duration_ms: started.elapsed().as_millis() as u64,
        });
    }

    Ok(JobResult::GitResult {
        operation,
        exit_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

fn wait_child_output_with_cancel<F>(
    operation: String,
    mut child: Child,
    timeout_seconds: u64,
    started: Instant,
    should_stop: F,
    log: LogCallback,
) -> Result<JobResult>
where
    F: Fn() -> bool + Send + 'static,
{
    let timeout_at = Instant::now() + Duration::from_secs(timeout_seconds.max(1));
    let stdout = Arc::new(std::sync::Mutex::new(String::new()));
    let stderr = Arc::new(std::sync::Mutex::new(String::new()));
    if let Some(stream) = child.stdout.take() {
        let log = Arc::clone(&log);
        let buffer = Arc::clone(&stdout);
        thread::spawn(move || {
            read_stream_lines(stream, "stdout", buffer, log);
        });
    }
    if let Some(stream) = child.stderr.take() {
        let log = Arc::clone(&log);
        let buffer = Arc::clone(&stderr);
        thread::spawn(move || {
            read_stream_lines(stream, "stderr", buffer, log);
        });
    }
    loop {
        if should_stop() {
            let _ = child.kill();
            let _ = child.wait();
            let stdout = buffered_text(&stdout);
            let stderr = buffered_text(&stderr);
            return Ok(JobResult::Error {
                code: "task_stopped".to_string(),
                message: format!("task stopped; stdout={} stderr={}", stdout, stderr),
                retryable: false,
            });
        }
        if Instant::now() >= timeout_at {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(JobResult::Error {
                code: "command_timeout".to_string(),
                message: format!("command exceeded {} seconds", timeout_seconds.max(1)),
                retryable: false,
            });
        }
        if let Some(status) = child.try_wait()? {
            thread::sleep(Duration::from_millis(50));
            let stdout = buffered_text(&stdout);
            let stderr = buffered_text(&stderr);
            if operation == "command" || operation.starts_with("plugin.") {
                return Ok(JobResult::CommandResult {
                    exit_code: status.code(),
                    stdout,
                    stderr,
                    duration_ms: started.elapsed().as_millis() as u64,
                });
            }
            return Ok(JobResult::GitResult {
                operation,
                exit_code: status.code(),
                stdout,
                stderr,
                duration_ms: started.elapsed().as_millis() as u64,
            });
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn read_stream_lines<R: Read>(
    stream: R,
    stream_name: &'static str,
    buffer: Arc<std::sync::Mutex<String>>,
    log: LogCallback,
) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                if let Ok(mut text) = buffer.lock() {
                    text.push_str(&line);
                }
                log(stream_name, &line);
            }
            Err(_) => break,
        }
    }
}

fn buffered_text(buffer: &Arc<std::sync::Mutex<String>>) -> String {
    buffer.lock().map(|text| text.clone()).unwrap_or_default()
}

pub fn execute_file(payload: &FilePayload) -> Result<JobResult> {
    let started = Instant::now();
    match payload {
        FilePayload::Read { path, max_bytes } => {
            let mut file = fs::File::open(path)?;
            let limit = max_bytes.unwrap_or(65_536).min(5_242_880) as usize;
            let mut buffer = Vec::new();
            Read::by_ref(&mut file)
                .take(limit as u64)
                .read_to_end(&mut buffer)?;
            Ok(JobResult::FileResult {
                operation: "read".to_string(),
                path: path.clone(),
                content: Some(String::from_utf8_lossy(&buffer).into_owned()),
                entries: Vec::new(),
                bytes: buffer.len() as u64,
                duration_ms: started.elapsed().as_millis() as u64,
            })
        }
        FilePayload::Write {
            path,
            content,
            append,
            create_dirs,
        } => {
            if *create_dirs {
                if let Some(parent) = Path::new(path).parent() {
                    fs::create_dir_all(parent)?;
                }
            }
            if *append {
                use std::io::Write;
                let mut file = fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)?;
                file.write_all(content.as_bytes())?;
            } else {
                fs::write(path, content)?;
            }
            Ok(JobResult::FileResult {
                operation: "write".to_string(),
                path: path.clone(),
                content: None,
                entries: Vec::new(),
                bytes: content.len() as u64,
                duration_ms: started.elapsed().as_millis() as u64,
            })
        }
        FilePayload::List {
            path,
            recursive,
            max_entries,
        } => {
            let mut entries = Vec::new();
            collect_entries(
                Path::new(path),
                *recursive,
                max_entries.unwrap_or(200),
                &mut entries,
            )?;
            Ok(JobResult::FileResult {
                operation: "list".to_string(),
                path: path.clone(),
                content: None,
                bytes: entries.len() as u64,
                entries,
                duration_ms: started.elapsed().as_millis() as u64,
            })
        }
        FilePayload::Upload {
            path,
            content_base64,
            create_dirs,
        } => {
            if *create_dirs {
                if let Some(parent) = Path::new(path).parent() {
                    fs::create_dir_all(parent)?;
                }
            }
            let bytes = general_purpose::STANDARD
                .decode(content_base64)
                .map_err(|error| ExecutorError::InvalidInput(error.to_string()))?;
            fs::write(path, &bytes)?;
            Ok(JobResult::FileResult {
                operation: "upload".to_string(),
                path: path.clone(),
                content: None,
                entries: Vec::new(),
                bytes: bytes.len() as u64,
                duration_ms: started.elapsed().as_millis() as u64,
            })
        }
        FilePayload::Download { path, max_bytes } => {
            let mut file = fs::File::open(path)?;
            let limit = max_bytes.unwrap_or(5_242_880).min(20_971_520) as usize;
            let mut buffer = Vec::new();
            Read::by_ref(&mut file)
                .take(limit as u64)
                .read_to_end(&mut buffer)?;
            Ok(JobResult::FileResult {
                operation: "download".to_string(),
                path: path.clone(),
                content: Some(general_purpose::STANDARD.encode(&buffer)),
                entries: Vec::new(),
                bytes: buffer.len() as u64,
                duration_ms: started.elapsed().as_millis() as u64,
            })
        }
    }
}

fn collect_entries(
    path: &Path,
    recursive: bool,
    max_entries: u64,
    entries: &mut Vec<Value>,
) -> Result<()> {
    if entries.len() as u64 >= max_entries {
        return Ok(());
    }
    for item in fs::read_dir(path)? {
        if entries.len() as u64 >= max_entries {
            break;
        }
        let item = item?;
        let metadata = item.metadata()?;
        let item_path = item.path();
        entries.push(json!({
            "path": item_path.display().to_string(),
            "is_dir": metadata.is_dir(),
            "len": metadata.len()
        }));
        if recursive && metadata.is_dir() {
            collect_entries(&item_path, recursive, max_entries, entries)?;
        }
    }
    Ok(())
}

pub fn execute_git(payload: &GitPayload) -> Result<JobResult> {
    match payload {
        GitPayload::Clone {
            repo,
            dest,
            branch,
            depth,
        } => {
            let mut args = vec!["clone".to_string()];
            if let Some(branch) = branch {
                args.extend(["--branch".to_string(), branch.clone()]);
            }
            if let Some(depth) = depth {
                args.extend(["--depth".to_string(), depth.to_string()]);
            }
            args.extend([repo.clone(), dest.clone()]);
            run_command_result("git.clone".to_string(), "git", &args, None, 600)
        }
        GitPayload::Pull { repo_dir } => run_command_result(
            "git.pull".to_string(),
            "git",
            &["pull".to_string(), "--ff-only".to_string()],
            Some(repo_dir),
            300,
        ),
        GitPayload::Status { repo_dir } => run_command_result(
            "git.status".to_string(),
            "git",
            &["status".to_string(), "--short".to_string()],
            Some(repo_dir),
            60,
        ),
        GitPayload::Checkout {
            repo_dir,
            reference,
        } => run_command_result(
            "git.checkout".to_string(),
            "git",
            &["checkout".to_string(), reference.clone()],
            Some(repo_dir),
            120,
        ),
    }
}

pub fn execute_docker(payload: &DockerPayload) -> Result<JobResult> {
    let result = match payload {
        DockerPayload::Ps => run_command_result(
            "docker.ps".to_string(),
            "docker",
            &["ps".to_string(), "--format".to_string(), "json".to_string()],
            None,
            60,
        )?,
        DockerPayload::Images => run_command_result(
            "docker.images".to_string(),
            "docker",
            &[
                "images".to_string(),
                "--format".to_string(),
                "json".to_string(),
            ],
            None,
            60,
        )?,
        DockerPayload::Run {
            image,
            args,
            timeout_seconds,
        } => {
            let mut docker_args = vec!["run".to_string(), "--rm".to_string(), image.clone()];
            docker_args.extend(args.clone());
            run_command_result(
                "docker.run".to_string(),
                "docker",
                &docker_args,
                None,
                *timeout_seconds,
            )?
        }
    };
    match result {
        JobResult::GitResult {
            operation,
            exit_code,
            stdout,
            stderr,
            duration_ms,
        } => Ok(JobResult::DockerResult {
            operation,
            exit_code,
            stdout,
            stderr,
            duration_ms,
        }),
        other => Ok(other),
    }
}

pub fn execute_browser(payload: &BrowserPayload) -> Result<JobResult> {
    match payload {
        BrowserPayload::Fetch {
            url,
            timeout_seconds,
            max_response_bytes,
            ..
        } => {
            let started = Instant::now();
            let client = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs((*timeout_seconds).max(1)))
                .build()?;
            let response = client.get(url).send()?;
            let status_code = response.status().as_u16();
            let mut text = response.text()?;
            if text.len() > *max_response_bytes as usize {
                text.truncate(*max_response_bytes as usize);
            }
            let title = extract_title(&text);
            Ok(JobResult::BrowserResult {
                url: url.clone(),
                status_code,
                title,
                text,
                screenshot_path: None,
                duration_ms: started.elapsed().as_millis() as u64,
            })
        }
        BrowserPayload::Automate {
            url,
            actions,
            screenshot_path,
            timeout_seconds,
        } => execute_browser_automation(url, actions, screenshot_path.as_deref(), *timeout_seconds),
    }
}

fn execute_browser_automation(
    url: &str,
    actions: &[Value],
    screenshot_path: Option<&str>,
    timeout_seconds: u64,
) -> Result<JobResult> {
    execute_browser_automation_with_cancel(
        url,
        actions,
        screenshot_path,
        timeout_seconds,
        || false,
        Arc::new(|_, _| {}),
    )
}

fn execute_browser_automation_with_cancel<F>(
    url: &str,
    actions: &[Value],
    screenshot_path: Option<&str>,
    timeout_seconds: u64,
    should_stop: F,
    log: LogCallback,
) -> Result<JobResult>
where
    F: Fn() -> bool + Send + 'static,
{
    let started = Instant::now();
    let script = build_playwright_script(url, actions, screenshot_path);
    let args = vec!["-e".to_string(), script];
    let result = run_command_result_with_cancel(
        "browser.automate".to_string(),
        "node",
        &args,
        None,
        timeout_seconds,
        should_stop,
        log,
    )?;
    let JobResult::GitResult {
        exit_code,
        stdout,
        stderr,
        ..
    } = result
    else {
        return Ok(result);
    };
    if let Some(code) = exit_code {
        if code != 0 {
            return Ok(JobResult::Error {
                code: if stdout.contains("task stopped") || stderr.contains("task stopped") {
                    "task_stopped".to_string()
                } else {
                    "browser_automation_failed".to_string()
                },
                message: if stderr.trim().is_empty() {
                    stdout
                } else {
                    stderr
                },
                retryable: false,
            });
        }
    }
    let parsed = serde_json::from_str::<Value>(&stdout).unwrap_or_else(|_| json!({}));
    Ok(JobResult::BrowserResult {
        url: url.to_string(),
        status_code: parsed
            .get("status_code")
            .and_then(Value::as_u64)
            .unwrap_or(200) as u16,
        title: parsed
            .get("title")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        text: if stdout.trim().is_empty() {
            stderr
        } else {
            stdout
        },
        screenshot_path: screenshot_path.map(ToString::to_string),
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

fn build_playwright_script(url: &str, actions: &[Value], screenshot_path: Option<&str>) -> String {
    let url = serde_json::to_string(url).unwrap_or_else(|_| "\"\"".to_string());
    let actions = serde_json::to_string(actions).unwrap_or_else(|_| "[]".to_string());
    let screenshot_path =
        serde_json::to_string(&screenshot_path).unwrap_or_else(|_| "null".to_string());
    format!(
        r#"
(async () => {{
  let chromium;
  try {{ chromium = require('playwright').chromium; }}
  catch (_) {{ chromium = require('@playwright/test').chromium; }}
  const browser = await chromium.launch({{ headless: true }});
  const page = await browser.newPage();
  const response = await page.goto({url}, {{ waitUntil: 'domcontentloaded' }});
  const actions = {actions};
  for (const action of actions) {{
    if (action.type === 'click') await page.click(action.selector);
    if (action.type === 'fill') await page.fill(action.selector, action.value || '');
    if (action.type === 'press') await page.press(action.selector || 'body', action.key || 'Enter');
    if (action.type === 'wait') await page.waitForTimeout(action.ms || 1000);
    if (action.type === 'download') {{
      const [download] = await Promise.all([page.waitForEvent('download'), page.click(action.selector)]);
      await download.saveAs(action.path);
    }}
  }}
  const screenshotPath = {screenshot_path};
  if (screenshotPath) await page.screenshot({{ path: screenshotPath, fullPage: true }});
  const title = await page.title();
  const text = await page.locator('body').innerText().catch(() => '');
  console.log(JSON.stringify({{ status_code: response ? response.status() : 0, title, text }}));
  await browser.close();
}})().catch((error) => {{
  console.error(error && error.stack ? error.stack : String(error));
  process.exit(1);
}});
"#
    )
}

pub fn execute_session(payload: &SessionPayload) -> Result<JobResult> {
    execute_session_with_cancel(payload, || false)
}

pub fn execute_session_with_cancel<F>(payload: &SessionPayload, should_stop: F) -> Result<JobResult>
where
    F: Fn() -> bool + Send + 'static,
{
    execute_session_with_cancel_and_logs(payload, should_stop, Arc::new(|_, _| {}))
}

pub fn execute_session_with_cancel_and_logs<F>(
    payload: &SessionPayload,
    should_stop: F,
    log: LogCallback,
) -> Result<JobResult>
where
    F: Fn() -> bool + Send + 'static,
{
    match payload {
        SessionPayload::Run {
            session_id,
            program,
            args,
            working_dir,
            timeout_seconds,
        } => {
            let result = run_command_result_with_cancel(
                "session".to_string(),
                program,
                args,
                working_dir.as_deref(),
                *timeout_seconds,
                should_stop,
                log,
            )?;
            match result {
                JobResult::GitResult {
                    exit_code,
                    stdout,
                    stderr,
                    duration_ms,
                    ..
                } => Ok(JobResult::SessionResult {
                    session_id: session_id
                        .clone()
                        .unwrap_or_else(|| "session-once".to_string()),
                    exit_code,
                    stdout,
                    stderr,
                    duration_ms,
                }),
                other => Ok(other),
            }
        }
    }
}

fn docker_result_from_command_like(result: JobResult) -> JobResult {
    match result {
        JobResult::GitResult {
            operation,
            exit_code,
            stdout,
            stderr,
            duration_ms,
        } => JobResult::DockerResult {
            operation,
            exit_code,
            stdout,
            stderr,
            duration_ms,
        },
        other => other,
    }
}

fn extract_title(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let start = lower.find("<title>")? + "<title>".len();
    let end = lower[start..].find("</title>")? + start;
    Some(text[start..end].trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executes_command_payload() {
        let result = execute_command(&CommandPayload {
            program: "printf".to_string(),
            args: vec!["agentgrid".to_string()],
            working_dir: None,
            timeout_seconds: 5,
        })
        .expect("command should run");

        match result {
            JobResult::CommandResult {
                exit_code,
                stdout,
                stderr,
                ..
            } => {
                assert_eq!(exit_code, Some(0));
                assert_eq!(stdout, "agentgrid");
                assert!(stderr.is_empty());
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn rejects_custom_payload() {
        let result = execute(&JobPayload::Custom {
            name: "unknown".to_string(),
            value: serde_json::json!({}),
        });

        assert!(matches!(result, Err(ExecutorError::Unsupported)));
    }
}
