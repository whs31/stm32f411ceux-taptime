use std::{
    error::Error,
    fmt, io,
    time::{Duration, Instant},
};

mod credentials;

use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use chrono::{NaiveDate, TimeZone, Timelike, Utc};
use clap::{Parser, Subcommand};
use credentials::{CredentialEndpoint, CredentialError, CredentialStore, SystemCredentialStore};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use rand_core::OsRng;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap},
};
use taptime_schema::{
    Day, DayFlag, User, Uuid as ProtoUuid,
    balance::BalanceType,
    event::EventType,
    services::{
        AdminLoginRequest, AdminUserDetail, AdminUserStats, BanKind, BanRecord, CreateBanRequest,
        DaySummary, DeleteUserRequest, GetUserStatsRequest, ListBansRequest, ListUsersRequest,
        MonthlyStats, RevokeBanRequest, admin_service_client::AdminServiceClient,
    },
};
use tonic::{Request, metadata::MetadataValue, transport::Channel};
use uuid::Uuid;
use zeroize::Zeroizing;

type AppResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Parser)]
#[command(name = "taptime_admin_cli")]
#[command(version, about = "TapTime administration TUI")]
struct Args {
    #[arg(
        long,
        env = "ADMIN_API_URL",
        default_value = "http://127.0.0.1:50051",
        global = true
    )]
    admin_api_url: String,

    /// Do not read or write the operating system credential store.
    #[arg(long, global = true)]
    no_password_cache: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    HashPassword {
        #[arg(long)]
        password: Option<String>,
    },
    /// Remove the saved password for the selected admin API URL.
    ForgetPassword,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Tab {
    Users,
    Bans,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UserPane {
    Account,
    Stats,
}

impl UserPane {
    fn toggled(self) -> Self {
        match self {
            Self::Account => Self::Stats,
            Self::Stats => Self::Account,
        }
    }
}

#[derive(Clone, Debug)]
enum Mode {
    Normal,
    Search { input: String },
    BanIp { input: String },
    Confirm { action: Action, input: String },
}

#[derive(Clone, Debug)]
enum Action {
    BanUser(ProtoUuid),
    BanIp(String),
    RevokeBan(BanKind, ProtoUuid),
    DeleteData(ProtoUuid),
    DeleteAccount(ProtoUuid),
}

impl Action {
    fn expected(&self) -> &'static str {
        match self {
            Self::BanUser(_) => "BAN USER",
            Self::BanIp(_) => "BAN IP",
            Self::RevokeBan(_, _) => "REVOKE BAN",
            Self::DeleteData(_) => "DELETE DATA",
            Self::DeleteAccount(_) => "DELETE ACCOUNT",
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::BanUser(_) => "Ban selected user",
            Self::BanIp(_) => "Ban IP/CIDR",
            Self::RevokeBan(_, _) => "Revoke selected ban",
            Self::DeleteData(_) => "Delete selected user's time data",
            Self::DeleteAccount(_) => "Delete selected user account",
        }
    }
}

struct App {
    client: AdminClient,
    tab: Tab,
    mode: Mode,
    users: Vec<taptime_schema::services::AdminUserListItem>,
    bans: Vec<BanRecord>,
    detail: Option<AdminUserDetail>,
    stats: Option<AdminUserStats>,
    user_pane: UserPane,
    stats_scroll: u16,
    stats_last_loaded: Option<Instant>,
    query: String,
    selected_user: usize,
    selected_ban: usize,
    status: String,
    should_quit: bool,
}

impl App {
    async fn new(client: AdminClient) -> AppResult<Self> {
        let mut app = Self {
            client,
            tab: Tab::Users,
            mode: Mode::Normal,
            users: Vec::new(),
            bans: Vec::new(),
            detail: None,
            stats: None,
            user_pane: UserPane::Account,
            stats_scroll: 0,
            stats_last_loaded: None,
            query: String::new(),
            selected_user: 0,
            selected_ban: 0,
            status: String::from("Ready"),
            should_quit: false,
        };
        app.refresh().await?;
        Ok(app)
    }

    async fn refresh(&mut self) -> AppResult<()> {
        let users = self.client.list_users(&self.query).await?;
        self.users = users.users;
        if self.selected_user >= self.users.len() {
            self.selected_user = self.users.len().saturating_sub(1);
        }
        self.bans = self.client.list_bans(false).await?;
        if self.selected_ban >= self.bans.len() {
            self.selected_ban = self.bans.len().saturating_sub(1);
        }
        self.load_selected_user().await?;
        self.status = "Refreshed".to_string();
        Ok(())
    }

    async fn load_selected_user(&mut self) -> AppResult<()> {
        self.load_selected_detail().await?;
        self.stats_scroll = 0;
        if self.user_pane == UserPane::Stats {
            self.load_selected_stats().await?;
        } else {
            self.stats = None;
            self.stats_last_loaded = None;
        }
        Ok(())
    }

    async fn load_selected_detail(&mut self) -> AppResult<()> {
        let Some(user_id) = self.selected_user_id() else {
            self.detail = None;
            self.stats = None;
            self.stats_last_loaded = None;
            return Ok(());
        };
        self.detail = Some(self.client.get_user_detail(user_id).await?);
        Ok(())
    }

    async fn load_selected_stats(&mut self) -> AppResult<()> {
        let Some(user_id) = self.selected_user_id() else {
            self.stats = None;
            self.stats_last_loaded = None;
            return Ok(());
        };
        let stats = self.client.get_user_stats(user_id).await?;
        self.status = format!("Stats synchronized at {}", fmt_ts(stats.generated_at));
        self.stats = Some(stats);
        self.stats_last_loaded = Some(Instant::now());
        self.clamp_stats_scroll();
        Ok(())
    }

    fn stats_refresh_due(&self) -> bool {
        self.tab == Tab::Users
            && self.user_pane == UserPane::Stats
            && self
                .stats_last_loaded
                .is_none_or(|loaded| loaded.elapsed() >= Duration::from_secs(30))
    }

    fn clamp_stats_scroll(&mut self) {
        let line_count = self.stats.as_ref().map(stats_line_count).unwrap_or(0);
        self.stats_scroll = clamp_scroll(self.stats_scroll, line_count);
    }

    fn selected_user_id(&self) -> Option<ProtoUuid> {
        self.users.get(self.selected_user)?.user.as_ref()?.id
    }

    fn selected_ban(&self) -> Option<&BanRecord> {
        self.bans.get(self.selected_ban)
    }

    async fn handle_key(&mut self, key: KeyEvent) -> AppResult<()> {
        match &mut self.mode {
            Mode::Search { input } => match key.code {
                KeyCode::Esc => self.mode = Mode::Normal,
                KeyCode::Enter => {
                    self.query = input.trim().to_string();
                    self.mode = Mode::Normal;
                    self.refresh().await?;
                }
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(ch) => input.push(ch),
                _ => {}
            },
            Mode::BanIp { input } => match key.code {
                KeyCode::Esc => self.mode = Mode::Normal,
                KeyCode::Enter => {
                    let ip = input.trim().to_string();
                    if !ip.is_empty() {
                        self.mode = Mode::Confirm {
                            action: Action::BanIp(ip),
                            input: String::new(),
                        };
                    }
                }
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(ch) => input.push(ch),
                _ => {}
            },
            Mode::Confirm { action, input } => match key.code {
                KeyCode::Esc => self.mode = Mode::Normal,
                KeyCode::Enter => {
                    if input == action.expected() {
                        let action = action.clone();
                        self.mode = Mode::Normal;
                        self.perform(action).await?;
                    } else {
                        self.status = format!("Confirmation must be exactly {}", action.expected());
                    }
                }
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(ch) => input.push(ch),
                _ => {}
            },
            Mode::Normal => self.handle_normal_key(key).await?,
        }
        Ok(())
    }

