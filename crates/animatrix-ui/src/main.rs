use animatrix_ai::{ModelRequest, PolicyMode, Provider, TaskKind};
use animatrix_core::{AppResult, ChannelId, ProjectId, ProviderKind};
use animatrix_domain::{Channel, Project};
use animatrix_project::ProjectManager;
use animatrix_storage::{AppConfig, AppState, LocalStore};
use eframe::egui;
use std::io::{self, Write};

#[derive(Debug)]
struct DemoProvider;

impl Provider for DemoProvider {
    fn provider_kind(&self) -> ProviderKind {
        ProviderKind::Local
    }

    fn supports(&self, task: TaskKind) -> bool {
        matches!(task, TaskKind::Text | TaskKind::Image | TaskKind::Video)
    }

    fn run(&self, request: ModelRequest) -> animatrix_ai::ModelResponse {
        animatrix_ai::ModelResponse {
            job_id: "demo-job".to_string(),
            provider: self.provider_kind(),
            task: request.task,
            status: "queued".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkflowStatus {
    Ready,
    Running,
    Blocked,
    Complete,
}

#[derive(Debug, Clone)]
struct WorkflowStep {
    name: String,
    status: WorkflowStatus,
    detail: String,
}

#[derive(Debug, Clone)]
struct WorkflowArea {
    steps: Vec<WorkflowStep>,
    active_step: usize,
}

impl WorkflowArea {
    fn from_project(project: Option<&Project>) -> Self {
        let project_name = project.map(|p| p.name.as_str()).unwrap_or("New project");
        let mut steps = vec![
            WorkflowStep {
                name: "Brief + direction".to_string(),
                status: WorkflowStatus::Complete,
                detail: "Creative intent locked".to_string(),
            },
            WorkflowStep {
                name: "Script + storyboard".to_string(),
                status: WorkflowStatus::Running,
                detail: format!("Working on {}", project_name),
            },
            WorkflowStep {
                name: "Voice + style pass".to_string(),
                status: WorkflowStatus::Ready,
                detail: "Waiting for asset approval".to_string(),
            },
            WorkflowStep {
                name: "Render + export".to_string(),
                status: WorkflowStatus::Blocked,
                detail: "Needs final scene review".to_string(),
            },
        ];

        if project.is_none() {
            steps[1].status = WorkflowStatus::Ready;
            steps[2].status = WorkflowStatus::Ready;
            steps[3].status = WorkflowStatus::Ready;
            steps[1].detail = "Create a project to begin".to_string();
        }

        Self {
            steps,
            active_step: 1,
        }
    }
}

#[derive(Debug, Clone)]
struct StudioState {
    selected_channel_id: Option<ChannelId>,
    selected_project_id: Option<ProjectId>,
    selected_channel: Option<Channel>,
    selected_project: Option<Project>,
    workflow: WorkflowArea,
}

impl StudioState {
    fn from_app_state(state: &AppState) -> Self {
        Self::from_app_state_with_selection(state, None, None)
    }

    fn from_app_state_with_selection(
        state: &AppState,
        selected_channel_id: Option<ChannelId>,
        selected_project_id: Option<ProjectId>,
    ) -> Self {
        let selected_channel = selected_channel_id
            .and_then(|id| state.channels.iter().find(|channel| channel.id == id).cloned())
            .or_else(|| state.channels.first().cloned());

        let selected_project = selected_project_id
            .and_then(|id| state.projects.iter().find(|project| project.id == id).cloned())
            .or_else(|| state.projects.first().cloned());

        Self {
            selected_channel_id: selected_channel.as_ref().map(|channel| channel.id),
            selected_project_id: selected_project.as_ref().map(|project| project.id),
            selected_channel,
            selected_project: selected_project.clone(),
            workflow: WorkflowArea::from_project(selected_project.as_ref()),
        }
    }

    fn select_channel(&mut self, channel: Channel) {
        self.selected_channel_id = Some(channel.id);
        self.selected_channel = Some(channel.clone());
        self.workflow = WorkflowArea::from_project(self.selected_project.as_ref());
    }

    fn select_project(&mut self, project: Project) {
        self.selected_project_id = Some(project.id);
        self.selected_project = Some(project.clone());
        if self.selected_channel.is_none() {
            self.selected_channel = Some(Channel {
                id: animatrix_core::ChannelId(animatrix_core::generate_id()),
                name: "Unassigned".to_string(),
                brand: animatrix_domain::BrandProfile {
                    name: "Unassigned".to_string(),
                    tagline: "No channel selected".to_string(),
                    tone: "neutral".to_string(),
                },
                style: animatrix_domain::StyleProfile {
                    art_style: "default".to_string(),
                    color_palette: vec!["#111827".to_string()],
                    camera_style: "default".to_string(),
                    pacing: "medium".to_string(),
                },
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            });
            self.selected_channel_id = Some(self.selected_channel.as_ref().unwrap().id);
        }
        self.workflow = WorkflowArea::from_project(Some(&project));
    }

    fn advance_workflow(&mut self) {
        if self.workflow.active_step + 1 >= self.workflow.steps.len() {
            return;
        }

        let current = self.workflow.active_step;
        self.workflow.steps[current].status = WorkflowStatus::Complete;
        self.workflow.active_step += 1;
        let next = self.workflow.active_step;
        self.workflow.steps[next].status = WorkflowStatus::Running;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CliCommand {
    Help,
    List,
    Advance,
    Render,
    SelectProject(usize),
    SelectChannel(usize),
    NewProject(String),
    NewChannel(String),
    Quit,
}

fn parse_command(input: &str, channel_count: usize, project_count: usize) -> Result<CliCommand, String> {
    let normalized = input.trim();
    if normalized.is_empty() {
        return Ok(CliCommand::List);
    }

    let lower = normalized.to_ascii_lowercase();
    match lower.as_str() {
        "help" => Ok(CliCommand::Help),
        "list" => Ok(CliCommand::List),
        "advance" => Ok(CliCommand::Advance),
        "render" => Ok(CliCommand::Render),
        "quit" | "exit" => Ok(CliCommand::Quit),
        _ => {
            if let Some(rest) = normalized.strip_prefix("project ") {
                let trimmed = rest.trim();
                if trimmed.is_empty() {
                    return Err("project requires a name".to_string());
                }
                let index = trimmed.parse::<usize>();
                if let Ok(index) = index {
                    if index >= project_count {
                        return Err(format!("project index out of range: {}", index));
                    }
                    return Ok(CliCommand::SelectProject(index));
                }
                return Ok(CliCommand::NewProject(trimmed.to_string()));
            }

            if let Some(rest) = normalized.strip_prefix("channel ") {
                let trimmed = rest.trim();
                if trimmed.is_empty() {
                    return Err("channel requires a name".to_string());
                }
                let index = trimmed.parse::<usize>();
                if let Ok(index) = index {
                    if index >= channel_count {
                        return Err(format!("channel index out of range: {}", index));
                    }
                    return Ok(CliCommand::SelectChannel(index));
                }
                return Ok(CliCommand::NewChannel(trimmed.to_string()));
            }

            if let Some(rest) = normalized.strip_prefix("new project ") {
                let trimmed = rest.trim();
                if trimmed.is_empty() {
                    return Err("new project requires a name".to_string());
                }
                return Ok(CliCommand::NewProject(trimmed.to_string()));
            }

            if let Some(rest) = normalized.strip_prefix("new channel ") {
                let trimmed = rest.trim();
                if trimmed.is_empty() {
                    return Err("new channel requires a name".to_string());
                }
                return Ok(CliCommand::NewChannel(trimmed.to_string()));
            }

            Err(format!("unknown command: {}", normalized))
        }
    }
}

struct AppRuntime {
    config: AppConfig,
    store: LocalStore,
    ui_state: StudioState,
}

impl AppRuntime {
    async fn bootstrap(config: &AppConfig) -> AppResult<Self> {
        let store = LocalStore::open_with_config(config, &config.database_url()).await?;
        let mut state = store.load_state().await?;

        if state.channels.is_empty() {
            let channel = ProjectManager::new_channel("Animatrix Studio", "AI video production");
            let project = ProjectManager::create_project(&channel, "Pilot Episode", "Long-form explainer pipeline");

            store.save_channel(&channel).await?;
            store.save_project(&project).await?;

            state.channels.push(channel.clone());
            state.projects.push(project.clone());
        }

        let ui_state = StudioState::from_app_state(&state);

        Ok(Self {
            config: config.clone(),
            store,
            ui_state,
        })
    }

    fn config(&self) -> &AppConfig {
        &self.config
    }

    fn ui_state(&self) -> &StudioState {
        &self.ui_state
    }

    async fn refresh_state(&mut self) -> AppResult<()> {
        let state = self.store.load_state().await?;
        self.ui_state = StudioState::from_app_state_with_selection(
            &state,
            self.ui_state.selected_channel_id,
            self.ui_state.selected_project_id,
        );
        Ok(())
    }

    fn render_help() {
        println!("Commands:");
        println!("  help                          show this menu");
        println!("  list                          show dashboard");
        println!("  advance                       move workflow to next step");
        println!("  render                        complete the current render and emit output asset");
        println!("  project <index>               select a project by index");
        println!("  channel <index>               select a channel by index");
        println!("  new project <name>            create a project and persist it");
        println!("  new channel <name>            create a channel and persist it");
        println!("  quit                          exit");
    }

    async fn handle_command(&mut self, input: &str) -> AppResult<bool> {
        let state = self.store.load_state().await?;
        let channel_count = state.channels.len();
        let project_count = state.projects.len();
        let command = parse_command(input, channel_count, project_count).map_err(|err| animatrix_core::AppError::new("cli_command", err))?;

        match command {
            CliCommand::Help => Self::render_help(),
            CliCommand::List => print_dashboard(&self.config, &StudioState::from_app_state(&state)),
            CliCommand::Advance => self.ui_state.advance_workflow(),
            CliCommand::Render => {
                let selected_project = self.ui_state.selected_project.clone().or_else(|| state.projects.first().cloned());
                let selected_channel = self.ui_state.selected_channel.clone().or_else(|| state.channels.first().cloned());

                if let (Some(project), Some(channel)) = (selected_project, selected_channel) {
                    let output_dir = self.config.data_dir.join("outputs");
                    let _ = std::fs::create_dir_all(&output_dir);
                    let output_path = output_dir.join(format!("render-{}.mp4", uuid::Uuid::new_v4()));
                    std::fs::write(&output_path, b"rendered output").unwrap();

                    let outcome = ProjectManager::complete_render(&project, &channel, &animatrix_jobs::Job::new(project.id, "render_export"), &output_path).unwrap();
                    println!("RENDER COMPLETE: {} -> {:?}", outcome.asset.path, outcome.event.event_type);
                    self.ui_state.workflow.steps.last_mut().unwrap().status = WorkflowStatus::Complete;
                    self.ui_state.workflow.active_step = self.ui_state.workflow.steps.len().saturating_sub(1);
                } else {
                    println!("No project or channel selected for render.");
                }
            }
            CliCommand::SelectProject(index) => {
                if let Some(project) = state.projects.get(index).cloned() {
                    self.ui_state.select_project(project);
                }
            }
            CliCommand::SelectChannel(index) => {
                if let Some(channel) = state.channels.get(index).cloned() {
                    self.ui_state.select_channel(channel);
                }
            }
            CliCommand::NewChannel(name) => {
                let channel = ProjectManager::new_channel(&name, "Created from CLI");
                self.store.save_channel(&channel).await?;
                self.ui_state.select_channel(channel);
                self.refresh_state().await?;
            }
            CliCommand::NewProject(name) => {
                let selected_channel = self.ui_state.selected_channel.clone().or_else(|| state.channels.first().cloned());
                let channel = selected_channel.unwrap_or_else(|| {
                    ProjectManager::new_channel("Default Channel", "Created from CLI")
                });
                let project = ProjectManager::create_project(&channel, &name, "Created from CLI");
                self.store.save_project(&project).await?;
                self.ui_state.select_project(project);
                self.refresh_state().await?;
            }
            CliCommand::Quit => return Ok(false),
        }

        Ok(true)
    }

    async fn run_cli(&mut self) -> AppResult<()> {
        println!("Type 'help' for commands.");
        loop {
            print!("animatrix> ");
            io::stdout().flush().map_err(|e| animatrix_core::AppError::new("stdout_flush", e.to_string()))?;

            let mut input = String::new();
            if io::stdin().read_line(&mut input).map_err(|e| animatrix_core::AppError::new("stdin_read", e.to_string()))? == 0 {
                break;
            }

            let should_continue = self.handle_command(&input).await?;
            if !should_continue {
                break;
            }
        }

        Ok(())
    }
}

fn render_workflow(workflow: &WorkflowArea) {
    println!("WORKFLOW AREA");
    for (index, step) in workflow.steps.iter().enumerate() {
        let marker = if index == workflow.active_step { "=>" } else { "  " };
        let status = match step.status {
            WorkflowStatus::Complete => "COMPLETE",
            WorkflowStatus::Running => "RUNNING",
            WorkflowStatus::Blocked => "BLOCKED",
            WorkflowStatus::Ready => "READY",
        };
        println!("{} [{}] {} - {}", marker, status, step.name, step.detail);
    }
}

fn print_dashboard(config: &AppConfig, ui_state: &StudioState) {
    let channel = ui_state.selected_channel.as_ref();
    let project = ui_state.selected_project.as_ref();

    println!("============================================================");
    println!("ANIMATRIX :: LOCAL STUDIO DASHBOARD");
    println!("============================================================");
    println!("Workspace: {}", config.data_dir.display());
    println!("Database: {}", config.database_path.display());
    println!();

    println!("SELECTION");
    if let Some(channel) = channel {
        println!("- Channel: {}", channel.name);
        println!("- Brand: {}", channel.brand.name);
    } else {
        println!("- Channel: none");
    }

    if let Some(project) = project {
        println!("- Project: {}", project.name);
        println!("- Summary: {}", project.description);
    } else {
        println!("- Project: none");
    }
    println!();

    render_workflow(&ui_state.workflow);
    println!();

    println!("PROJECTS");
    if ui_state.selected_project.is_some() {
        for project in [ui_state.selected_project.as_ref()].into_iter().flatten() {
            println!("- {}", project.name);
        }
    } else {
        println!("- none");
    }

    println!();
    println!("CHANNELS");
    if ui_state.selected_channel.is_some() {
        for channel in [ui_state.selected_channel.as_ref()].into_iter().flatten() {
            println!("- {}", channel.name);
        }
    } else {
        println!("- none");
    }
    println!("============================================================");
}

struct StudioWindowApp {
    config: AppConfig,
    state: AppState,
    selected_channel: Option<Channel>,
    selected_project: Option<Project>,
}

impl StudioWindowApp {
    fn new(config: AppConfig, state: AppState) -> Self {
        let selected_channel = state.channels.first().cloned();
        let selected_project = state.projects.first().cloned();

        Self {
            config,
            state,
            selected_channel,
            selected_project,
        }
    }
}

impl eframe::App for StudioWindowApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("ANIMATRIX :: STUDIO");
            ui.label(format!("Workspace: {}", self.config.data_dir.display()));
            ui.label(format!("Database: {}", self.config.database_path.display()));
            ui.separator();

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label("Channels");
                    for channel in &self.state.channels {
                        let is_selected = self.selected_channel.as_ref().map(|c| c.id == channel.id).unwrap_or(false);
                        if ui.selectable_label(is_selected, &channel.name).clicked() {
                            self.selected_channel = Some(channel.clone());
                        }
                    }
                });

                ui.vertical(|ui| {
                    ui.label("Projects");
                    for project in &self.state.projects {
                        let is_selected = self.selected_project.as_ref().map(|p| p.id == project.id).unwrap_or(false);
                        if ui.selectable_label(is_selected, &project.name).clicked() {
                            self.selected_project = Some(project.clone());
                            if self.selected_channel.is_none() {
                                self.selected_channel = self.state.channels.first().cloned();
                            }
                        }
                    }
                });
            });

