use std::io::{self, BufRead, Write};

use agentgrid_sdk::{AgentGridClient, DEFAULT_HUB_URL};
use anyhow::Result;
use clap::Parser;
use serde_json::{json, Value};

#[derive(Debug, Parser)]
#[command(name = "agentgrid-mcp")]
#[command(about = "AgentGrid MCP Server v1 over stdio")]
struct Cli {
    #[arg(long, default_value = DEFAULT_HUB_URL)]
    hub: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let client = AgentGridClient::new(cli.hub);
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = serde_json::from_str(&line)?;
        let response = handle_request(&client, request);
        writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
        stdout.flush()?;
    }
    Ok(())
}

fn handle_request(client: &AgentGridClient, request: Value) -> Value {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": { "name": "agentgrid-mcp", "version": "0.1.0" },
            "capabilities": { "tools": {} }
        })),
        "tools/list" => Ok(json!({ "tools": mcp_tools() })),
        "tools/call" => call_tool(client, request.get("params").cloned().unwrap_or_default()),
        "notifications/initialized" => return json!({}),
        _ => Err(format!("unsupported method: {method}")),
    };
    match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(message) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32000, "message": message }
        }),
    }
}

fn mcp_tools() -> Vec<Value> {
    vec![
        tool(
            "agentgrid_runtime_manifest",
            "读取 AgentGrid AI Runtime manifest。",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "agentgrid_list_tools",
            "列出 AgentGrid ToolContract 工具目录。",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "agentgrid_list_nodes",
            "列出集群节点和资源状态。",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "agentgrid_list_task_templates",
            "列出 AgentGrid 任务模板商店。",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "agentgrid_start_task_template",
            "启动一个标准任务模板。",
            json!({
                "type": "object",
                "required": ["template_id"],
                "properties": {
                    "template_id": { "type": "string" },
                    "parameters": { "type": "object" },
                    "title": { "type": "string" },
                    "node_id": { "type": "string" },
                    "os": { "type": "string" }
                }
            }),
        ),
        tool(
            "agentgrid_submit_task",
            "按 tool_id + payload 提交标准 Runtime 任务。",
            json!({
                "type": "object",
                "required": ["tool_id", "payload"],
                "properties": {
                    "tool_id": { "type": "string" },
                    "payload": { "type": "object" },
                    "title": { "type": "string" },
                    "node_id": { "type": "string" },
                    "verify": { "type": "object" }
                }
            }),
        ),
        tool(
            "agentgrid_get_task",
            "查询 Runtime 任务快照。",
            json!({
                "type": "object",
                "required": ["task_id"],
                "properties": { "task_id": { "type": "string" } }
            }),
        ),
        tool(
            "agentgrid_run_command",
            "提交命令任务并返回任务编号。",
            json!({
                "type": "object",
                "required": ["program"],
                "properties": {
                    "program": { "type": "string" },
                    "args": { "type": "array", "items": { "type": "string" } },
                    "node_id": { "type": "string" },
                    "title": { "type": "string" }
                }
            }),
        ),
        tool(
            "agentgrid_run_plugin",
            "提交 Worker 插件任务并返回任务编号。",
            json!({
                "type": "object",
                "required": ["plugin_id"],
                "properties": {
                    "plugin_id": { "type": "string" },
                    "action": { "type": "string" },
                    "input": { "type": "object" },
                    "node_id": { "type": "string" },
                    "title": { "type": "string" }
                }
            }),
        ),
        tool(
            "agentgrid_list_webhooks",
            "列出任务回调 Webhook 订阅。",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "agentgrid_create_webhook",
            "创建任务回调 Webhook 订阅。",
            json!({
                "type": "object",
                "required": ["name", "url"],
                "properties": {
                    "name": { "type": "string" },
                    "url": { "type": "string" },
                    "events": { "type": "array", "items": { "type": "string" } },
                    "secret": { "type": "string" },
                    "enabled": { "type": "boolean" }
                }
            }),
        ),
    ]
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({ "name": name, "description": description, "inputSchema": input_schema })
}

fn call_tool(client: &AgentGridClient, params: Value) -> std::result::Result<Value, String> {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let value = match name {
        "agentgrid_runtime_manifest" => client.runtime_manifest(),
        "agentgrid_list_tools" => client.tools(),
        "agentgrid_list_nodes" => client.nodes(),
        "agentgrid_list_task_templates" => client.task_templates(),
        "agentgrid_start_task_template" => {
            let template_id = args
                .get("template_id")
                .and_then(Value::as_str)
                .unwrap_or("");
            let request = json!({
                "parameters": args.get("parameters").cloned().unwrap_or_else(|| json!({})),
                "title": args.get("title").and_then(Value::as_str),
                "node_id": args.get("node_id").and_then(Value::as_str),
                "os": args.get("os").and_then(Value::as_str),
                "created_by": "agentgrid-mcp"
            });
            client.start_task_template(template_id, request)
        }
        "agentgrid_submit_task" => client.submit_runtime_task(args),
        "agentgrid_get_task" => {
            let task_id = args.get("task_id").and_then(Value::as_str).unwrap_or("");
            client.get_runtime_task(task_id)
        }
        "agentgrid_run_command" => {
            let program = args.get("program").and_then(Value::as_str).unwrap_or("");
            let cmd_args = args
                .get("args")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect();
            let node_id = args
                .get("node_id")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            let title = args
                .get("title")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            client.submit_command(program, cmd_args, node_id, title)
        }
        "agentgrid_run_plugin" => {
            let plugin_id = args.get("plugin_id").and_then(Value::as_str).unwrap_or("");
            let action = args.get("action").and_then(Value::as_str).unwrap_or("run");
            let input = args.get("input").cloned().unwrap_or_else(|| json!({}));
            let node_id = args
                .get("node_id")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            let title = args
                .get("title")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            client.submit_plugin(plugin_id, action, input, node_id, title)
        }
        "agentgrid_list_webhooks" => client.webhooks(),
        "agentgrid_create_webhook" => client.create_webhook(args),
        _ => return Err(format!("unknown tool: {name}")),
    }
    .map_err(|error| error.to_string())?;
    Ok(json!({
        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()) }]
    }))
}