    async fn handle_normal_key(&mut self, key: KeyEvent) -> AppResult<()> {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Tab => {
                self.tab = if self.tab == Tab::Users {
                    Tab::Bans
                } else {
                    Tab::Users
                }
            }
            KeyCode::Char('/') => {
                self.mode = Mode::Search {
                    input: self.query.clone(),
                };
            }
            KeyCode::Char('p') => {
                self.mode = Mode::BanIp {
                    input: String::new(),
                };
            }
            KeyCode::Char('r') => self.refresh().await?,
            KeyCode::Enter if self.tab == Tab::Users => self.load_selected_detail().await?,
            KeyCode::Char('s') if self.tab == Tab::Users => {
                self.user_pane = self.user_pane.toggled();
                self.stats_scroll = 0;
                if self.user_pane == UserPane::Stats {
                    self.load_selected_stats().await?;
                } else {
                    self.status = "Showing account details".to_string();
                }
            }
            KeyCode::PageUp if self.tab == Tab::Users && self.user_pane == UserPane::Stats => {
                self.stats_scroll = self.stats_scroll.saturating_sub(6);
            }
            KeyCode::PageDown if self.tab == Tab::Users && self.user_pane == UserPane::Stats => {
                self.stats_scroll = self.stats_scroll.saturating_add(6);
                self.clamp_stats_scroll();
            }
            KeyCode::Char('b') if self.tab == Tab::Users => {
                if let Some(user_id) = self.selected_user_id() {
                    self.mode = Mode::Confirm {
                        action: Action::BanUser(user_id),
                        input: String::new(),
                    };
                }
            }
            KeyCode::Char('d') if self.tab == Tab::Users => {
                if let Some(user_id) = self.selected_user_id() {
                    self.mode = Mode::Confirm {
                        action: Action::DeleteData(user_id),
                        input: String::new(),
                    };
                }
            }
            KeyCode::Char('x') if self.tab == Tab::Users => {
                if let Some(user_id) = self.selected_user_id() {
                    self.mode = Mode::Confirm {
                        action: Action::DeleteAccount(user_id),
                        input: String::new(),
                    };
                }
            }
            KeyCode::Char('u') if self.tab == Tab::Bans => {
                if let Some(ban) = self.selected_ban()
                    && let Some(ban_id) = ban.id
                {
                    let kind = BanKind::try_from(ban.kind).unwrap_or(BanKind::Unspecified);
                    self.mode = Mode::Confirm {
                        action: Action::RevokeBan(kind, ban_id),
                        input: String::new(),
                    };
                }
            }
            KeyCode::Up => self.move_selection(-1).await?,
            KeyCode::Down => self.move_selection(1).await?,
            _ => {}
        }
        Ok(())
    }

    async fn move_selection(&mut self, delta: isize) -> AppResult<()> {
        match self.tab {
            Tab::Users => {
                self.selected_user = move_index(self.selected_user, self.users.len(), delta);
                self.load_selected_user().await?;
            }
            Tab::Bans => {
                self.selected_ban = move_index(self.selected_ban, self.bans.len(), delta);
            }
        }
        Ok(())
    }

    async fn perform(&mut self, action: Action) -> AppResult<()> {
        match action {
            Action::BanUser(user_id) => {
                self.client.create_user_ban(user_id).await?;
                self.status = "User banned".to_string();
            }
            Action::BanIp(ip_cidr) => {
                self.client.create_ip_ban(ip_cidr).await?;
                self.status = "IP/CIDR banned".to_string();
            }
            Action::RevokeBan(kind, ban_id) => {
                self.client.revoke_ban(kind, ban_id).await?;
                self.status = "Ban revoked".to_string();
            }
            Action::DeleteData(user_id) => {
                self.client.delete_user_time_data(user_id).await?;
                self.status = "Time data deleted".to_string();
            }
            Action::DeleteAccount(user_id) => {
                self.client.delete_user_account(user_id).await?;
                self.status = "Account deleted".to_string();
            }
        }
        self.refresh().await?;
        Ok(())
    }
}

struct AdminClient {
    inner: AdminServiceClient<Channel>,
    token: String,
}

impl AdminClient {
    async fn connect(endpoint: String, password: String) -> Result<Self, AdminConnectError> {
        let mut inner = AdminServiceClient::connect(endpoint)
            .await
            .map_err(AdminConnectError::Transport)?;
        let response = inner
            .admin_login(AdminLoginRequest { password })
            .await
            .map_err(AdminConnectError::Login)?
            .into_inner();
        Ok(Self {
            inner,
            token: response.admin_token,
        })
    }

    fn request<T>(&self, message: T) -> Result<Request<T>, tonic::Status> {
        let mut request = Request::new(message);
        let header = format!("Bearer {}", self.token)
            .parse::<MetadataValue<_>>()
            .map_err(|_| tonic::Status::internal("Invalid admin token"))?;
        request.metadata_mut().insert("authorization", header);
        Ok(request)
    }

    async fn list_users(
        &mut self,
        query: &str,
    ) -> AppResult<taptime_schema::services::ListUsersResponse> {
        Ok(self
            .inner
            .list_users(self.request(ListUsersRequest {
                query: query.to_string(),
                limit: 100,
                offset: 0,
            })?)
            .await?
            .into_inner())
    }

    async fn get_user_detail(&mut self, user_id: ProtoUuid) -> AppResult<AdminUserDetail> {
        Ok(self
            .inner
            .get_user_detail(
                self.request(taptime_schema::services::GetUserDetailRequest {
                    user_id: Some(user_id),
                })?,
            )
            .await?
            .into_inner())
    }

    async fn get_user_stats(&mut self, user_id: ProtoUuid) -> AppResult<AdminUserStats> {
        Ok(self
            .inner
            .get_user_stats(self.request(GetUserStatsRequest {
                user_id: Some(user_id),
            })?)
            .await?
            .into_inner())
    }

    async fn list_bans(&mut self, include_inactive: bool) -> AppResult<Vec<BanRecord>> {
        Ok(self
            .inner
            .list_bans(self.request(ListBansRequest {
                kind: BanKind::Unspecified as i32,
                include_inactive,
            })?)
            .await?
            .into_inner()
            .bans)
    }

    async fn create_user_ban(&mut self, user_id: ProtoUuid) -> AppResult<()> {
        self.inner
            .create_ban(self.request(CreateBanRequest {
                kind: BanKind::User as i32,
                user_id: Some(user_id),
                ip_cidr: String::new(),
                reason: "Admin ban".to_string(),
                expires_at: None,
            })?)
            .await?;
        Ok(())
    }

    async fn create_ip_ban(&mut self, ip_cidr: String) -> AppResult<()> {
        self.inner
            .create_ban(self.request(CreateBanRequest {
                kind: BanKind::Ip as i32,
                user_id: None,
                ip_cidr,
                reason: "Admin ban".to_string(),
                expires_at: None,
            })?)
            .await?;
        Ok(())
    }

