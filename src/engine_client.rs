use serde_json;
use std::sync::Arc;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

#[derive(Debug)]
struct EngineState {
    child: tokio::process::Child,
    stdin: tokio::process::ChildStdin,
    stdout: tokio::io::BufReader<tokio::process::ChildStdout>,
}

#[derive(Debug)]
pub struct EngineClient {
    state: Arc<Mutex<Option<EngineState>>>,
}

impl EngineClient {
    pub fn new() -> Self {
        EngineClient {
            state: Arc::new(Mutex::new(None)),
        }
    }
    pub async fn start(&self, path: String) -> Result<(), String> {
        let mut guard = self.state.lock().await;

        if guard.is_some() {
            return Ok(());
        }

        let mut child = tokio::process::Command::new(path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;

        let stdin = child.stdin.take().ok_or("Failed to open stdin")?;
        let stdout = tokio::io::BufReader::new(child.stdout.take().ok_or("Failed to open stdout")?);

        *guard = Some(EngineState {
            child,
            stdin,
            stdout,
        });
        Ok(())
    }

    pub async fn send_command(&self, json_cmd: String) -> Result<String, String> {
        let mut guard = self.state.lock().await;

        let state = guard.as_mut().ok_or("Engine not started")?;

        let mut cmd = json_cmd;
        if !cmd.ends_with('\n') {
            cmd.push('\n');
        };

        state
            .stdin
            .write_all(cmd.as_bytes())
            .await
            .map_err(|e| e.to_string())?;



        let mut buffer = String::new();
        loop {
            buffer.clear();
            state
                .stdout
                .read_line(&mut buffer)
                .await
                .map_err(|e| e.to_string())?;

            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&buffer) {
                if v.get("ok").is_some() {
                    return Ok(buffer.trim().to_string());
                }
            }
        }
    }

    pub async fn stop(&self) -> Result<(), String> {
        let mut guard = self.state.lock().await;

        if let Some(mut state) = guard.take() {
            state.child.kill().await.map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    // pub async fn is_running(&self) -> bool {
    //     self.state.lock().await.is_some()
    // }
}