            ui.separator();
            ui.label("Workflow");
            let workflow = WorkflowArea::from_project(self.selected_project.as_ref());
            for (index, step) in workflow.steps.iter().enumerate() {
                let status = match step.status {
                    WorkflowStatus::Complete => "COMPLETE",
                    WorkflowStatus::Running => "RUNNING",
                    WorkflowStatus::Blocked => "BLOCKED",
                    WorkflowStatus::Ready => "READY",
                };
                let prefix = if index == workflow.active_step { "=>" } else { "  " };
                ui.label(format!("{} [{}] {} - {}", prefix, status, step.name, step.detail));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn studio_state_builds_default_workflow() {
        let state = AppState {
            channels: vec![],
            projects: vec![],
        };

        let ui_state = StudioState::from_app_state(&state);
        assert_eq!(ui_state.workflow.steps.len(), 4);
        assert_eq!(ui_state.workflow.active_step, 1);
        assert_eq!(ui_state.workflow.steps[1].status, WorkflowStatus::Ready);
    }

    #[test]
    fn refresh_preserves_selected_items_by_id() {
        let now = chrono::Utc::now();
        let channel = Channel {
            id: ChannelId(animatrix_core::generate_id()),
            name: "Brand One".to_string(),
            brand: animatrix_domain::BrandProfile {
                name: "Brand One".to_string(),
                tagline: "tagline".to_string(),
                tone: "bold".to_string(),
            },
            style: animatrix_domain::StyleProfile {
                art_style: "explainer".to_string(),
                color_palette: vec!["#000000".to_string()],
                camera_style: "center".to_string(),
                pacing: "medium".to_string(),
            },
            created_at: now,
            updated_at: now,
        };
        let project = Project {
            id: ProjectId(animatrix_core::generate_id()),
            channel_id: channel.id,
            name: "Story Arc".to_string(),
            description: "Persisted narrative".to_string(),
            created_at: now,
            updated_at: now,
        };

        let state = AppState { channels: vec![channel.clone()], projects: vec![project.clone()] };
        let ui_state = StudioState::from_app_state_with_selection(&state, Some(channel.id), Some(project.id));

        assert_eq!(ui_state.selected_channel.as_ref().map(|c| c.name.as_str()), Some("Brand One"));
        assert_eq!(ui_state.selected_project.as_ref().map(|p| p.name.as_str()), Some("Story Arc"));
    }

    #[test]
    fn advance_workflow_updates_status_and_selection() {
        let mut ui_state = StudioState {
            selected_channel_id: None,
            selected_project_id: None,
            selected_channel: None,
            selected_project: None,
            workflow: WorkflowArea::from_project(None),
        };

        ui_state.advance_workflow();
        assert_eq!(ui_state.workflow.active_step, 2);
        assert_eq!(ui_state.workflow.steps[1].status, WorkflowStatus::Complete);
        assert_eq!(ui_state.workflow.steps[2].status, WorkflowStatus::Running);
    }

    #[test]
    fn parse_command_handles_project_and_channel_selection() {
        assert_eq!(parse_command("project 0", 2, 3).unwrap(), CliCommand::SelectProject(0));
        assert_eq!(parse_command("channel 1", 2, 3).unwrap(), CliCommand::SelectChannel(1));
        assert_eq!(parse_command("advance", 1, 1).unwrap(), CliCommand::Advance);
        assert_eq!(parse_command("render", 1, 1).unwrap(), CliCommand::Render);
        assert_eq!(parse_command("new project Story Arc", 1, 1).unwrap(), CliCommand::NewProject("Story Arc".to_string()));
        assert_eq!(parse_command("new channel Brand One", 1, 1).unwrap(), CliCommand::NewChannel("Brand One".to_string()));
    }
}

#[tokio::main]
async fn main() {
    let cli_mode = std::env::args().any(|arg| arg == "--cli");

    if cli_mode {
        let config = AppConfig::default().expect("failed to resolve config directory");
        let mut runtime = AppRuntime::bootstrap(&config)
            .await
            .expect("failed to initialize app runtime");

        runtime.refresh_state().await.expect("failed to refresh app state");
        let ui_state = runtime.ui_state();

        let provider = DemoProvider;
        let request = ModelRequest {
            task: TaskKind::Image,
            preferred_provider: Some(ProviderKind::Local),
            policy: PolicyMode::LocalFirst,
            prompt: "Create a storyboard frame for a professional explainer scene".to_string(),
            created_at: chrono::Utc::now(),
        };

        let response = provider.run(request.clone());
        print_dashboard(runtime.config(), ui_state);
        println!("AI JOB: {} [{}]", response.job_id, response.status);
        println!("AI TASK: {:?}", response.task);
        println!("MODEL POLICY: {:?}", request.policy);
        println!("PROVIDER: {:?}", response.provider);

        runtime.run_cli().await.expect("interactive app loop failed");
        return;
    }

    let config = AppConfig::default().expect("failed to resolve config directory");
    let store = LocalStore::open_with_config(&config, &config.database_url())
        .await
        .expect("failed to open studio store");
    let state = store.load_state().await.expect("failed to load studio state");
    let app = StudioWindowApp::new(config, state);

    let options = eframe::NativeOptions::default();
    eframe::run_native("Animatrix Studio", options, Box::new(|_cc| Ok(Box::new(app))))
        .expect("failed to start Animatrix window");
}