    async fn revoke_ban(&mut self, kind: BanKind, ban_id: ProtoUuid) -> AppResult<()> {
        self.inner
            .revoke_ban(self.request(RevokeBanRequest {
                ban_id: Some(ban_id),
                kind: kind as i32,
            })?)
            .await?;
        Ok(())
    }

    async fn delete_user_time_data(&mut self, user_id: ProtoUuid) -> AppResult<()> {
        self.inner
            .delete_user_time_data(self.request(DeleteUserRequest {
                user_id: Some(user_id),
            })?)
            .await?;
        Ok(())
    }

    async fn delete_user_account(&mut self, user_id: ProtoUuid) -> AppResult<()> {
        self.inner
            .delete_user_account(self.request(DeleteUserRequest {
                user_id: Some(user_id),
            })?)
            .await?;
        Ok(())
    }
}

#[derive(Debug)]
enum AdminConnectError {
    Transport(tonic::transport::Error),
    Login(tonic::Status),
}

impl fmt::Display for AdminConnectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(error) => write!(formatter, "Admin API connection failed: {error}"),
            Self::Login(error) => write!(formatter, "Admin login failed: {error}"),
        }
    }
}

impl Error for AdminConnectError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transport(error) => Some(error),
            Self::Login(error) => Some(error),
        }
    }
}

#[derive(Debug)]
enum AuthenticationError {
    Rejected,
    Other(Box<dyn Error + Send + Sync>),
}

impl fmt::Display for AuthenticationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected => formatter.write_str("Invalid admin password"),
            Self::Other(error) => error.fmt(formatter),
        }
    }
}

impl Error for AuthenticationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Rejected => None,
            Self::Other(error) => Some(error.as_ref()),
        }
    }
}

#[allow(async_fn_in_trait)]
trait Authenticator {
    type Client;

    async fn authenticate(
        &mut self,
        endpoint: &str,
        password: String,
    ) -> Result<Self::Client, AuthenticationError>;
}

struct NetworkAuthenticator;

impl Authenticator for NetworkAuthenticator {
    type Client = AdminClient;

    async fn authenticate(
        &mut self,
        endpoint: &str,
        password: String,
    ) -> Result<Self::Client, AuthenticationError> {
        match AdminClient::connect(endpoint.to_string(), password).await {
            Ok(client) => Ok(client),
            Err(AdminConnectError::Login(status))
                if status.code() == tonic::Code::Unauthenticated =>
            {
                Err(AuthenticationError::Rejected)
            }
            Err(error) => Err(AuthenticationError::Other(Box::new(error))),
        }
    }
}

trait LoginPrompter {
    fn password(&mut self) -> AppResult<Zeroizing<String>>;
    fn confirm_save(&mut self, endpoint: &str) -> AppResult<bool>;
    fn notice(&mut self, message: &str);
    fn warning(&mut self, message: &str);
}

struct TerminalPrompter;

impl LoginPrompter for TerminalPrompter {
    fn password(&mut self) -> AppResult<Zeroizing<String>> {
        Ok(Zeroizing::new(read_password("Admin password: ")?))
    }

    fn confirm_save(&mut self, endpoint: &str) -> AppResult<bool> {
        read_confirmation(&format!("Save password securely for {endpoint}? [y/N] "))
    }

    fn notice(&mut self, message: &str) {
        eprintln!("{message}");
    }

    fn warning(&mut self, message: &str) {
        eprintln!("Warning: {message}");
    }
}

async fn login_with_credentials<A, S, P>(
    endpoint: &CredentialEndpoint,
    cache_disabled: bool,
    authenticator: &mut A,
    store: &mut S,
    prompter: &mut P,
) -> AppResult<A::Client>
where
    A: Authenticator,
    S: CredentialStore,
    P: LoginPrompter,
{
    let cache_enabled = !cache_disabled && endpoint.cache_allowed();
    if !cache_disabled && !endpoint.cache_allowed() {
        prompter.warning(
            "password caching is disabled for remote plaintext HTTP; use HTTPS to enable it",
        );
    }

    let mut vault_available = cache_enabled;
    if cache_enabled {
        match store.get_password(endpoint.account()) {
            Ok(password) => {
                let password = Zeroizing::new(password);
                match authenticator
                    .authenticate(endpoint.endpoint(), password.to_string())
                    .await
                {
                    Ok(client) => return Ok(client),
                    Err(AuthenticationError::Rejected) => {
                        if let Err(error) = store.delete_password(endpoint.account())
                            && error != CredentialError::NotFound
                        {
                            prompter.warning(&format!(
                                "could not remove the rejected cached password: {error}"
                            ));
                        }
                        prompter
                            .notice("The cached admin password was rejected and has been removed.");
                    }
                    Err(AuthenticationError::Other(error)) => return Err(error),
                }
            }
            Err(CredentialError::NotFound) => {}
            Err(CredentialError::Unavailable(error)) => {
                vault_available = false;
                prompter.warning(&format!(
                    "the operating system credential store is unavailable ({error}); continuing without caching"
                ));
            }
        }
    }

    let password = prompter.password()?;
    let client = match authenticator
        .authenticate(endpoint.endpoint(), password.to_string())
        .await
    {
        Ok(client) => client,
        Err(AuthenticationError::Rejected) => return Err("Invalid admin password".into()),
        Err(AuthenticationError::Other(error)) => return Err(error),
    };

    if vault_available && prompter.confirm_save(endpoint.endpoint())? {
        if let Err(error) = store.set_password(endpoint.account(), password.as_str()) {
            prompter.warning(&format!(
                "could not save the admin password securely: {error}"
            ));
        } else {
            prompter.notice("Admin password saved in the operating system credential store.");
        }
    }

    Ok(client)
}

fn forget_password(
    endpoint: &CredentialEndpoint,
    store: &mut impl CredentialStore,
) -> AppResult<()> {
    match store.delete_password(endpoint.account()) {
        Ok(()) => println!("Removed the cached password for {}.", endpoint.endpoint()),
        Err(CredentialError::NotFound) => {
            println!("No cached password exists for {}.", endpoint.endpoint());
        }
        Err(error) => return Err(format!("Could not access the credential store: {error}").into()),
    }
    Ok(())
}

#[tokio::main]
async fn main() -> AppResult<()> {
    let args = Args::parse();
    match args.command {
        Some(Command::HashPassword { password }) => {
            let password = Zeroizing::new(match password {
                Some(password) => password,
                None => read_password("Admin password: ")?,
            });
            println!("{}", hash_password(&password)?);
            Ok(())
        }
        Some(Command::ForgetPassword) => {
            let endpoint = CredentialEndpoint::parse(&args.admin_api_url)?;
            forget_password(&endpoint, &mut SystemCredentialStore)
        }
        None => {
            let endpoint = CredentialEndpoint::parse(&args.admin_api_url)?;
            let client = login_with_credentials(
                &endpoint,
                args.no_password_cache,
                &mut NetworkAuthenticator,
                &mut SystemCredentialStore,
                &mut TerminalPrompter,
            )
            .await?;
            run_tui(client).await
        }
    }
}

async fn run_tui(client: AdminClient) -> AppResult<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new(client).await?;

    let result = loop {
        terminal.draw(|frame| draw(frame, &app))?;
        if app.should_quit {
            break Ok(());
        }
        if app.stats_refresh_due()
            && let Err(err) = app.load_selected_stats().await
        {
            app.status = format!("Stats refresh failed: {err}");
            app.stats_last_loaded = Some(Instant::now());
        }
        if event::poll(Duration::from_millis(150))?
            && let Event::Key(key) = event::read()?
            && let Err(err) = app.handle_key(key).await
        {
            app.status = err.to_string();
        }
    };

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn draw(frame: &mut Frame, app: &App) {
    let [tabs_area, body_area, footer_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(3),
    ])
    .areas(frame.area());

    let selected_tab = if app.tab == Tab::Users { 0 } else { 1 };
    let tabs = Tabs::new(["Users", "Bans"])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("TapTime Admin"),
        )
        .select(selected_tab)
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(tabs, tabs_area);

    match app.tab {
        Tab::Users => draw_users(frame, body_area, app),
        Tab::Bans => draw_bans(frame, body_area, app),
    }
    draw_footer(frame, footer_area, app);
    draw_mode(frame, app);
}

fn draw_users(frame: &mut Frame, area: Rect, app: &App) {
    let [list_area, detail_area] =
        Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)]).areas(area);
    let items = app.users.iter().map(|item| {
        let user = item.user.as_ref();
        let marker = if item.user_banned { " banned" } else { "" };
        ListItem::new(vec![
            Line::from(format!(
                "{}{}",
                user.map(|u| u.email.as_str()).unwrap_or("<missing user>"),
                marker
            )),
            Line::from(Span::styled(
                user.map(|u| u.name.as_str()).unwrap_or(""),
                Style::default().fg(Color::DarkGray),
            )),
        ])
    });
    let mut state = ListState::default();
    if !app.users.is_empty() {
        state.select(Some(app.selected_user));
    }
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Users [{}]", app.query)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray));
    frame.render_stateful_widget(list, list_area, &mut state);

    let (title, detail, scroll) = match app.user_pane {
        UserPane::Account => (
            "Account",
            match &app.detail {
                Some(detail) => detail_lines(detail),
                None => vec![Line::from("No user selected")],
            },
            0,
        ),
        UserPane::Stats => (
            "Stats",
            match (&app.stats, &app.detail) {
                (Some(stats), Some(detail)) => stats_lines(stats, detail.user.as_ref()),
                _ => vec![Line::from("No user statistics loaded")],
            },
            app.stats_scroll,
        ),
    };
    let paragraph = Paragraph::new(detail)
        .block(Block::default().borders(Borders::ALL).title(title))
        .scroll((scroll, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, detail_area);
}

fn draw_bans(frame: &mut Frame, area: Rect, app: &App) {
    let items = app.bans.iter().map(|ban| {
        let kind = ban_kind_label(ban);
        let subject = ban_subject(ban);
        ListItem::new(vec![
            Line::from(format!("{kind} {subject}")),
            Line::from(Span::styled(
                format!("{} {}", active_label(ban), ban.reason),
                Style::default().fg(Color::DarkGray),
            )),
        ])
    });
    let mut state = ListState::default();
    if !app.bans.is_empty() {
        state.select(Some(app.selected_ban));
    }
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Active Bans"))
        .highlight_style(Style::default().bg(Color::DarkGray));
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let help = match app.tab {
        Tab::Users => {
            "q quit | tab bans | s account/stats | pgup/pgdn scroll | / search | r refresh | b ban | p ban ip | d data | x account"
        }
        Tab::Bans => "q quit | tab users | r refresh | u revoke selected ban | p ban ip",
    };
    let text = vec![Line::from(help), Line::from(app.status.as_str())];
    frame.render_widget(
        Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Status")),
        area,
    );
}

fn draw_mode(frame: &mut Frame, app: &App) {
    let (title, text) = match &app.mode {
        Mode::Normal => return,
        Mode::Search { input } => ("Search users", format!("Query: {input}")),
        Mode::BanIp { input } => ("Ban IP/CIDR", format!("IP/CIDR: {input}")),
        Mode::Confirm { action, input } => (
            action.label(),
            format!("Type {} to confirm: {input}", action.expected()),
        ),
    };
    let area = centered_rect(62, 24, frame.area());
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(title))
            .alignment(Alignment::Center),
        area,
    );
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct LiveDayMetrics {
    clocked: i64,
    presence: i64,
    balance: i64,
    first_check_in: Option<i64>,
    last_check_out: Option<i64>,
    checked_in: bool,
}

fn stats_line_count(stats: &AdminUserStats) -> usize {
    40 + stats
        .today_summary
        .as_ref()
        .and_then(|summary| summary.day.as_ref())
        .map(|day| day.events.len().max(1))
        .unwrap_or(1)
}

fn clamp_scroll(scroll: u16, line_count: usize) -> u16 {
    scroll.min(line_count.saturating_sub(1) as u16)
}

fn stats_lines(stats: &AdminUserStats, user: Option<&User>) -> Vec<Line<'static>> {
    let Some(summary) = stats.today_summary.as_ref() else {
        return vec![Line::from("Missing today's summary")];
    };
    let Some(day) = summary.day.as_ref() else {
        return vec![Line::from("Missing today's day")];
    };

    let now_seconds = user
        .and_then(user_time_zone)
        .map(|time_zone| {
            let time = Utc::now().with_timezone(&time_zone).time();
            i64::from(time.num_seconds_from_midnight())
        })
        .unwrap_or_else(|| i64::from(Utc::now().time().num_seconds_from_midnight()));
    let live = live_day_metrics(summary, now_seconds);
    let (month_clocked, month_overtime, month_undertime) =
        live_aggregate(stats.month_to_date.as_ref(), summary, live, true);
    let overall_includes_today = match (&stats.overall_start, &stats.today) {
        (Some(start), Some(today)) => start.days_since_epoch <= today.days_since_epoch,
        _ => true,
    };
    let (overall_clocked, overall_overtime, overall_undertime) = live_aggregate(
        stats.overall.as_ref(),
        summary,
        live,
        overall_includes_today,
    );

    let status = if live.checked_in {
        "Checked in"
    } else if day.events.is_empty() {
        "No events"
    } else {
        "Checked out"
    };
    let target = duration_seconds(summary.work_target.as_ref());
    let lunch = duration_seconds(day.lunch_break_duration.as_ref());
    let mut lines = vec![
        Line::from(format!(
            "User: {}",
            user.map(|user| user.name.as_str()).unwrap_or("-")
        )),
        Line::from(format!(
            "Timezone: {}",
            user.and_then(|user| user.time_zone.as_ref())
                .map(|time_zone| time_zone.time_zone.as_str())
                .unwrap_or("UTC")
        )),
        Line::from(format!("Updated: {}", fmt_ts(stats.generated_at))),
        Line::from(""),
        Line::from(Span::styled(
            format!("Today ({})", fmt_date(stats.today.as_ref())),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("Status: {status}")),
        Line::from(format!(
            "Day type: {}",
            day_kind(day, summary.before_start_date)
        )),
        Line::from(format!(
            "First check-in: {}",
            fmt_clock(live.first_check_in)
        )),
        Line::from(format!("Last checkout: {}", fmt_clock(live.last_check_out))),
        Line::from(format!("Work: {}", fmt_duration(live.clocked))),
        Line::from(format!("Presence: {}", fmt_duration(live.presence))),
        Line::from(format!(
            "Target: {} (+{} lunch)",
            fmt_duration(target),
            fmt_duration(lunch)
        )),
        balance_line("Balance", live.balance),
        Line::from("Events:"),
    ];
    if day.events.is_empty() {
        lines.push(Line::from("  none"));
    } else {
        for event in &day.events {
            let (kind, time) = match event.event_type.as_ref() {
                Some(EventType::CheckIn(time)) => ("IN ", time_to_seconds(time)),
                Some(EventType::CheckOut(time)) => ("OUT", time_to_seconds(time)),
                None => ("?  ", None),
            };
            lines.push(Line::from(format!("  {kind} {}", fmt_clock(time))));
        }
    }

    push_aggregate_lines(
        &mut lines,
        "Month to date",
        stats.month_to_date.as_ref(),
        month_clocked,
        month_overtime,
        month_undertime,
    );
    push_aggregate_lines(
        &mut lines,
        &format!("Overall (since {})", fmt_date(stats.overall_start.as_ref())),
        stats.overall.as_ref(),
        overall_clocked,
        overall_overtime,
        overall_undertime,
    );
    lines
}

fn user_time_zone(user: &User) -> Option<chrono_tz::Tz> {
    user.time_zone.as_ref()?.time_zone.parse().ok()
}

fn live_day_metrics(summary: &DaySummary, current_seconds: i64) -> LiveDayMetrics {
    let Some(day) = summary.day.as_ref() else {
        return LiveDayMetrics::default();
    };
    let mut clocked = 0;
    let mut open_check_in = None;
    let mut first_check_in = None;
    let mut last_check_out = None;
    for event in &day.events {
        match event.event_type.as_ref() {
            Some(EventType::CheckIn(time)) => {
                let seconds = time_to_seconds(time);
                first_check_in = first_check_in.or(seconds);
                open_check_in = seconds;
            }
            Some(EventType::CheckOut(time)) => {
                let seconds = time_to_seconds(time);
                last_check_out = seconds;
                if let (Some(check_in), Some(check_out)) = (open_check_in.take(), seconds) {
                    clocked += (check_out - check_in).max(0);
                }
            }
            None => {}
        }
    }
    if let Some(check_in) = open_check_in {
        clocked += (current_seconds - check_in).max(0);
    }
    let checked_in = open_check_in.is_some();
    let presence = match first_check_in {
        None => 0,
        Some(first) if checked_in => (current_seconds - first).max(0),
        Some(first) => last_check_out
            .map(|last| (last - first).max(0))
            .unwrap_or(0),
    };
    let balance = if summary.before_start_date || !is_regular_required_day(day) {
        server_balance_seconds(summary)
    } else {
        presence - required_presence_seconds(day)
    };
    LiveDayMetrics {
        clocked,
        presence,
        balance,
        first_check_in,
        last_check_out,
        checked_in,
    }
}

fn live_aggregate(
    stats: Option<&MonthlyStats>,
    today: &DaySummary,
    live: LiveDayMetrics,
    include_today: bool,
) -> (i64, i64, i64) {
    let mut clocked = duration_seconds(stats.and_then(|stats| stats.total_clocked_work.as_ref()));
    let mut overtime = duration_seconds(stats.and_then(|stats| stats.overtime.as_ref()));
    let mut undertime = duration_seconds(stats.and_then(|stats| stats.undertime.as_ref()));
    if include_today {
        let closed_clocked = duration_seconds(today.clocked_work.as_ref());
        let closed_balance = server_balance_seconds(today);
        clocked = (clocked + live.clocked - closed_clocked).max(0);
        overtime = (overtime + live.balance.max(0) - closed_balance.max(0)).max(0);
        undertime = (undertime + (-live.balance).max(0) - (-closed_balance).max(0)).max(0);
    }
    (clocked, overtime, undertime)
}

fn push_aggregate_lines(
    lines: &mut Vec<Line<'static>>,
    title: &str,
    stats: Option<&MonthlyStats>,
    clocked: i64,
    overtime: i64,
    undertime: i64,
) {
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        title.to_string(),
        Style::default().add_modifier(Modifier::BOLD),
    )));
    lines.push(balance_line("Balance", overtime - undertime));
    lines.push(Line::from(format!("Clocked: {}", fmt_duration(clocked))));
    lines.push(Line::from(format!("Overtime: {}", fmt_duration(overtime))));
    lines.push(Line::from(format!(
        "Undertime: {}",
        fmt_duration(undertime)
    )));
    let stats = stats.cloned().unwrap_or_default();
    lines.push(Line::from(format!("Worked days: {}", stats.worked_days)));
    lines.push(Line::from(format!("Remote: {}", stats.remote_days)));
    lines.push(Line::from(format!("Day off: {}", stats.day_offs)));
    lines.push(Line::from(format!("Vacation: {}", stats.vacation_days)));
    lines.push(Line::from(format!("Skipped: {}", stats.skipped_days)));
    lines.push(Line::from(format!(
        "Weekend work: {}",
        stats.full_weekend_work_days
    )));
    lines.push(Line::from(format!(
        "Vacation work: {}",
        stats.full_vacation_work_days
    )));
}

fn duration_seconds(duration: Option<&prost_types::Duration>) -> i64 {
    duration.map(|duration| duration.seconds).unwrap_or(0)
}

fn time_to_seconds(time: &taptime_schema::LocalTime) -> Option<i64> {
    (time.hour < 24 && time.minute < 60 && time.second < 60)
        .then_some(i64::from(time.hour * 3600 + time.minute * 60 + time.second))
}

fn required_presence_seconds(day: &Day) -> i64 {
    let work = duration_seconds(day.required_work_hours.as_ref());
    if work <= 0 {
        0
    } else {
        work + duration_seconds(day.lunch_break_duration.as_ref())
    }
}

fn is_regular_required_day(day: &Day) -> bool {
    let non_regular = DayFlag::Weekend as u32
        | DayFlag::DayOff as u32
        | DayFlag::Remote as u32
        | DayFlag::Vacation as u32;
    day.flags & non_regular == 0
}

fn day_kind(day: &Day, before_start_date: bool) -> String {
    if before_start_date {
        return "Before start".to_string();
    }
    let mut labels = Vec::new();
    if day.flags & DayFlag::Weekend as u32 != 0 {
        labels.push("Weekend");
    }
    if day.flags & DayFlag::Remote as u32 != 0 {
        labels.push("Remote");
    }
    if day.flags & DayFlag::DayOff as u32 != 0 {
        labels.push("Day off");
    }
    if day.flags & DayFlag::Vacation as u32 != 0 {
        labels.push("Vacation");
    }
    if labels.is_empty() {
        "Regular".to_string()
    } else {
        labels.join(", ")
    }
}

fn server_balance_seconds(summary: &DaySummary) -> i64 {
    match summary
        .balance
        .as_ref()
        .and_then(|balance| balance.balance_type.as_ref())
    {
        Some(BalanceType::Overtime(duration)) => duration.seconds,
        Some(BalanceType::UnderTime(duration)) => -duration.seconds,
        _ => 0,
    }
}

fn fmt_duration(seconds: i64) -> String {
    let sign = if seconds < 0 { "-" } else { "" };
    let seconds = seconds.saturating_abs();
    format!("{sign}{}h {:02}m", seconds / 3600, seconds % 3600 / 60)
}

fn fmt_signed_duration(seconds: i64) -> String {
    if seconds > 0 {
        format!("+{}", fmt_duration(seconds))
    } else {
        fmt_duration(seconds)
    }
}

fn balance_line(label: &str, seconds: i64) -> Line<'static> {
    let color = if seconds > 0 {
        Color::Green
    } else if seconds < 0 {
        Color::Red
    } else {
        Color::Reset
    };
    Line::from(vec![
        Span::raw(format!("{label}: ")),
        Span::styled(fmt_signed_duration(seconds), Style::default().fg(color)),
    ])
}

fn fmt_clock(seconds: Option<i64>) -> String {
    seconds
        .map(|seconds| {
            format!(
                "{:02}:{:02}:{:02}",
                seconds / 3600,
                seconds % 3600 / 60,
                seconds % 60
            )
        })
        .unwrap_or_else(|| "-".to_string())
}

fn fmt_date(value: Option<&taptime_schema::Date>) -> String {
    value
        .and_then(|date| NaiveDate::from_epoch_days(date.days_since_epoch))
        .map(|date| date.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn detail_lines(detail: &AdminUserDetail) -> Vec<Line<'static>> {
    let Some(user) = &detail.user else {
        return vec![Line::from("Missing user")];
    };
    let mut lines = vec![
        Line::from(format!("Name: {}", user.name)),
        Line::from(format!("Email: {}", user.email)),
        Line::from(format!(
            "Organization: {}",
            user.organization.as_deref().unwrap_or("-")
        )),
        Line::from(format!("ID: {}", user_id(user))),
        Line::from(format!("Created: {}", fmt_ts(user.created_at))),
        Line::from(format!("Last seen: {}", fmt_ts(user.last_seen))),
        Line::from(format!("Events: {}", detail.event_count)),
        Line::from(format!("Flag days: {}", detail.day_flag_count)),
        Line::from(""),
        Line::from("Known IPs:"),
    ];
    if detail.known_ips.is_empty() {
        lines.push(Line::from("  none"));
    } else {
        for ip in &detail.known_ips {
            lines.push(Line::from(format!(
                "  {} seen {}x last {}",
                ip.ip,
                ip.request_count,
                fmt_ts(ip.last_seen)
            )));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from("User bans:"));
    if detail.bans.is_empty() {
        lines.push(Line::from("  none"));
    } else {
        for ban in &detail.bans {
            lines.push(Line::from(format!(
                "  {} {} {}",
                active_label(ban),
                fmt_ts(ban.created_at),
                ban.reason
            )));
        }
    }
    lines
}

fn user_id(user: &User) -> String {
    user.id
        .as_ref()
        .map(proto_uuid)
        .unwrap_or_else(|| "-".to_string())
}

fn proto_uuid(id: &ProtoUuid) -> String {
    let id: Uuid = id.into();
    id.to_string()
}

fn fmt_ts(value: Option<prost_types::Timestamp>) -> String {
    value
        .and_then(|ts| Utc.timestamp_opt(ts.seconds, ts.nanos as u32).single())
        .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn ban_kind_label(ban: &BanRecord) -> &'static str {
    match BanKind::try_from(ban.kind).unwrap_or(BanKind::Unspecified) {
        BanKind::User => "USER",
        BanKind::Ip => "IP",
        BanKind::Unspecified => "BAN",
    }
}

fn ban_subject(ban: &BanRecord) -> String {
    match BanKind::try_from(ban.kind).unwrap_or(BanKind::Unspecified) {
        BanKind::User => ban
            .user_id
            .as_ref()
            .map(proto_uuid)
            .unwrap_or_else(|| "-".to_string()),
        BanKind::Ip => ban.ip_cidr.clone(),
        BanKind::Unspecified => "-".to_string(),
    }
}

fn active_label(ban: &BanRecord) -> &'static str {
    if ban.active { "active" } else { "inactive" }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let [_, area, _] = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .areas(area);
    let [_, area, _] = Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .areas(area);
    area
}

fn move_index(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    (current as isize + delta).clamp(0, len as isize - 1) as usize
}

fn hash_password(password: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| format!("Hashing failed: {e}"))?
        .to_string())
}

struct RawModeGuard;

impl RawModeGuard {
    fn enable() -> io::Result<Self> {
        enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

fn read_password(prompt: &str) -> AppResult<String> {
    print!("{prompt}");
    use std::io::Write;
    io::stdout().flush()?;
    let raw_mode = RawModeGuard::enable()?;
    let mut password = String::new();
    loop {
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Enter => break,
                KeyCode::Backspace => {
                    password.pop();
                }
                KeyCode::Char(ch) => password.push(ch),
                KeyCode::Esc => {
                    return Err("Cancelled".into());
                }
                _ => {}
            }
        }
    }
    drop(raw_mode);
    println!();
    Ok(password)
}

fn read_confirmation(prompt: &str) -> AppResult<bool> {
    print!("{prompt}");
    use std::io::Write;
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[derive(Default)]
    struct FakeStore {
        password: Option<String>,
        get_error: Option<CredentialError>,
        set_error: Option<CredentialError>,
        delete_error: Option<CredentialError>,
        get_calls: usize,
        set_calls: usize,
        delete_calls: usize,
    }

    impl CredentialStore for FakeStore {
        fn get_password(&mut self, _account: &str) -> Result<String, CredentialError> {
            self.get_calls += 1;
            if let Some(error) = self.get_error.take() {
                return Err(error);
            }
            self.password.clone().ok_or(CredentialError::NotFound)
        }

        fn set_password(&mut self, _account: &str, password: &str) -> Result<(), CredentialError> {
            self.set_calls += 1;
            if let Some(error) = self.set_error.take() {
                return Err(error);
            }
            self.password = Some(password.to_string());
            Ok(())
        }

        fn delete_password(&mut self, _account: &str) -> Result<(), CredentialError> {
            self.delete_calls += 1;
            if let Some(error) = self.delete_error.take() {
                return Err(error);
            }
            if self.password.take().is_some() {
                Ok(())
            } else {
                Err(CredentialError::NotFound)
            }
        }
    }

    enum AuthenticationOutcome {
        Success,
        Rejected,
        Failed,
    }

    struct FakeAuthenticator {
        outcomes: VecDeque<AuthenticationOutcome>,
        attempts: Vec<(String, String)>,
    }

    impl FakeAuthenticator {
        fn new(outcomes: impl IntoIterator<Item = AuthenticationOutcome>) -> Self {
            Self {
                outcomes: outcomes.into_iter().collect(),
                attempts: Vec::new(),
            }
        }
    }

    impl Authenticator for FakeAuthenticator {
        type Client = ();

        async fn authenticate(
            &mut self,
            endpoint: &str,
            password: String,
        ) -> Result<Self::Client, AuthenticationError> {
            self.attempts.push((endpoint.to_string(), password));
            match self.outcomes.pop_front().unwrap() {
                AuthenticationOutcome::Success => Ok(()),
                AuthenticationOutcome::Rejected => Err(AuthenticationError::Rejected),
                AuthenticationOutcome::Failed => Err(AuthenticationError::Other(Box::new(
                    io::Error::other("server unavailable"),
                ))),
            }
        }
    }

    #[derive(Default)]
    struct FakePrompter {
        passwords: VecDeque<String>,
        save_answers: VecDeque<bool>,
        notices: Vec<String>,
        warnings: Vec<String>,
    }

    impl LoginPrompter for FakePrompter {
        fn password(&mut self) -> AppResult<Zeroizing<String>> {
            Ok(Zeroizing::new(self.passwords.pop_front().unwrap()))
        }

        fn confirm_save(&mut self, _endpoint: &str) -> AppResult<bool> {
            Ok(self.save_answers.pop_front().unwrap())
        }

        fn notice(&mut self, message: &str) {
            self.notices.push(message.to_string());
        }

        fn warning(&mut self, message: &str) {
            self.warnings.push(message.to_string());
        }
    }

    fn credential_endpoint(value: &str) -> CredentialEndpoint {
        CredentialEndpoint::parse(value).unwrap()
    }

    fn duration(seconds: i64) -> prost_types::Duration {
        prost_types::Duration { seconds, nanos: 0 }
    }

    fn local_time(hour: u32, minute: u32) -> taptime_schema::LocalTime {
        taptime_schema::LocalTime {
            hour,
            minute,
            second: 0,
        }
    }

    fn check_in(hour: u32, minute: u32) -> taptime_schema::Event {
        taptime_schema::Event {
            id: None,
            event_type: Some(EventType::CheckIn(local_time(hour, minute))),
        }
    }

    fn check_out(hour: u32, minute: u32) -> taptime_schema::Event {
        taptime_schema::Event {
            id: None,
            event_type: Some(EventType::CheckOut(local_time(hour, minute))),
        }
    }

    fn summary(events: Vec<taptime_schema::Event>) -> DaySummary {
        DaySummary {
            day: Some(Day {
                date: Some(taptime_schema::Date {
                    days_since_epoch: 19_723,
                }),
                events,
                flags: 0,
                required_work_hours: Some(duration(8 * 60 * 60)),
                lunch_break_duration: Some(duration(30 * 60)),
            }),
            clocked_work: Some(duration(60 * 60)),
            balance: Some(taptime_schema::Balance {
                balance_type: Some(BalanceType::UnderTime(duration(8 * 60 * 60 + 30 * 60))),
            }),
            skipped: false,
            full_day_worked: false,
            required_work_hours_overridden: false,
            work_target: Some(duration(8 * 60 * 60)),
            before_start_date: false,
        }
    }

    #[test]
    fn move_index_clamps_to_valid_range() {
        assert_eq!(move_index(0, 0, 1), 0);
        assert_eq!(move_index(0, 3, -1), 0);
        assert_eq!(move_index(1, 3, 1), 2);
        assert_eq!(move_index(2, 3, 1), 2);
    }

    #[test]
    fn confirmation_words_match_plan() {
        assert_eq!(
            Action::DeleteData(ProtoUuid::default()).expected(),
            "DELETE DATA"
        );
        assert_eq!(
            Action::DeleteAccount(ProtoUuid::default()).expected(),
            "DELETE ACCOUNT"
        );
        assert_eq!(Action::BanIp("127.0.0.1".into()).expected(), "BAN IP");
    }

    #[test]
    fn user_pane_toggle_round_trips() {
        assert_eq!(UserPane::Account.toggled(), UserPane::Stats);
        assert_eq!(UserPane::Stats.toggled(), UserPane::Account);
    }

    #[test]
    fn scroll_is_clamped_to_available_lines() {
        assert_eq!(clamp_scroll(12, 0), 0);
        assert_eq!(clamp_scroll(12, 5), 4);
        assert_eq!(clamp_scroll(3, 5), 3);
    }

    #[test]
    fn live_metrics_include_open_session_and_keep_event_order() {
        let summary = summary(vec![check_in(9, 0), check_out(10, 0), check_in(11, 0)]);

        let metrics = live_day_metrics(&summary, 12 * 60 * 60);

        assert_eq!(metrics.clocked, 2 * 60 * 60);
        assert_eq!(metrics.presence, 3 * 60 * 60);
        assert_eq!(metrics.balance, -(5 * 60 * 60 + 30 * 60));
        assert_eq!(metrics.first_check_in, Some(9 * 60 * 60));
        assert_eq!(metrics.last_check_out, Some(10 * 60 * 60));
        assert!(metrics.checked_in);

        let stats = AdminUserStats {
            today: summary.day.as_ref().and_then(|day| day.date),
            overall_start: summary.day.as_ref().and_then(|day| day.date),
            generated_at: None,
            today_summary: Some(summary),
            month_to_date: Some(MonthlyStats::default()),
            overall: Some(MonthlyStats::default()),
        };
        let rendered = stats_lines(&stats, None)
            .into_iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        let first_in = rendered
            .iter()
            .position(|line| line.contains("IN  09:00:00"))
            .unwrap();
        let checkout = rendered
            .iter()
            .position(|line| line.contains("OUT 10:00:00"))
            .unwrap();
        let second_in = rendered
            .iter()
            .position(|line| line.contains("IN  11:00:00"))
            .unwrap();
        assert!(first_in < checkout && checkout < second_in);
    }

    #[test]
    fn live_aggregate_replaces_closed_today_contribution() {
        let summary = summary(vec![check_in(9, 0), check_out(10, 0), check_in(11, 0)]);
        let metrics = live_day_metrics(&summary, 12 * 60 * 60);
        let stats = MonthlyStats {
            total_clocked_work: Some(duration(60 * 60)),
            overtime: Some(duration(0)),
            undertime: Some(duration(8 * 60 * 60 + 30 * 60)),
            ..Default::default()
        };

        let (clocked, overtime, undertime) = live_aggregate(Some(&stats), &summary, metrics, true);

        assert_eq!(clocked, 2 * 60 * 60);
        assert_eq!(overtime, 0);
        assert_eq!(undertime, 5 * 60 * 60 + 30 * 60);
    }

    #[test]
    fn signed_duration_format_preserves_balance_sign() {
        assert_eq!(fmt_signed_duration(3_900), "+1h 05m");
        assert_eq!(fmt_signed_duration(-3_900), "-1h 05m");
        assert_eq!(fmt_signed_duration(0), "0h 00m");
    }

    #[tokio::test]
    async fn prompted_password_is_saved_only_after_confirmation() {
        let endpoint = credential_endpoint("https://example.com");
        let mut store = FakeStore::default();
        let mut authenticator = FakeAuthenticator::new([AuthenticationOutcome::Success]);
        let mut prompter = FakePrompter {
            passwords: VecDeque::from(["new-secret".to_string()]),
            save_answers: VecDeque::from([true]),
            ..Default::default()
        };

        login_with_credentials(
            &endpoint,
            false,
            &mut authenticator,
            &mut store,
            &mut prompter,
        )
        .await
        .unwrap();

        assert_eq!(store.get_calls, 1);
        assert_eq!(store.set_calls, 1);
        assert_eq!(store.password.as_deref(), Some("new-secret"));
        assert_eq!(prompter.notices.len(), 1);
    }

    #[tokio::test]
    async fn declined_save_does_not_write_the_vault() {
        let endpoint = credential_endpoint("https://example.com");
        let mut store = FakeStore::default();
        let mut authenticator = FakeAuthenticator::new([AuthenticationOutcome::Success]);
        let mut prompter = FakePrompter {
            passwords: VecDeque::from(["one-time".to_string()]),
            save_answers: VecDeque::from([false]),
            ..Default::default()
        };

        login_with_credentials(
            &endpoint,
            false,
            &mut authenticator,
            &mut store,
            &mut prompter,
        )
        .await
        .unwrap();

        assert_eq!(store.set_calls, 0);
        assert!(store.password.is_none());
    }

    #[tokio::test]
    async fn cache_bypass_never_accesses_the_vault() {
        let endpoint = credential_endpoint("https://example.com");
        let mut store = FakeStore {
            password: Some("cached".to_string()),
            ..Default::default()
        };
        let mut authenticator = FakeAuthenticator::new([AuthenticationOutcome::Success]);
        let mut prompter = FakePrompter {
            passwords: VecDeque::from(["one-time".to_string()]),
            ..Default::default()
        };

        login_with_credentials(
            &endpoint,
            true,
            &mut authenticator,
            &mut store,
            &mut prompter,
        )
        .await
        .unwrap();

        assert_eq!(store.get_calls, 0);
        assert_eq!(store.set_calls, 0);
        assert_eq!(authenticator.attempts[0].1, "one-time");
    }

    #[tokio::test]
    async fn unavailable_vault_warns_and_uses_one_time_password() {
        let endpoint = credential_endpoint("https://example.com");
        let mut store = FakeStore {
            get_error: Some(CredentialError::Unavailable("vault locked".to_string())),
            ..Default::default()
        };
        let mut authenticator = FakeAuthenticator::new([AuthenticationOutcome::Success]);
        let mut prompter = FakePrompter {
            passwords: VecDeque::from(["one-time".to_string()]),
            ..Default::default()
        };

        login_with_credentials(
            &endpoint,
            false,
            &mut authenticator,
            &mut store,
            &mut prompter,
        )
        .await
        .unwrap();

        assert_eq!(store.set_calls, 0);
        assert!(prompter.warnings[0].contains("vault locked"));
    }

    #[tokio::test]
    async fn remote_http_warns_and_never_accesses_the_vault() {
        let endpoint = credential_endpoint("http://server:50051");
        let mut store = FakeStore {
            password: Some("cached".to_string()),
            ..Default::default()
        };
        let mut authenticator = FakeAuthenticator::new([AuthenticationOutcome::Success]);
        let mut prompter = FakePrompter {
            passwords: VecDeque::from(["one-time".to_string()]),
            ..Default::default()
        };

        login_with_credentials(
            &endpoint,
            false,
            &mut authenticator,
            &mut store,
            &mut prompter,
        )
        .await
        .unwrap();

        assert_eq!(store.get_calls, 0);
        assert_eq!(store.set_calls, 0);
        assert!(prompter.warnings[0].contains("remote plaintext HTTP"));
    }

    #[tokio::test]
    async fn save_failure_warns_but_keeps_successful_login() {
        let endpoint = credential_endpoint("https://example.com");
        let mut store = FakeStore {
            set_error: Some(CredentialError::Unavailable("vault locked".to_string())),
            ..Default::default()
        };
        let mut authenticator = FakeAuthenticator::new([AuthenticationOutcome::Success]);
        let mut prompter = FakePrompter {
            passwords: VecDeque::from(["new-secret".to_string()]),
            save_answers: VecDeque::from([true]),
            ..Default::default()
        };

        login_with_credentials(
            &endpoint,
            false,
            &mut authenticator,
            &mut store,
            &mut prompter,
        )
        .await
        .unwrap();

        assert_eq!(store.set_calls, 1);
        assert!(prompter.warnings[0].contains("vault locked"));
    }

    #[tokio::test]
    async fn rejected_cached_password_is_deleted_and_replaced() {
        let endpoint = credential_endpoint("https://example.com");
        let mut store = FakeStore {
            password: Some("stale-secret".to_string()),
            ..Default::default()
        };
        let mut authenticator = FakeAuthenticator::new([
            AuthenticationOutcome::Rejected,
            AuthenticationOutcome::Success,
        ]);
        let mut prompter = FakePrompter {
            passwords: VecDeque::from(["fresh-secret".to_string()]),
            save_answers: VecDeque::from([true]),
            ..Default::default()
        };

        login_with_credentials(
            &endpoint,
            false,
            &mut authenticator,
            &mut store,
            &mut prompter,
        )
        .await
        .unwrap();

        assert_eq!(store.delete_calls, 1);
        assert_eq!(store.set_calls, 1);
        assert_eq!(store.password.as_deref(), Some("fresh-secret"));
        assert_eq!(authenticator.attempts[0].1, "stale-secret");
        assert_eq!(authenticator.attempts[1].1, "fresh-secret");
        assert!(prompter.notices[0].contains("rejected"));
    }

    #[tokio::test]
    async fn transport_failure_preserves_cached_password_and_hides_it_from_errors() {
        let endpoint = credential_endpoint("https://example.com");
        let mut store = FakeStore {
            password: Some("do-not-print-this".to_string()),
            ..Default::default()
        };
        let mut authenticator = FakeAuthenticator::new([AuthenticationOutcome::Failed]);
        let mut prompter = FakePrompter::default();

        let error = login_with_credentials(
            &endpoint,
            false,
            &mut authenticator,
            &mut store,
            &mut prompter,
        )
        .await
        .unwrap_err();

        assert_eq!(store.delete_calls, 0);
        assert_eq!(store.password.as_deref(), Some("do-not-print-this"));
        assert!(!error.to_string().contains("do-not-print-this"));
    }

    #[tokio::test]
    async fn rejected_prompted_password_is_not_saved_or_printed() {
        let endpoint = credential_endpoint("https://example.com");
        let mut store = FakeStore::default();
        let mut authenticator = FakeAuthenticator::new([AuthenticationOutcome::Rejected]);
        let mut prompter = FakePrompter {
            passwords: VecDeque::from(["do-not-print-this".to_string()]),
            ..Default::default()
        };

        let error = login_with_credentials(
            &endpoint,
            false,
            &mut authenticator,
            &mut store,
            &mut prompter,
        )
        .await
        .unwrap_err();

        assert_eq!(store.set_calls, 0);
        assert!(!error.to_string().contains("do-not-print-this"));
    }

    #[test]
    fn forget_password_is_idempotent() {
        let endpoint = credential_endpoint("https://example.com");
        let mut store = FakeStore::default();
        forget_password(&endpoint, &mut store).unwrap();
        assert_eq!(store.delete_calls, 1);
    }

    #[test]
    fn cli_accepts_cache_controls() {
        let args = Args::try_parse_from([
            "taptime_admin_cli",
            "--admin-api-url=https://example.com",
            "--no-password-cache",
        ])
        .unwrap();
        assert!(args.no_password_cache);
        assert_eq!(args.admin_api_url, "https://example.com");

        let args = Args::try_parse_from([
            "taptime_admin_cli",
            "forget-password",
            "--admin-api-url=https://example.com",
        ])
        .unwrap();
        assert!(matches!(args.command, Some(Command::ForgetPassword)));
    }
}
